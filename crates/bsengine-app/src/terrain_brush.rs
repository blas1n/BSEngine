//! Terrain brush tool: picking (screen -> world ray -> terrain surface
//! point) and applying height/paint edits. All the "real work" lives here
//! rather than in the editor UI crate (`bsengine-rhi-wgpu`) because this is
//! the only crate with `PhysicsWorld`, `GpuMeshRegistry`, `GpuTextureRegistry`,
//! and `InspectorState` all in reach at once -- the same reason
//! `generate_terrain_chunks` lives in this crate rather than in
//! `bsengine-editor` or `bsengine-rhi-wgpu`.

use bevy_app::{App, Plugin, Update};
use bsengine_core::InspectorState;
use bsengine_ecs::{Query, Res, ResMut};
use bsengine_input::MouseState;
use bsengine_physics::PhysicsWorld;
use glam::{Mat4, Vec3};

/// Unprojects a screen-space point into a world-space ray, given the
/// camera's combined view-projection matrix and its world position.
/// Standard technique: unproject the near and far NDC points through the
/// inverse view-projection matrix, then the ray direction is far - near.
pub fn screen_to_world_ray(
    view_proj: Mat4,
    cam_pos: Vec3,
    screen_pos: (f32, f32),
    viewport_pos: (f32, f32),
    viewport_size: (f32, f32),
) -> (Vec3, Vec3) {
    let ndc_x = ((screen_pos.0 - viewport_pos.0) / viewport_size.0) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((screen_pos.1 - viewport_pos.1) / viewport_size.1) * 2.0;
    let inv_vp = view_proj.inverse();
    let near = inv_vp * glam::Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far = inv_vp * glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    let near_world = near.truncate() / near.w;
    let far_world = far.truncate() / far.w;
    let dir = (far_world - near_world).normalize();
    (cam_pos, dir)
}

/// Every frame, if the editor is in terrain-brush mode and the cursor is
/// over the viewport, raycasts from the camera through the cursor and
/// writes the hit (if any, and if it landed on a `TerrainChunkOf` entity)
/// into `InspectorState::terrain_pick`.
///
/// The raw cursor position comes from `bsengine_input::MouseState` (the
/// same resource `bsengine-render`'s `render_frame` reads for its own
/// cursor position, via `ms.position`) rather than from any field on
/// `InspectorState`, which does not track raw screen coordinates.
fn pick_terrain_under_cursor(
    physics: Res<PhysicsWorld>,
    chunk_query: Query<&crate::terrain::TerrainChunkOf>,
    mut inspector: Option<ResMut<InspectorState>>,
    mouse_state: Option<Res<MouseState>>,
) {
    let Some(insp) = inspector.as_mut() else {
        return;
    };
    if !insp.terrain_brush_active || !insp.viewport_contains_cursor {
        insp.terrain_pick = None;
        return;
    }
    let Some(view_proj) = insp.editor_view_proj else {
        insp.terrain_pick = None;
        return;
    };
    let Some(mouse_state) = mouse_state.as_deref() else {
        insp.terrain_pick = None;
        return;
    };
    let cam_pos = Vec3::from(insp.editor_cam_pos);
    let cursor = (
        mouse_state.position.0 as f32,
        mouse_state.position.1 as f32,
    );
    let (origin, dir) = screen_to_world_ray(
        Mat4::from_cols_array_2d(&view_proj),
        cam_pos,
        cursor,
        (insp.viewport_pos[0], insp.viewport_pos[1]),
        (insp.viewport_size[0], insp.viewport_size[1]),
    );
    let hit = physics.cast_ray(origin, dir, 10_000.0);
    insp.terrain_pick = hit.and_then(|h| {
        let chunk_entity = h.entity?;
        let owner = chunk_query.get(chunk_entity).ok()?;
        Some((owner.0.index() as u64, h.point.to_array()))
    });
}

/// Bevy plugin that runs the terrain brush's picking system each frame.
pub struct TerrainBrushPlugin;

