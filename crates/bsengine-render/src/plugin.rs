use bevy_app::{App, Plugin, PostUpdate, Update};
use bevy_ecs::prelude::{Entity, EventReader, IntoSystemConfigs, ParamSet, Query, ResMut, Without};
use bsengine_core::{
    Camera, DirectionalLight, GlobalTransform, HudTexts, Material, Parent, PointLight, SpotLight,
    Transform, Visible,
};
use bsengine_ecs::Res;
use bsengine_rhi_wgpu::{
    GpuMeshRegistry, GpuTextureRegistry, LightData, MaterialParams, PointLightEntry,
    SpotLightEntry, WgpuSurfaceResource,
};
use bsengine_window::WindowResized;
use glam::{Mat4, Vec3};
use std::collections::HashMap;

use crate::components::MeshRenderer;

/// Returns false if the sphere is completely outside the view frustum.
/// Uses Gribb-Hartmann plane extraction from the view-projection matrix
/// (assumes perspective_rh / −1..1 clip depth convention).
fn sphere_visible_in_frustum(view_proj: Mat4, world_center: Vec3, world_radius: f32) -> bool {
    let r0 = view_proj.row(0);
    let r1 = view_proj.row(1);
    let r2 = view_proj.row(2);
    let r3 = view_proj.row(3);
    let planes = [
        r3 + r0, // left
        r3 - r0, // right
        r3 + r1, // bottom
        r3 - r1, // top
        r3 + r2, // near  (perspective_rh: near maps to −1)
        r3 - r2, // far
    ];
    let p = world_center.extend(1.0);
    for plane in &planes {
        if plane.dot(p) < -world_radius * plane.truncate().length() {
            return false;
        }
    }
    true
}

