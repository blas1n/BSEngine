use bevy_app::{App, Plugin, PostUpdate, Update};
use bevy_ecs::prelude::{Entity, EventReader, IntoSystemConfigs, ParamSet, Query, ResMut, Without};
use bsengine_core::{
    AmbientOcclusion, Bloom, Camera, CustomShader, DirectionalLight, EditorPanelRegistry,
    EditorPlayState, GlobalTransform, HudTexts, InspectorState, Material, Parent, PointLight,
    SkyboxPath, SpotLight, Time, ToneMap, Transform, UiState, Visible,
};
use bsengine_ecs::Res;
use bsengine_input::{Input, KeyCode, KeyInput, MouseButton, MouseState};
use bsengine_rhi_wgpu::{
    GpuMeshRegistry, GpuTextureRegistry, LightData, MaterialParams, PointLightEntry,
    SpotLightEntry, WgpuSurfaceResource,
};
use bsengine_window::WindowResized;
use glam::{Mat4, Vec3, Vec4};
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

fn spot_light_entry(sl: &SpotLight, gt: Option<&GlobalTransform>, t: &Transform) -> SpotLightEntry {
    let pos = gt
        .map(|g| g.to_matrix().w_axis.truncate())
        .unwrap_or(t.translation.0);
    let dir = gt
        .map(|g| -glam::Mat3::from_mat4(g.to_matrix()).z_axis)
        .unwrap_or_else(|| t.rotation.0 * Vec3::NEG_Z);
    SpotLightEntry {
        position: pos,
        direction: dir,
        color: *sl.color,
        intensity: sl.intensity,
        range: sl.range,
        inner_angle: sl.inner_angle_degrees.to_radians(),
        outer_angle: sl.outer_angle_degrees.to_radians(),
    }
}

