//! Frame/GPU statistics: texture memory tracking, draw-call/triangle counting,
//! and feature-gated GPU pass timing. See
//! `docs/superpowers/specs/2026-08-27-frame-profiler-gpu-debugger-design.md`.

use std::ops::Deref;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

static TEXTURE_MEMORY_BYTES: AtomicU64 = AtomicU64::new(0);
static TEXTURE_COUNT: AtomicU32 = AtomicU32::new(0);

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
    match format {
        wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        | wgpu::TextureFormat::Depth32Float
        | wgpu::TextureFormat::R32Float
        // The BRDF integration LUT: two 16-bit floats, so also 4 bytes.
        | wgpu::TextureFormat::Rg16Float => 4,
        wgpu::TextureFormat::Rgba16Float => 8,
        other => {
            tracing::warn!(
                "profiler::bytes_per_texel: unhandled format {other:?}, assuming 4 bytes/texel"
            );
            4
        }
    }
}

fn texture_size_bytes(desc: &wgpu::TextureDescriptor) -> u64 {
    let texels_per_mip0 =
        desc.size.width as u64 * desc.size.height as u64 * desc.size.depth_or_array_layers as u64;
    // Full mip chain sums to a bit less than 4/3 of the base level; mip_level_count
    // is almost always 1 in this crate today, so this only matters if that changes.
    let mip_factor = if desc.mip_level_count <= 1 {
        1.0
    } else {
        (1.0 - 0.25f64.powi(desc.mip_level_count as i32)) / 0.75
    };
    ((texels_per_mip0 as f64) * mip_factor) as u64 * bytes_per_texel(desc.format)
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
    /// Whether the adapter supports `wgpu::Features::TIMESTAMP_QUERY`, i.e.
    /// whether [`Self::gpu_pass_times_ms`] carries real data.
    pub gpu_timestamps_supported: bool,
    /// Number of draw calls issued this frame.
    pub draw_calls: u32,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device() -> (std::sync::Arc<wgpu::Device>, std::sync::Arc<wgpu::Queue>) {
        let surface = pollster::block_on(crate::surface::WgpuSurface::new_offscreen(16, 16, false))
            .expect("these tests need an adapter; a skip here would look like a pass");
        (surface.device_arc(), surface.queue_arc())
    }

    #[test]
    fn create_tracked_texture_increments_global_counters_and_drop_decrements_them() {
        let (device, _queue) = test_device();
        let before_bytes = texture_memory_bytes();
        let before_count = texture_count();

        let desc = wgpu::TextureDescriptor {
            label: Some("profiler test texture"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        let expected_bytes = 64u64 * 64 * 4; // width * height * 4 bytes/texel (Rgba8Unorm)

        let tracked = create_tracked_texture(&device, &desc);
        assert_eq!(texture_memory_bytes(), before_bytes + expected_bytes);
        assert_eq!(texture_count(), before_count + 1);

        drop(tracked);
        assert_eq!(texture_memory_bytes(), before_bytes);
        assert_eq!(texture_count(), before_count);
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
}
