use bevy_app::{App, Plugin, PostUpdate};
use bsengine_ecs::Res;
use bsengine_rhi_wgpu::WgpuSurfaceResource;

fn render_frame(surface: Option<Res<WgpuSurfaceResource>>) {
    let Some(surface) = surface else { return };
    if let Err(e) = surface.0.render_frame() {
        tracing::warn!("render_frame error: {e}");
    }
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, render_frame);
    }
}

#[cfg(test)]
mod tests {
    use super::RenderPlugin;
    use bsengine_app::new_app;
    use bsengine_rhi_wgpu::WgpuRHIPlugin;

    #[test]
    fn render_plugin_runs_without_rhi() {
        let mut app = new_app();
        app.add_plugins(RenderPlugin);
        app.update();
    }

    #[test]
    fn render_plugin_runs_with_rhi_headless() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(RenderPlugin);
        app.update();
        app.update();
        app.update();
    }
}
