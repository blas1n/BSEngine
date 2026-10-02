//! Frame/GPU statistics: texture memory tracking, draw-call/triangle counting,
//! and feature-gated GPU pass timing. See
//! `docs/superpowers/specs/2026-08-27-frame-profiler-gpu-debugger-design.md`.

use std::ops::Deref;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

static TEXTURE_MEMORY_BYTES: AtomicU64 = AtomicU64::new(0);
static TEXTURE_COUNT: AtomicU32 = AtomicU32::new(0);
static STREAMED_TEXTURE_BYTES: AtomicU64 = AtomicU64::new(0);
static STREAMING_BUDGET_BYTES: AtomicU64 = AtomicU64::new(0);
static TEXTURES_BELOW_WANTED: AtomicU32 = AtomicU32::new(0);

/// What the texture streamer last reported: bytes the streamed textures
/// hold between them, the budget they are held to (`0` for none), and how
/// many of them hold less than the screen asks for. Written once per frame
/// by `bsengine-render`'s streaming system, read into [`FrameStats`] like
/// the texture totals are, so the profiler panel and `get_frame_stats` see
/// the streamer's state without a resource of their own.
pub fn record_streaming(resident_bytes: u64, budget_bytes: u64, below_wanted: u32) {
    STREAMED_TEXTURE_BYTES.store(resident_bytes, Ordering::Relaxed);
    STREAMING_BUDGET_BYTES.store(budget_bytes, Ordering::Relaxed);
    TEXTURES_BELOW_WANTED.store(below_wanted, Ordering::Relaxed);
}

/// The last [`record_streaming`] values as
/// `(resident_bytes, budget_bytes, textures_below_wanted)`.
pub fn streaming_snapshot() -> (u64, u64, u32) {
    (
        STREAMED_TEXTURE_BYTES.load(Ordering::Relaxed),
        STREAMING_BUDGET_BYTES.load(Ordering::Relaxed),
        TEXTURES_BELOW_WANTED.load(Ordering::Relaxed),
    )
}

/// How many frames of [`FrameStats`] `WgpuSurface` keeps in its rolling
/// history -- roughly 2 seconds at 60fps. Older frames are dropped as new
/// ones arrive.
pub const FRAME_STATS_HISTORY_CAPACITY: usize = 120;

/// Current total GPU texture memory tracked via [`create_tracked_texture`]/
/// [`create_tracked_texture_with_data`], across every texture this crate has
/// created and not yet dropped -- shadow maps, post-process targets, asset
/// textures, mesh thumbnails, everything. Live/instantaneous, not tied to any
/// particular frame.
pub fn texture_memory_bytes() -> u64 {
    TEXTURE_MEMORY_BYTES.load(Ordering::Relaxed)
}

/// Count of currently-live textures tracked the same way as
/// [`texture_memory_bytes`].
pub fn texture_count() -> u32 {
    TEXTURE_COUNT.load(Ordering::Relaxed)
}

/// Bytes per texel for the texture formats this crate actually creates.
/// Deliberately a closed match over known formats rather than depending on a
/// wgpu block-size API whose exact surface in this workspace's pinned
/// version wasn't verified while writing this plan -- an unmatched format
/// logs a warning and falls back to 4 bytes/texel (correct for every RGBA8
/// variant, an undercount only for wider formats) rather than panicking.
pub(crate) fn bytes_per_texel(format: wgpu::TextureFormat) -> u64 {
    known_bytes_per_texel(format).unwrap_or_else(|| {
        tracing::warn!(
            "profiler::bytes_per_texel: unhandled format {format:?}, assuming 4 bytes/texel"
        );
        4
    })
}

/// [`bytes_per_texel`] for the formats it knows, `None` for the rest. Apart
/// so a test can tell a known 4-byte format from the 4-byte fallback, which
/// read the same through `bytes_per_texel` -- only the warning differs.
fn known_bytes_per_texel(format: wgpu::TextureFormat) -> Option<u64> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        // A WebGPU canvas's formats (see `WgpuSurface::new`).
        | wgpu::TextureFormat::Bgra8Unorm
        | wgpu::TextureFormat::Bgra8UnormSrgb
        | wgpu::TextureFormat::Depth32Float
        | wgpu::TextureFormat::R32Float
        // The BRDF integration LUT: two 16-bit floats, so also 4 bytes.
        | wgpu::TextureFormat::Rg16Float => Some(4),
        wgpu::TextureFormat::Rgba16Float => Some(8),
        _ => None,
    }
}

