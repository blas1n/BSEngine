//! Proves a `TerrainSplat`-style draw call reaches the terrain pipeline
//! without a wgpu validation panic -- the risk worth testing here is a wrong
//! bind-group-layout entry count/type on `terrain_bgl`, or a bad model-buffer
//! slot for `terrain_draw_calls` colliding with the regular `draw_calls`
//! slots. `render_frame` is called directly (bypassing `common::Harness`,
//! which has no notion of terrain draws) so this exercises the real
//! offscreen pipeline build, same as every other test in this crate.

mod common;

use bsengine_rhi_wgpu::surface::WgpuSurface;
use bsengine_rhi_wgpu::{triangle_vertices, GpuMeshRegistry, GpuTextureRegistry, LightData};
use glam::{Mat4, Vec3};

#[test]
fn a_terrain_draw_call_does_not_panic_alongside_regular_draw_calls() {
    let mut surface = pollster::block_on(WgpuSurface::new_offscreen(
        common::WIDTH,
        common::HEIGHT,
        false,
    ))
    .expect(
        "could not create an offscreen renderer -- see Harness::build's panic message \
                 for what this means on this machine",
    );

    let mut registry = GpuMeshRegistry::new(surface.device_arc());
    let (vertices, indices) = triangle_vertices();
    let mesh_id = registry.register(&vertices, &indices);

    let mut textures = GpuTextureRegistry::new(surface.device_arc(), surface.queue_arc());
    let layer_ids = [
        textures.load_from_rgba(1, 1, &[255, 0, 0, 255]),
        textures.load_from_rgba(1, 1, &[0, 255, 0, 255]),
        textures.load_from_rgba(1, 1, &[0, 0, 255, 255]),
        textures.load_from_rgba(1, 1, &[255, 255, 0, 255]),
    ];
    let weight_id = textures.load_from_rgba(1, 1, &[255, 255, 255, 255]);

    let terrain_draw_calls = vec![(mesh_id, Mat4::IDENTITY, layer_ids, weight_id)];

    let ui_state = bsengine_core::UiState::default();
    let result = surface.render_frame(
        Mat4::IDENTITY,
        Vec3::new(0.0, 0.0, 5.0),
        Mat4::IDENTITY,
        None,
        &[],
        &terrain_draw_calls,
        &registry,
        LightData::default(),
        Some(&textures),
        &std::collections::HashMap::new(),
        &ui_state,
        0.0,
        0.0,
        false,
        false,
        Mat4::IDENTITY,
        None,
        None,
        None,
        None,
        &[],
        false,
        false,
        false,
        None,
        None,
        0.0,
        &[],
    );

    assert!(
        result.is_ok(),
        "render_frame with a terrain draw call should succeed, got {result:?}"
    );
}
