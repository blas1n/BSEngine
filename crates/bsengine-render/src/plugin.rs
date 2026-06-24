use bevy_app::{App, Plugin, PostUpdate};
use bevy_ecs::prelude::Query;
use bsengine_core::{Camera, Transform};
use bsengine_ecs::Res;
use bsengine_rhi_wgpu::{GpuMeshRegistry, WgpuSurfaceResource};
use glam::Mat4;

use crate::components::MeshRenderer;

fn render_frame(
    surface: Option<Res<WgpuSurfaceResource>>,
    registry: Option<Res<GpuMeshRegistry>>,
    camera_query: Query<(&Camera, &Transform)>,
    mesh_query: Query<(&MeshRenderer, &Transform)>,
) {
    let (Some(surface), Some(registry)) = (surface, registry) else {
        return;
    };

    let view_proj = camera_query
        .iter()
        .next()
        .map(|(cam, t)| cam.projection_matrix() * t.view_matrix())
        .unwrap_or(Mat4::IDENTITY);

    let draw_calls: Vec<(u64, Mat4)> = mesh_query
        .iter()
        .map(|(mr, t)| (mr.mesh_id, t.to_matrix()))
        .collect();

    if let Err(e) = surface.0.render_frame(view_proj, &draw_calls, &registry) {
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
    fn render_plugin_runs_without_surface() {
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
