use bevy_app::{App, Plugin, PostUpdate, Update};
use bevy_ecs::prelude::{Entity, EventReader, IntoSystemConfigs, ParamSet, Query, Without};
use bsengine_core::{
    Camera, DirectionalLight, GlobalTransform, Material, Parent, PointLight, Transform,
};
use bsengine_ecs::Res;
use bsengine_rhi_wgpu::{
    GpuMeshRegistry, GpuTextureRegistry, LightData, PointLightEntry, WgpuSurfaceResource,
};
use bsengine_window::WindowResized;
use glam::Mat4;
use std::collections::HashMap;

use crate::components::MeshRenderer;

/// Pass 1: root entities (no Parent) get GlobalTransform = local Transform.
fn propagate_roots(mut query: Query<(&Transform, &mut GlobalTransform), Without<Parent>>) {
    for (t, mut gt) in query.iter_mut() {
        gt.0 = t.to_matrix();
    }
}

/// Pass 2: children get GlobalTransform = parent's GT * local Transform.
/// Uses ParamSet to safely read root GlobalTransforms and write child GlobalTransforms.
fn propagate_children(
    mut set: ParamSet<(
        Query<(Entity, &GlobalTransform), Without<Parent>>,
        Query<(&Transform, &mut GlobalTransform, &Parent)>,
    )>,
) {
    let parent_mats: HashMap<Entity, Mat4> = set.p0().iter().map(|(e, gt)| (e, gt.0)).collect();

    for (t, mut gt, parent) in set.p1().iter_mut() {
        if let Some(&mat) = parent_mats.get(&parent.0) {
            gt.0 = mat * t.to_matrix();
        }
    }
}

fn render_frame(
    surface: Option<Res<WgpuSurfaceResource>>,
    registry: Option<Res<GpuMeshRegistry>>,
    tex_registry: Option<Res<GpuTextureRegistry>>,
    camera_query: Query<(&Camera, &Transform)>,
    mesh_query: Query<(
        &MeshRenderer,
        &Transform,
        Option<&GlobalTransform>,
        Option<&Material>,
    )>,
    light_query: Query<&DirectionalLight>,
    point_light_query: Query<(&PointLight, Option<&GlobalTransform>, &Transform)>,
) {
    let (Some(surface), Some(registry)) = (surface, registry) else {
        return;
    };

    let view_proj = camera_query
        .iter()
        .next()
        .map(|(cam, t)| cam.projection_matrix() * t.view_matrix())
        .unwrap_or(Mat4::IDENTITY);

    let draw_calls: Vec<(u64, Mat4, Option<u64>)> = mesh_query
        .iter()
        .map(|(mr, t, gt, mat)| {
            let model = gt.map(|g| g.to_matrix()).unwrap_or_else(|| t.to_matrix());
            let tex_id = mat.and_then(|m| m.texture_id);
            (mr.mesh_id, model, tex_id)
        })
        .collect();

    let collected_point_lights: Vec<PointLightEntry> = point_light_query
        .iter()
        .map(|(pl, gt, t)| {
            let pos = gt
                .map(|g| g.to_matrix().w_axis.truncate())
                .unwrap_or(t.translation);
            PointLightEntry {
                position: pos,
                color: pl.color,
                intensity: pl.intensity,
                range: pl.range,
            }
        })
        .collect();

    let light = if let Some(l) = light_query.iter().next() {
        LightData {
            direction: l.direction,
            color: l.color,
            ambient: l.ambient,
            point_lights: collected_point_lights,
        }
    } else {
        LightData {
            point_lights: collected_point_lights,
            ..Default::default()
        }
    };

    let tex_reg_ref = tex_registry.as_deref();

    if let Err(e) = surface
        .0
        .render_frame(view_proj, &draw_calls, &registry, light, tex_reg_ref)
    {
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
        app.add_systems(
            PostUpdate,
            (propagate_roots, propagate_children, render_frame).chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::RenderPlugin;
    use bsengine_app::new_app;
    use bsengine_core::{Camera, PointLight, Transform};
    use bsengine_rhi_wgpu::WgpuRHIPlugin;
    use bsengine_window::WindowResized;
    use glam::Vec3;

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
        app.world_mut().send_event(WindowResized {
            width: 800,
            height: 600,
        });
        app.update();

        let cam = app.world().get::<Camera>(cam_entity).unwrap();
        let expected = 800.0_f32 / 600.0_f32;
        assert!((cam.aspect_ratio - expected).abs() < 1e-4);
    }

    #[test]
    fn render_plugin_accepts_point_lights() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(RenderPlugin);
        app.world_mut().spawn((
            PointLight {
                color: Vec3::new(1.0, 0.5, 0.0),
                intensity: 2.0,
                range: 5.0,
            },
            Transform::from_translation(Vec3::new(0.0, 2.0, 0.0)),
        ));
        app.update();
    }
}
