use bevy_app::Update;
use bsengine_app::new_app;
use bsengine_ecs::{ResMut, Resource};
use bsengine_render::RenderPlugin;
use bsengine_rhi_wgpu::{RhiResource, WgpuRHIPlugin};

#[derive(Resource, Default)]
struct FrameCount(u32);

fn count_frames(mut count: ResMut<FrameCount>) {
    count.0 += 1;
}

#[test]
fn render_pipeline_runs_multiple_frames() {
    let mut app = new_app();
    app.add_plugins(WgpuRHIPlugin)
        .add_plugins(RenderPlugin)
        .init_resource::<FrameCount>()
        .add_systems(Update, count_frames);

    app.update();
    app.update();
    app.update();

    assert_eq!(app.world().resource::<FrameCount>().0, 3);
}

#[test]
fn rhi_resource_accessible_after_plugin() {
    let mut app = new_app();
    app.add_plugins(WgpuRHIPlugin).add_plugins(RenderPlugin);

    app.update();

    let rhi = app.world().resource::<RhiResource>();
    let _mesh = rhi.0.create_mesh();
    let _shader = rhi.0.create_shader("// stub");
    let _texture = rhi.0.create_texture();
}
