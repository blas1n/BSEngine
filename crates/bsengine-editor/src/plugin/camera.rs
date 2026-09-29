//! The editor viewport camera.

use super::*;

pub(super) fn update_editor_camera(
    inspector: Option<ResMut<InspectorState>>,
    mouse: Option<bsengine_ecs::Res<bsengine_input::MouseState>>,
    buttons: Option<bsengine_ecs::Res<bsengine_input::Input<bsengine_input::MouseButton>>>,
) {
    let Some(mut insp) = inspector else { return };
    if !insp.editor_mode {
        return;
    }

    if let (Some(mouse), Some(buttons)) = (mouse, buttons) {
        if insp.viewport_contains_cursor {
            let dx = mouse.delta.0 as f32;
            let dy = mouse.delta.1 as f32;
            let scroll = mouse.scroll_delta as f32;

            if buttons.is_pressed(&bsengine_input::MouseButton::Right) {
                insp.cam_yaw -= dx * 0.005;
                insp.cam_pitch = (insp.cam_pitch - dy * 0.005).clamp(-1.5, 1.5);
            }

            if buttons.is_pressed(&bsengine_input::MouseButton::Middle) {
                let right = glam::Vec3::new(insp.cam_yaw.sin(), 0.0, -insp.cam_yaw.cos());
                let speed = 0.01 * insp.cam_distance;
                let target = glam::Vec3::from(insp.cam_target) - right * dx * speed
                    + glam::Vec3::Y * dy * speed;
                insp.cam_target = target.to_array();
            }

            if scroll != 0.0 {
                insp.cam_distance = (insp.cam_distance - scroll * insp.cam_distance * 0.1).max(0.5);
            }
        }
    }

    let aspect = if insp.viewport_size[1] > 0.0 {
        insp.viewport_size[0] / insp.viewport_size[1]
    } else {
        16.0 / 9.0
    };
    let pitch = insp.cam_pitch;
    let yaw = insp.cam_yaw;
    let dist = insp.cam_distance;
    let target = glam::Vec3::from(insp.cam_target);
    let eye = target
        + glam::Vec3::new(
            dist * yaw.cos() * pitch.cos(),
            dist * pitch.sin(),
            dist * yaw.sin() * pitch.cos(),
        );
    // The Timeline panel's preview replaces the orbit camera's answer here
    // rather than writing `editor_view_proj` itself. This system assigns it
    // unconditionally every frame, so a second writer would simply race it --
    // and the orbit parameters above are deliberately left untouched, which is
    // what makes ending a preview return the user to exactly the viewpoint
    // they had instead of stranding the camera at the cutscene.
    let preview_camera = insp
        .timeline_preview
        .as_ref()
        .and_then(|p| p.camera.clone());

    let (eye, view, proj) = match preview_camera {
        Some(camera) => {
            let eye = glam::Vec3::from(camera.position);
            let rotation = glam::Quat::from_array(camera.rotation);
            let fov = camera
                .fov_y_degrees
                .map_or(std::f32::consts::FRAC_PI_4, |d| d.to_radians());
            // The engine's cameras look down -Z, the convention
            // `CameraPose::rotation` builds against.
            let forward = rotation * glam::Vec3::NEG_Z;
            let up = rotation * glam::Vec3::Y;
            (
                eye,
                glam::Mat4::look_at_rh(eye, eye + forward, up),
                glam::Mat4::perspective_rh(fov, aspect, 0.1, 1000.0),
            )
        }
        None => (
            eye,
            glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y),
            glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect, 0.1, 1000.0),
        ),
    };

    insp.editor_view_proj = Some((proj * view).to_cols_array_2d());
    insp.editor_proj = proj.to_cols_array_2d();
    insp.editor_cam_pos = eye.to_array();
}
