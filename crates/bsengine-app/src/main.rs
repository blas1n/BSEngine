use bsengine_app::new_app;
use bsengine_input::InputPlugin;
use bsengine_render::RenderPlugin;
use bsengine_rhi_wgpu::WgpuRHIPlugin;
use bsengine_window::WindowPlugin;

fn main() {
    new_app()
        .add_plugins(WgpuRHIPlugin)
        .add_plugins(WindowPlugin::default())
        .add_plugins(InputPlugin)
        .add_plugins(RenderPlugin)
        .run();
}