/// Computes an orthographic view-projection from the light's direction for shadow mapping.
/// Uses rh_zo (0..1 depth) to match wgpu's depth buffer convention.
fn compute_light_view_proj(light_dir: Vec3) -> Mat4 {
    let dir = light_dir.normalize();
    let up = if dir.y.abs() < 0.999 {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let eye = -dir * 50.0;
    let view = Mat4::look_at_rh(eye, Vec3::ZERO, up);
    let proj = Mat4::orthographic_rh(-30.0, 30.0, -30.0, 30.0, 0.1, 200.0);
    proj * view
}

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
    surface: Option<ResMut<WgpuSurfaceResource>>,
    registry: Option<Res<GpuMeshRegistry>>,
    tex_registry: Option<Res<GpuTextureRegistry>>,
    hud_texts: Option<Res<HudTexts>>,
    camera_query: Query<(&Camera, &Transform)>,
    mesh_query: Query<(
        &MeshRenderer,
        &Transform,
        Option<&GlobalTransform>,
        Option<&Material>,
        Option<&Visible>,
    )>,
    light_query: Query<&DirectionalLight>,
    point_light_query: Query<(&PointLight, Option<&GlobalTransform>, &Transform)>,
    spot_light_query: Query<(&SpotLight, Option<&GlobalTransform>, &Transform)>,
) {
    let (Some(mut surface), Some(registry)) = (surface, registry) else {
        return;
    };
    let empty = std::collections::HashMap::new();
    let hud_map = hud_texts.as_deref().map(|h| &h.0).unwrap_or(&empty);

    let (view_proj, cam_pos) = camera_query
        .iter()
        .next()
        .map(|(cam, t)| (cam.projection_matrix() * t.view_matrix(), t.translation))
        .unwrap_or((Mat4::IDENTITY, Vec3::ZERO));

    let draw_calls: Vec<(u64, Mat4, Option<u64>, MaterialParams)> = mesh_query
        .iter()
        .filter_map(|(mr, t, gt, mat, vis)| {
            if !vis.map(|v| v.is_visible).unwrap_or(true) {
                return None;
            }
            let model = gt.map(|g| g.to_matrix()).unwrap_or_else(|| t.to_matrix());
            if let Some((local_center, local_radius)) = registry.get_bounds(mr.mesh_id) {
                let world_center = (model * local_center.extend(1.0)).truncate();
                let max_scale = model
                    .x_axis
                    .truncate()
                    .length()
                    .max(model.y_axis.truncate().length())
                    .max(model.z_axis.truncate().length());
                let world_radius = local_radius * max_scale.max(1.0);
                if !sphere_visible_in_frustum(view_proj, world_center, world_radius) {
                    return None;
                }
            }
            let tex_id = mat.and_then(|m| m.texture_id);
            let mat_params = mat
                .map(|m| MaterialParams {
                    metallic: m.metallic,
                    roughness: m.roughness,
                    emissive: m.emissive,
                    base_color: m.base_color,
                })
                .unwrap_or_default();
            Some((mr.mesh_id, model, tex_id, mat_params))
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

    let collected_spot_lights: Vec<SpotLightEntry> = spot_light_query
        .iter()
        .map(|(sl, gt, t)| {
            let pos = gt
                .map(|g| g.to_matrix().w_axis.truncate())
                .unwrap_or(t.translation);
            let dir = gt
                .map(|g| -glam::Mat3::from_mat4(g.to_matrix()).z_axis)
                .unwrap_or_else(|| t.rotation * Vec3::NEG_Z);
            SpotLightEntry {
                position: pos,
                direction: dir,
                color: sl.color,
                intensity: sl.intensity,
                range: sl.range,
                inner_angle: sl.inner_angle,
                outer_angle: sl.outer_angle,
            }
        })
        .collect();

    let light = if let Some(l) = light_query.iter().next() {
        LightData {
            direction: l.direction,
            color: l.color,
            ambient: l.ambient,
            point_lights: collected_point_lights,
            spot_lights: collected_spot_lights,
        }
    } else {
        LightData {
            point_lights: collected_point_lights,
            spot_lights: collected_spot_lights,
            ..Default::default()
        }
    };

    let light_view_proj = compute_light_view_proj(light.direction);
    let tex_reg_ref = tex_registry.as_deref();

    if let Err(e) = surface.0.render_frame(
        view_proj,
        cam_pos,
        light_view_proj,
        &draw_calls,
        &registry,
        light,
        tex_reg_ref,
        hud_map,
    ) {
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
    use crate::components::MeshRenderer;
    use bsengine_app::new_app;
    use bsengine_core::{Camera, Material, PointLight, SpotLight, Transform};
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

    #[test]
    fn render_plugin_uses_pbr_material() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(RenderPlugin);
        app.world_mut().spawn((
            MeshRenderer { mesh_id: 999 },
            Transform::from_translation(Vec3::ZERO),
            Material {
                metallic: 0.8,
                roughness: 0.2,
                emissive: Vec3::new(0.1, 0.0, 0.0),
                ..Default::default()
            },
        ));
        app.update();
    }

    #[test]
    fn render_plugin_accepts_spot_lights() {
        use bsengine_core::SpotLight;
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(RenderPlugin);
        app.world_mut().spawn((
            SpotLight {
                color: Vec3::new(0.9, 0.9, 1.0),
                intensity: 3.0,
                range: 12.0,
                ..Default::default()
            },
            Transform::from_translation(Vec3::new(0.0, 5.0, 0.0)),
        ));
        app.update();
    }

    #[test]
    fn light_view_proj_is_invertible() {
        use super::compute_light_view_proj;
        let dir = Vec3::new(-0.4, -0.8, -0.4).normalize();
        let vp = compute_light_view_proj(dir);
        assert!(
            vp.determinant().abs() > 1e-6,
            "light VP should be invertible"
        );
    }

    #[test]
    fn light_view_proj_up_axis_does_not_degenerate() {
        use super::compute_light_view_proj;
        // straight-down light — should pick Z as up without NaN/zero-det
        let vp = compute_light_view_proj(Vec3::new(0.0, -1.0, 0.0));
        assert!(vp.determinant().abs() > 1e-6);
    }

    #[test]
    fn frustum_cull_sphere_in_front_is_visible() {
        use super::sphere_visible_in_frustum;
        use glam::Mat4;
        let vp = Mat4::perspective_rh(std::f32::consts::FRAC_PI_3, 1.0, 0.1, 100.0);
        assert!(sphere_visible_in_frustum(
            vp,
            Vec3::new(0.0, 0.0, -5.0),
            0.5
        ));
    }

    #[test]
    fn frustum_cull_sphere_behind_camera_is_culled() {
        use super::sphere_visible_in_frustum;
        use glam::Mat4;
        let vp = Mat4::perspective_rh(std::f32::consts::FRAC_PI_3, 1.0, 0.1, 100.0);
        assert!(!sphere_visible_in_frustum(
            vp,
            Vec3::new(0.0, 0.0, 5.0),
            0.5
        ));
    }

    #[test]
    fn frustum_cull_sphere_past_far_plane_is_culled() {
        use super::sphere_visible_in_frustum;
        use glam::Mat4;
        let vp = Mat4::perspective_rh(std::f32::consts::FRAC_PI_3, 1.0, 0.1, 100.0);
        assert!(!sphere_visible_in_frustum(
            vp,
            Vec3::new(0.0, 0.0, -150.0),
            0.5
        ));
    }
}
