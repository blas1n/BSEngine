use bevy_app::{App, Plugin, PostUpdate, Update};
use bevy_ecs::prelude::{EventReader, Query};
use bsengine_core::{Camera, Transform};
use bsengine_ecs::Res;
use bsengine_rhi_wgpu::{GpuMeshRegistry, WgpuSurfaceResource};
use bsengine_window::WindowResized;
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

fn update_camera_aspect(mut events: EventReader<WindowResized>, mut cameras: Query<&mut Camera>) {
    for ev in events.read() {
        for mut cam in cameras.iter_mut() {
            cam.update_aspect_ratio(ev.width, ev.height);
        }
    }
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<WindowResized>();
        app.add_systems(Update, update_camera_aspect);
        app.add_systems(PostUpdate, render_frame);
    }
}

#[cfg(test)]
mod tests {
    use super::RenderPlugin;
    use bsengine_app::new_app;
    use bsengine_core::Camera;
    use bsengine_rhi_wgpu::WgpuRHIPlugin;
    use bsengine_window::WindowResized;

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

    #[test]
    fn camera_aspect_updates_on_window_resize() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(RenderPlugin);

        let cam_entity = app.world_mut().spawn(Camera::default()).id();
        // 800x600 (4:3) is different from the default 16:9
        app.world_mut().send_event(WindowResized {
            width: 800,
            height: 600,
        });
        app.update();

        let cam = app.world().get::<Camera>(cam_entity).unwrap();
        let expected = 800.0_f32 / 600.0_f32;
        assert!((cam.aspect_ratio - expected).abs() < 1e-4);
    }
}