impl Plugin for TerrainBrushPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, pick_terrain_under_cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_center_unprojects_to_the_camera_forward_direction() {
        let cam_pos = Vec3::new(0.0, 5.0, 10.0);
        let proj = Mat4::perspective_rh(60f32.to_radians(), 16.0 / 9.0, 0.1, 1000.0);
        let view = Mat4::look_at_rh(cam_pos, Vec3::ZERO, Vec3::Y);
        let view_proj = proj * view;

        let (origin, dir) =
            screen_to_world_ray(view_proj, cam_pos, (640.0, 360.0), (0.0, 0.0), (1280.0, 720.0));

        assert!((origin - cam_pos).length() < 1e-4);
        let expected_dir = (Vec3::ZERO - cam_pos).normalize();
        assert!(
            dir.dot(expected_dir) > 0.999,
            "screen center should unproject close to the camera-forward direction, \
             got dir={dir:?}, expected~={expected_dir:?}"
        );
    }

    #[test]
    fn a_ray_through_a_known_point_passes_near_it() {
        // Camera looking straight down at the origin from above; the world
        // point (2, 0, 0) should unproject from wherever its own screen
        // projection is.
        let cam_pos = Vec3::new(0.0, 10.0, 0.0);
        let proj = Mat4::perspective_rh(60f32.to_radians(), 1.0, 0.1, 1000.0);
        let view = Mat4::look_at_rh(cam_pos, Vec3::ZERO, Vec3::NEG_Z);
        let view_proj = proj * view;

        let target = Vec3::new(2.0, 0.0, 0.0);
        let clip = view_proj * target.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        let screen_x = (ndc.x * 0.5 + 0.5) * 800.0;
        let screen_y = (1.0 - (ndc.y * 0.5 + 0.5)) * 800.0;

        let (origin, dir) = screen_to_world_ray(
            view_proj,
            cam_pos,
            (screen_x, screen_y),
            (0.0, 0.0),
            (800.0, 800.0),
        );

        // Distance from `target` to the infinite ray (origin, dir).
        let to_target = target - origin;
        let t = to_target.dot(dir);
        let closest = origin + dir * t;
        let dist = (closest - target).length();
        assert!(
            dist < 0.05,
            "ray through target's own screen projection should pass within 5cm of it, got {dist}"
        );
    }

    #[test]
    fn terrain_brush_plugin_can_be_added_to_app() {
        let mut app = crate::new_app();
        app.add_plugins(bsengine_physics::PhysicsPlugin);
        app.add_plugins(bsengine_input::InputPlugin);
        app.insert_resource(InspectorState::default());
        app.add_plugins(TerrainBrushPlugin);
        app.update();
    }

    #[test]
    fn inactive_brush_leaves_terrain_pick_none() {
        let mut app = crate::new_app();
        app.add_plugins(bsengine_physics::PhysicsPlugin);
        app.add_plugins(bsengine_input::InputPlugin);
        // `terrain_brush_active` defaults to false; the picking system must
        // leave `terrain_pick` untouched (still `None`) rather than raycast.
        app.insert_resource(InspectorState::default());
        app.add_plugins(TerrainBrushPlugin);
        app.update();

        let insp = app.world().resource::<InspectorState>();
        assert_eq!(insp.terrain_pick, None);
    }

    #[test]
    fn active_brush_without_a_view_proj_leaves_terrain_pick_none() {
        let mut app = crate::new_app();
        app.add_plugins(bsengine_physics::PhysicsPlugin);
        app.add_plugins(bsengine_input::InputPlugin);
        // Active and hovered, but no camera matrix yet (e.g. before the
        // editor viewport has rendered a first frame) -- must not panic and
        // must leave the pick cleared rather than raycasting with stale data.
        // Built via mutation, not struct-literal update syntax: InspectorState
        // has private fields (e.g. `prev_selected_id`), so only this crate's
        // own `Default` impl can construct one at all.
        let mut inspector = InspectorState::default();
        inspector.terrain_brush_active = true;
        inspector.viewport_contains_cursor = true;
        inspector.editor_view_proj = None;
        app.insert_resource(inspector);
        app.add_plugins(TerrainBrushPlugin);
        app.update();

        let insp = app.world().resource::<InspectorState>();
        assert_eq!(insp.terrain_pick, None);
    }
}