/// Bytes one mip level of `width` x `height` takes in `format`: texels times
/// bytes per texel, or, for a block-compressed format, 4x4 blocks (rounded
/// up -- a 1x1 level is still one block) times bytes per block. The number
/// the texture registry writes per level and the streaming budget is spent
/// in, so it lives here beside the footprint that sums it.
pub(crate) fn level_bytes(format: wgpu::TextureFormat, width: u32, height: u32) -> u64 {
    let blocks = |bytes_per_block: u64| {
        (width as u64).div_ceil(4) * (height as u64).div_ceil(4) * bytes_per_block
    };
    match format {
        wgpu::TextureFormat::Bc1RgbaUnorm | wgpu::TextureFormat::Bc1RgbaUnormSrgb => blocks(8),
        wgpu::TextureFormat::Bc3RgbaUnorm | wgpu::TextureFormat::Bc3RgbaUnormSrgb => blocks(16),
        other => width as u64 * height as u64 * bytes_per_texel(other),
    }
}

fn texture_size_bytes(desc: &wgpu::TextureDescriptor) -> u64 {
    // Level by level rather than by the 4/3 rule of thumb: a
    // block-compressed level is a whole number of blocks, and the streaming
    // budget counts exactly what the registry uploads.
    (0..desc.mip_level_count)
        .map(|level| {
            let width = (desc.size.width >> level).max(1);
            let height = (desc.size.height >> level).max(1);
            level_bytes(desc.format, width, height) * desc.size.depth_or_array_layers as u64
        })
        .sum()
}

/// A `wgpu::Texture` whose GPU memory footprint is counted in the global
/// [`texture_memory_bytes`]/[`texture_count`] totals for as long as it's
/// alive. `Deref`s to `wgpu::Texture` so every existing `.create_view(...)`
/// call site needs no changes beyond swapping the creation call and the
/// holding struct field's type.
pub struct TrackedTexture {
    texture: wgpu::Texture,
    size_bytes: u64,
}

impl Deref for TrackedTexture {
    type Target = wgpu::Texture;
    fn deref(&self) -> &wgpu::Texture {
        &self.texture
    }
}

