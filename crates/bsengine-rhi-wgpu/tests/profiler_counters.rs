//! The profiler's texture counters are process-global, so this test gets a
//! process of its own.
//!
//! `texture_memory_bytes()` / `texture_count()` are two `static` atomics
//! shared by every `TrackedTexture` in the process. This test samples them,
//! creates one texture, samples again, drops it, samples a third time, and
//! expects the pair of samples to differ by exactly that texture. As a unit
//! test inside the library's test binary it ran in the same process as every
//! other test in the crate, and most of those build an offscreen
//! `WgpuSurface`, which allocates its own tracked render targets. Whenever
//! one of them happened to allocate between two of this test's samples the
//! delta was off by that surface's targets -- 7,372,800 bytes on the run
//! that prompted this file (expected 16,384, saw 7,389,184) -- and the test
//! failed, stopping `cargo test --workspace` at this crate so that every
//! crate after it never ran. It passed alone and on every rerun, which is
//! how it survived for a month as a "known flake".
//!
//! Neither a lock nor a retry fixes that. A lock in this test cannot stop
//! *engine* code in other tests from creating textures. A retry that waits
//! for a quiet window would still pass a broken implementation whenever
//! concurrent noise happened to cancel out, and would hide the flake rather
//! than remove it. What does fix it is process isolation: an integration
//! test file is its own binary, Cargo runs test binaries one at a time, and
//! this file has exactly one test -- so between any two samples below,
//! nothing else in the process can touch the counters.
//!
//! ⚠️ Keep this file to one test. A second test here would reintroduce the
//! race inside this binary.

use bsengine_rhi_wgpu::profiler::{create_tracked_texture, texture_count, texture_memory_bytes};
use bsengine_rhi_wgpu::surface::WgpuSurface;

#[test]
fn create_tracked_texture_increments_global_counters_and_drop_decrements_them() {
    // The surface itself owns tracked textures; that is fine -- they are
    // created before the first sample and outlive the last one.
    let surface = pollster::block_on(WgpuSurface::new_offscreen(16, 16, false))
        .expect("these tests need an adapter; a skip here would look like a pass");
    let device = surface.device_arc();

    let before_bytes = texture_memory_bytes();
    let before_count = texture_count();

    let desc = wgpu::TextureDescriptor {
        label: Some("profiler counter test texture"),
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
    assert_eq!(
        texture_memory_bytes(),
        before_bytes + expected_bytes,
        "creating one 64x64 Rgba8Unorm texture must add exactly its size to the global total"
    );
    assert_eq!(
        texture_count(),
        before_count + 1,
        "creating one texture must add exactly one to the global count"
    );

    drop(tracked);
    assert_eq!(
        texture_memory_bytes(),
        before_bytes,
        "dropping the texture must give back exactly what creating it took"
    );
    assert_eq!(
        texture_count(),
        before_count,
        "dropping the texture must decrement the count it incremented"
    );
}
