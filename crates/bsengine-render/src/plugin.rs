use bevy_app::{App, Plugin, PostUpdate};
use bsengine_ecs::Res;
use bsengine_rhi_wgpu::RhiResource;

fn clear_pass(_rhi: Option<Res<RhiResource>>) {
    let Some(_rhi) = _rhi else { return };
    // Placeholder: real surface clear will be added in Phase 4
    // when wgpu Surface is wired from the winit window handle.
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, clear_pass);
    }
}

#[cfg(test)]
mod tests {
    use super::RenderPlugin;
    use bsengine_app::new_app;
    use bsengine_rhi_wgpu::WgpuRHIPlugin;

    #[test]
    fn render_plugin_runs_without_rhi() {
        // RenderPlugin must not panic when RhiResource is absent
        let mut app = new_app();
        app.add_plugins(RenderPlugin);
        app.update();
    }

    #[test]
    fn render_plugin_runs_with_rhi() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(RenderPlugin);
        app.update();
        app.update();
        app.update();
    }
}
