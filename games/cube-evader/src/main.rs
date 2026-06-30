use bsengine_app::new_app;
use bsengine_render::RenderPlugin;
use bsengine_rhi_wgpu::WgpuRHIPlugin;
use bsengine_window::{WindowDescriptor, WindowPlugin};

fn main() {
    new_app()
        .add_plugins(WgpuRHIPlugin)
        .add_plugins(WindowPlugin {
            descriptor: WindowDescriptor {
                title: "Cube Evader".to_string(),
                width: 1280,
                height: 720,
                resizable: true,
            },
        })
        .add_plugins(RenderPlugin)
        .run();
}