impl TrackedTexture {
    /// What this texture contributes to [`texture_memory_bytes`]: the size
    /// computed from its descriptor at creation, every declared mip level
    /// included. Per texture, so a test can watch one texture's footprint
    /// while others come and go on other threads.
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

impl Drop for TrackedTexture {
    fn drop(&mut self) {
        TEXTURE_MEMORY_BYTES.fetch_sub(self.size_bytes, Ordering::Relaxed);
        TEXTURE_COUNT.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Creates a texture via `device.create_texture` and tracks its memory
/// footprint until it's dropped.
pub fn create_tracked_texture(
    device: &wgpu::Device,
    desc: &wgpu::TextureDescriptor,
) -> TrackedTexture {
    let texture = device.create_texture(desc);
    let size_bytes = texture_size_bytes(desc);
    TEXTURE_MEMORY_BYTES.fetch_add(size_bytes, Ordering::Relaxed);
    TEXTURE_COUNT.fetch_add(1, Ordering::Relaxed);
    TrackedTexture {
        texture,
        size_bytes,
    }
}

/// Same as [`create_tracked_texture`] but for the `create_texture_with_data`
/// convenience path (creates and uploads in one call) that `mesh_thumbnail.rs`
/// uses.
pub fn create_tracked_texture_with_data(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    desc: &wgpu::TextureDescriptor,
    order: wgpu::util::TextureDataOrder,
    data: &[u8],
) -> TrackedTexture {
    use wgpu::util::DeviceExt;
    let texture = device.create_texture_with_data(queue, desc, order, data);
    let size_bytes = texture_size_bytes(desc);
    TEXTURE_MEMORY_BYTES.fetch_add(size_bytes, Ordering::Relaxed);
    TEXTURE_COUNT.fetch_add(1, Ordering::Relaxed);
    TrackedTexture {
        texture,
        size_bytes,
    }
}

/// One GPU render pass's measured duration. Only produced when the adapter
/// supports `wgpu::Features::TIMESTAMP_QUERY` -- see `FrameStats::gpu_timestamps_supported`.
#[derive(Clone, Debug)]
pub struct PassTiming {
    /// Human-readable name of the render pass this timing was measured for.
    pub name: String,
    /// Measured GPU duration of the pass, in milliseconds.
    pub duration_ms: f32,
}

/// One frame's worth of profiling data, as reported by `WgpuSurface::render_frame`
/// and consumed by `ProfilerPanel` and the `get_frame_stats` MCP tool.
#[derive(Clone, Debug)]
pub struct FrameStats {
    /// Total CPU-side time spent building and submitting this frame, in milliseconds.
    pub cpu_frame_time_ms: f32,
    /// Per-pass GPU timings, empty when [`Self::gpu_timestamps_supported`] is false.
    pub gpu_pass_times_ms: Vec<PassTiming>,
    /// How many frames before this one the GPU produced [`Self::gpu_pass_times_ms`]:
    /// the timings are read back a few frames late so reading them never
    /// stalls the CPU (see `WgpuSurface::start_timestamp_readback`). `None`
    /// until the first readback lands, and always without timestamp support.
    pub gpu_pass_times_frames_ago: Option<u32>,
    /// Whether the adapter supports `wgpu::Features::TIMESTAMP_QUERY`, i.e.
    /// whether [`Self::gpu_pass_times_ms`] carries real data.
    pub gpu_timestamps_supported: bool,
    /// Number of draw calls issued this frame.
    pub draw_calls: u32,
    /// Number of objects drawn this frame.
    ///
    /// Separate from [`Self::draw_calls`] because instanced passes submit
    /// many objects per draw call. This answers "how much did the frame
    /// draw", while `draw_calls` answers "how many API calls did that
    /// cost". Where a pass is not instanced the two rise together.
    pub objects_drawn: u32,
    /// Number of triangles submitted this frame.
    pub triangles: u64,
    /// Number of entities dropped this frame by occlusion culling --
    /// entities that passed frustum culling but were found completely
    /// hidden behind an `Occluder`. Zero when occlusion culling is off or
    /// no occluders exist.
    pub occluded_count: u32,
    /// Snapshot of [`texture_memory_bytes`] at the time this frame's stats were collected.
    pub texture_memory_bytes: u64,
    /// Snapshot of [`texture_count`] at the time this frame's stats were collected.
    pub texture_count: u32,
    /// Bytes the streamed textures held between them when the streamer last
    /// ran; a part of [`Self::texture_memory_bytes`].
    pub streamed_texture_bytes: u64,
    /// The budget those bytes are held to, `0` for none.
    pub streaming_budget_bytes: u64,
    /// How many streamed textures held less than the screen asked for --
    /// still coming in, or held down by the budget.
    pub textures_below_wanted: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device() -> (std::sync::Arc<wgpu::Device>, std::sync::Arc<wgpu::Queue>) {
        let surface = pollster::block_on(crate::surface::WgpuSurface::new_offscreen(16, 16, false))
            .expect("these tests need an adapter; a skip here would look like a pass");
        (surface.device_arc(), surface.queue_arc())
    }

    // ⚠️ The counter test -- "create increments the global totals, drop
    // decrements them" -- is deliberately NOT here. It lives in
    // `tests/profiler_counters.rs`, alone in its own binary. The totals are
    // process-global statics, and this module's test binary runs every test
    // in the crate in one process, most of them building an offscreen
    // surface that allocates tracked targets of its own; whenever one did so
    // between the test's two samples, the delta was off by that surface and
    // the test failed (expected +16,384, saw +7,389,184), taking the whole
    // `cargo test --workspace` down with it via fail-fast. Moving it back
    // here reintroduces that race. See the header of that file.

    /// What the streamer records is what the frame stats read back. Only
    /// this test in the binary writes these statics, so unlike the texture
    /// totals there is nothing to race.
    #[test]
    fn the_streaming_snapshot_is_what_was_last_recorded() {
        record_streaming(12_345, 67_890, 3);
        assert_eq!(streaming_snapshot(), (12_345, 67_890, 3));
        record_streaming(0, 0, 0);
        assert_eq!(streaming_snapshot(), (0, 0, 0));
    }

    #[test]
    fn tracked_texture_derefs_to_wgpu_texture_for_create_view() {
        let (device, _queue) = test_device();
        let desc = wgpu::TextureDescriptor {
            label: Some("deref test"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        let tracked = create_tracked_texture(&device, &desc);
        // Compiles only if Deref<Target = wgpu::Texture> works -- this is the point of the test.
        let _view = tracked.create_view(&wgpu::TextureViewDescriptor::default());
    }

    #[test]
    fn bytes_per_texel_matches_known_formats() {
        assert_eq!(bytes_per_texel(wgpu::TextureFormat::Rgba8Unorm), 4);
        assert_eq!(bytes_per_texel(wgpu::TextureFormat::Rgba8UnormSrgb), 4);
        assert_eq!(bytes_per_texel(wgpu::TextureFormat::Rgba16Float), 8);
        assert_eq!(bytes_per_texel(wgpu::TextureFormat::Depth32Float), 4);
        assert_eq!(bytes_per_texel(wgpu::TextureFormat::R32Float), 4);
        assert_eq!(bytes_per_texel(wgpu::TextureFormat::Rg16Float), 4);
    }

    /// The browser's canvas formats (the swapchain and its sRGB view) are
    /// known, not the fallback, which warned four times as every browser
    /// build started.
    #[test]
    fn the_web_canvas_formats_are_known() {
        use wgpu::TextureFormat as F;
        assert_eq!(known_bytes_per_texel(F::Bgra8Unorm), Some(4));
        assert_eq!(known_bytes_per_texel(F::Bgra8UnormSrgb), Some(4));
        // ...and the split still reports an unknown one as unknown.
        assert_eq!(known_bytes_per_texel(F::Rg8Unorm), None);
    }
}