/// Pass 1: root entities (no Parent) get GlobalTransform = local Transform.
fn propagate_roots(mut query: Query<(&Transform, &mut GlobalTransform), Without<Parent>>) {
    for (t, mut gt) in query.iter_mut() {
        gt.0 = t.to_matrix().into();
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
    let parent_mats: HashMap<Entity, Mat4> = set.p0().iter().map(|(e, gt)| (e, gt.0 .0)).collect();

    for (t, mut gt, parent) in set.p1().iter_mut() {
        if let Some(&mat) = parent_mats.get(&parent.0) {
            gt.0 = (mat * t.to_matrix()).into();
        }
    }
}

/// Lazy-compiles any `CustomShader` not yet cached in the surface, reading
/// its WGSL source through `bsengine_asset::load` (`LoadMode::Sync`) and
/// handing the text to `compile_and_store_shader`. Split out of
/// `render_frame` (rather than folded in as two more top-level params)
/// because that function is already at Bevy 0.14's 16-top-level-param
/// `SystemParamFunction` ceiling — see the comment on `render_frame`'s
/// `render_queries` param. Registered in the same `PostUpdate` `.chain()`
/// as `render_frame` (see `RenderPlugin::build`), immediately before it, so
/// compiled shaders are available the same frame `render_frame` needs them
/// — an explicit, compiler-checked ordering constraint rather than relying
/// on this being a separate schedule that merely happens to run earlier.
///
/// Its `Query<&CustomShader>` is intentionally broader than the old inline
/// loop it replaces: it fires for *any* entity with a `CustomShader`
/// component, not just ones that also match `render_frame`'s mandatory
/// `&MeshRenderer, &Transform` query. Harmless (a shader that's never drawn
/// just sits compiled-and-unused in the surface's cache) and arguably an
/// improvement (a shader is ready the instant `MeshRenderer`/`Transform`
/// are added later, rather than one frame behind).
fn compile_pending_shaders(
    surface: Option<ResMut<WgpuSurfaceResource>>,
    custom_shaders: Query<&CustomShader>,
    mut shader_assets: bevy_ecs::prelude::ResMut<
        bevy_asset::Assets<crate::shader_asset::ShaderSource>,
    >,
    asset_server: bevy_ecs::prelude::Res<bevy_asset::AssetServer>,
) {
    let Some(mut surface) = surface else {
        return;
    };
    for cs in custom_shaders.iter() {
        if !surface.0.has_custom_shader(&cs.path) {
            match bsengine_asset::load(
                bsengine_asset::LoadMode::Sync,
                &asset_server,
                &mut shader_assets,
                &cs.path,
                crate::shader_asset::load_shader_source,
            ) {
                Ok(handle) => {
                    if let Some(src) = shader_assets.get(&handle) {
                        surface.0.compile_and_store_shader(&cs.path, &src.0);
                    }
                }
                Err(e) => tracing::warn!("[custom_shader] cannot read '{}': {e}", cs.path),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system params; splitting into a struct is a larger refactor
fn render_frame(
    surface: Option<ResMut<WgpuSurfaceResource>>,
    time: Option<Res<Time>>,
    registry: Option<Res<GpuMeshRegistry>>,
    tex_registry: Option<Res<GpuTextureRegistry>>,
    hud_texts: Option<Res<HudTexts>>,
    skybox_path: Option<Res<SkyboxPath>>,
    mut ui_state: Option<ResMut<UiState>>,
    mut inspector: Option<ResMut<InspectorState>>,
    mouse_state: Option<Res<MouseState>>,
    mouse_buttons: Option<Res<Input<MouseButton>>>,
    mut key_events: EventReader<KeyInput>,
    keys: Option<Res<Input<KeyCode>>>,
    // Bundled into one ParamSet: Bevy 0.14's SystemParamFunction impls only
    // go up to 16 top-level params, and adding editor_panels as a 17th
    // plain parameter broke `IntoSystem` resolution for this function
    // (surfaced as a `.chain()` trait-bound error in RenderPlugin::build).
    // Folding these five Querys into one param keeps the total at 13.
    mut render_queries: ParamSet<(
        Query<(
            &Camera,
            &Transform,
            Option<&Bloom>,
            Option<&ToneMap>,
            Option<&AmbientOcclusion>,
        )>,
        Query<(
            &MeshRenderer,
            &Transform,
            Option<&GlobalTransform>,
            Option<&Material>,
            Option<&Visible>,
            Option<&CustomShader>,
        )>,
        Query<(&DirectionalLight, Option<&GlobalTransform>, &Transform)>,
        Query<(&PointLight, Option<&GlobalTransform>, &Transform)>,
        Query<(&SpotLight, Option<&GlobalTransform>, &Transform)>,
    )>,
    editor_panels: Option<Res<EditorPanelRegistry>>,
    type_registry: Option<Res<bevy_ecs::reflect::AppTypeRegistry>>,
) {
    let (Some(mut surface), Some(registry)) = (surface, registry) else {
        return;
    };
    let empty = std::collections::HashMap::new();
    let hud_map = hud_texts.as_deref().map(|h| &h.0).unwrap_or(&empty);
    let empty_ui = UiState::default();
    let ui = ui_state.as_deref().unwrap_or(&empty_ui);
    let (cursor_x, cursor_y) = mouse_state
        .as_deref()
        .map(|ms| (ms.position.0 as f32, ms.position.1 as f32))
        .unwrap_or((0.0, 0.0));
    let left_just_pressed = mouse_buttons
        .as_deref()
        .map(|b| b.just_pressed(&MouseButton::Left))
        .unwrap_or(false);
    let left_just_released = mouse_buttons
        .as_deref()
        .map(|b| b.just_released(&MouseButton::Left))
        .unwrap_or(false);
    let key_events_this_frame: Vec<KeyInput> = key_events.read().cloned().collect();
    let ctrl_held = keys
        .as_deref()
        .map(|k| k.is_pressed(&KeyCode::ControlLeft) || k.is_pressed(&KeyCode::ControlRight))
        .unwrap_or(false);
    let shift_held = keys
        .as_deref()
        .map(|k| k.is_pressed(&KeyCode::ShiftLeft) || k.is_pressed(&KeyCode::ShiftRight))
        .unwrap_or(false);
    let alt_held = keys
        .as_deref()
        .map(|k| k.is_pressed(&KeyCode::AltLeft) || k.is_pressed(&KeyCode::AltRight))
        .unwrap_or(false);

    // Load or reload skybox when SkyboxPath changes
    if let Some(sp) = &skybox_path {
        let current = sp.0.as_deref();
        let loaded = surface.0.loaded_skybox_path();
        if current != loaded {
            match current {
                Some(p) => {
                    if let Err(e) = surface.0.set_skybox(p) {
                        tracing::warn!("skybox: {e}");
                    }
                }
                None => surface.0.clear_skybox(),
            }
        }
    }

    let (mut view_proj, mut cam_pos, mut cam_proj, bloom, tone_map, ambient_occlusion) =
        render_queries
            .p0()
            .iter()
            .next()
            .map(|(cam, t, b, tm, ao)| {
                let proj = cam.projection_matrix();
                (
                    proj * t.view_matrix(),
                    t.translation.0,
                    proj,
                    b.copied(),
                    tm.copied(),
                    ao.copied(),
                )
            })
            .unwrap_or((Mat4::IDENTITY, Vec3::ZERO, Mat4::IDENTITY, None, None, None));

    // While editing (not Playing), override camera matrices from the orbit
    // camera computed by EditorPlugin. Once Play starts, the viewport should
    // show what the game's own Camera entity sees, same as a build would.
    if let Some(insp) = inspector.as_deref() {
        if insp.editor_mode && insp.play_state == EditorPlayState::Stopped {
            if let Some(vp) = insp.editor_view_proj {
                view_proj = Mat4::from_cols_array_2d(&vp);
            }
            cam_pos = Vec3::from(insp.editor_cam_pos);
            cam_proj = Mat4::from_cols_array_2d(&insp.editor_proj);
        }
    }

    // Rotation-only VP inverse for skybox (no translation → direction-only)
    let sky_vp_inv: Option<Mat4> = if surface.0.has_skybox() {
        render_queries.p0().iter().next().map(|(cam, t, _, _, _)| {
            let proj = cam.projection_matrix();
            let view = t.view_matrix();
            let view_rot = Mat4::from_cols(view.x_axis, view.y_axis, view.z_axis, Vec4::W);
            (proj * view_rot).inverse()
        })
    } else {
        None
    };

    let draw_calls: Vec<(u64, Mat4, Option<u64>, MaterialParams, Option<String>)> = render_queries
        .p1()
        .iter()
        .filter_map(|(mr, t, gt, mat, vis, cs)| {
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
                    emissive: *m.emissive,
                    base_color: *m.base_color,
                })
                .unwrap_or_default();
            Some((
                mr.mesh_id,
                model,
                tex_id,
                mat_params,
                cs.map(|c| c.path.clone()),
            ))
        })
        .collect();

    let collected_point_lights: Vec<PointLightEntry> = render_queries
        .p3()
        .iter()
        .map(|(pl, gt, t)| {
            let pos = gt
                .map(|g| g.to_matrix().w_axis.truncate())
                .unwrap_or(t.translation.0);
            PointLightEntry {
                position: pos,
                color: *pl.color,
                intensity: pl.intensity,
                range: pl.range,
            }
        })
        .collect();

    let collected_spot_lights: Vec<SpotLightEntry> = render_queries
        .p4()
        .iter()
        .map(|(sl, gt, t)| spot_light_entry(sl, gt, t))
        .collect();

    let light = if let Some((l, gt, t)) = render_queries.p2().iter().next() {
        let direction = gt
            .map(|g| -glam::Mat3::from_mat4(g.to_matrix()).z_axis)
            .unwrap_or_else(|| t.rotation.0 * Vec3::NEG_Z);
        LightData {
            direction,
            color: *l.color,
            ambient: *l.ambient,
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

    match surface.0.render_frame(
        view_proj,
        cam_pos,
        light_view_proj,
        sky_vp_inv,
        &draw_calls,
        &registry,
        light,
        tex_reg_ref,
        hud_map,
        ui,
        cursor_x,
        cursor_y,
        left_just_pressed,
        left_just_released,
        cam_proj,
        bloom,
        tone_map,
        ambient_occlusion,
        inspector.as_deref_mut(),
        &key_events_this_frame,
        ctrl_held,
        shift_held,
        alt_held,
        editor_panels.as_deref(),
        type_registry.as_deref(),
        time.as_deref().map(|t| t.elapsed_seconds).unwrap_or(0.0),
    ) {
        Ok(clicked) => {
            if let Some(ref mut state) = ui_state {
                state.clicked = clicked;
            }
        }
        Err(e) => tracing::warn!("render_frame error: {e}"),
    }
}

fn update_camera_aspect(mut events: EventReader<WindowResized>, mut cameras: Query<&mut Camera>) {
    for ev in events.read() {
        for mut cam in cameras.iter_mut() {
            cam.update_aspect_ratio(ev.width, ev.height);
        }
    }
}

/// Bevy plugin that registers the render-related resources, events, and per-frame
/// systems (transform propagation, camera aspect updates, frame rendering).
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        use bevy_asset::AssetApp;
        app.init_asset::<crate::shader_asset::ShaderSource>()
            .register_asset_loader(crate::shader_asset::ShaderSourceLoader)
            .init_resource::<UiState>()
            .add_event::<WindowResized>()
            .add_event::<KeyInput>()
            .add_systems(Update, update_camera_aspect)
            .add_systems(
                PostUpdate,
                (
                    propagate_roots,
                    propagate_children,
                    compile_pending_shaders,
                    render_frame,
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::RenderPlugin;
    use crate::components::MeshRenderer;
    use bsengine_app::new_app;
    use bsengine_core::{Camera, Material, PointLight, Transform};
    use bsengine_rhi_wgpu::WgpuRHIPlugin;
    use bsengine_window::WindowResized;
    use glam::Vec3;

    #[test]
    fn render_plugin_runs_without_surface() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        app.update();
    }

    // Proves "compile_pending_shaders runs before render_frame, same frame"
    // structurally, via the real `PostUpdate` schedule's topological system
    // order — not via observing `WgpuSurfaceResource` state (has_custom_shader
    // / compile_and_store_shader), which would need a real GPU surface.
    // `WgpuSurface::new` requires a real `Arc<winit::window::Window>`, and
    // `WgpuRHIPlugin`'s surface-creation system only runs given a
    // `WindowHandle` resource, which is only ever produced by
    // `bsengine_window`'s real winit event loop (`App::run`, not `#[test]`);
    // no test anywhere in this workspace constructs a real
    // `WgpuSurfaceResource`, and CI runners have no display. So this test
    // verifies the same thing at the level this codebase can actually reach:
    // `Schedule::systems()`'s iteration order is the executor's genuine
    // topologically-sorted execution order (bevy_ecs's `ScheduleGraph`
    // builds it from `.chain()`'s dependency edges), so finding
    // `compile_pending_shaders` before `render_frame` in that order is a
    // real assertion about execution order, not a restatement of the code.
    #[test]
    fn compile_pending_shaders_runs_before_render_frame() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        // Schedules are only populated with their executable system list
        // after at least one run.
        app.update();

        let schedule = app
            .get_schedule(bevy_app::PostUpdate)
            .expect("RenderPlugin registers systems into PostUpdate");
        let names: Vec<String> = schedule
            .systems()
            .expect("schedule is initialized after app.update()")
            .map(|(_, system)| system.name().to_string())
            .collect();

        let compile_idx = names
            .iter()
            .position(|n| n.contains("compile_pending_shaders"))
            .unwrap_or_else(|| {
                panic!("compile_pending_shaders not found in PostUpdate: {names:?}")
            });
        let render_idx = names
            .iter()
            .position(|n| n.contains("render_frame"))
            .unwrap_or_else(|| panic!("render_frame not found in PostUpdate: {names:?}"));

        assert!(
            compile_idx < render_idx,
            "compile_pending_shaders (index {compile_idx}) must run before render_frame \
             (index {render_idx}) so shaders compiled this frame are available to it; \
             actual PostUpdate order: {names:?}"
        );
    }

    #[test]
    fn render_plugin_runs_with_rhi_headless() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        app.update();
        app.update();
        app.update();
    }

    #[test]
    fn camera_aspect_updates_on_window_resize() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(bsengine_asset::AssetPlugin);
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
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        app.world_mut().spawn((
            PointLight {
                color: Vec3::new(1.0, 0.5, 0.0).into(),
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
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        app.world_mut().spawn((
            MeshRenderer { mesh_id: 999 },
            Transform::from_translation(Vec3::ZERO),
            Material {
                metallic: 0.8,
                roughness: 0.2,
                emissive: Vec3::new(0.1, 0.0, 0.0).into(),
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
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        app.world_mut().spawn((
            SpotLight {
                color: Vec3::new(0.9, 0.9, 1.0).into(),
                intensity: 3.0,
                range: 12.0,
                ..Default::default()
            },
            Transform::from_translation(Vec3::new(0.0, 5.0, 0.0)),
        ));
        app.update();
    }

    #[test]
    fn spot_light_entry_converts_degrees_to_radians() {
        use bsengine_core::SpotLight;

        let sl = SpotLight {
            inner_angle_degrees: 45.0.into(),
            outer_angle_degrees: 60.0.into(),
            ..SpotLight::default()
        };
        let t = Transform::from_translation(Vec3::new(0.0, 5.0, 0.0));

        let entry = super::spot_light_entry(&sl, None, &t);

        assert!((entry.inner_angle - 45_f32.to_radians()).abs() < 1e-6);
        assert!((entry.outer_angle - 60_f32.to_radians()).abs() < 1e-6);
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
