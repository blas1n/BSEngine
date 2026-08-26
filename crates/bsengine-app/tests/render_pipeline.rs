use bevy_app::Update;
use bsengine_app::new_app;
use bsengine_asset::AssetPlugin;
use bsengine_ecs::{ResMut, Resource};
use bsengine_render::RenderPlugin;
use bsengine_rhi_wgpu::WgpuRHIPlugin;

#[derive(Resource, Default)]
struct FrameCount(u32);

fn count_frames(mut count: ResMut<FrameCount>) {
    count.0 += 1;
}

#[test]
fn render_pipeline_runs_multiple_frames() {
    let mut app = new_app();
    app.add_plugins(AssetPlugin)
        .add_plugins(WgpuRHIPlugin::windowed())
        .add_plugins(RenderPlugin)
        .init_resource::<FrameCount>()
        .add_systems(Update, count_frames);

    app.update();
    app.update();
    app.update();

    assert_eq!(app.world().resource::<FrameCount>().0, 3);
}
