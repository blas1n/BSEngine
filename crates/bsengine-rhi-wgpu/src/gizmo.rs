//! Screen-space translate gizmo: pure projection/hit-test math plus an egui
//! painter routine. Kept separate from `surface.rs` so the math is
//! unit-testable without a live wgpu/winit context.

use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};
use glam::{Mat4, Quat, Vec3};

pub const AXIS_X: u8 = 0;
pub const AXIS_Y: u8 = 1;
pub const AXIS_Z: u8 = 2;

const HANDLE_HIT_RADIUS: f32 = 8.0;
const HANDLE_LENGTH_FRACTION: f32 = 0.15;

// Camera entities don't carry their real aspect ratio in EntityInfo (only
// fov_y is tracked), so the frustum gizmo uses a fixed 16:9 stand-in — this
// is a visual indicator of position/orientation, not a pixel-accurate preview.
const FRUSTUM_VIZ_DISTANCE: f32 = 1.2;
const FRUSTUM_DEFAULT_ASPECT: f32 = 16.0 / 9.0;

pub fn axis_dir(axis: u8) -> Vec3 {
    match axis {
        AXIS_X => Vec3::X,
        AXIS_Y => Vec3::Y,
        _ => Vec3::Z,
    }
}

fn axis_color(axis: u8) -> Color32 {
    match axis {
        AXIS_X => Color32::from_rgb(220, 60, 60),
        AXIS_Y => Color32::from_rgb(60, 200, 80),
        _ => Color32::from_rgb(70, 120, 230),
    }
}

/// Projects a world-space point into screen-space pixel coordinates within
/// `rect`, using a combined view-projection matrix. Returns `None` if the
/// point is behind the camera (w <= 0), where projection is undefined.
pub fn world_to_screen(pos: Vec3, view_proj: &[[f32; 4]; 4], rect: Rect) -> Option<Pos2> {
    let m = Mat4::from_cols_array_2d(view_proj);
    let clip = m * pos.extend(1.0);
    if clip.w <= 1e-4 {
        return None;
    }
    let ndc_x = clip.x / clip.w;
    let ndc_y = clip.y / clip.w;
    let sx = rect.min.x + (ndc_x * 0.5 + 0.5) * rect.width();
    let sy = rect.min.y + (1.0 - (ndc_y * 0.5 + 0.5)) * rect.height();
    Some(Pos2::new(sx, sy))
}

/// Distance from `p` to the segment `a`-`b`, in screen pixels.
pub fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    let t = if len_sq > 1e-8 {
        ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let closest = a + ab * t;
    (p - closest).length()
}

/// World-space handle length for a gizmo centered at `pos`, scaled so its
/// on-screen size stays roughly constant regardless of camera distance.
pub fn handle_length(pos: Vec3, cam_pos: Vec3) -> f32 {
    (pos - cam_pos).length() * HANDLE_LENGTH_FRACTION
}

/// Screen-space unit direction and pixels-per-world-unit scale for one axis,
/// used to convert a 2D mouse drag delta into a 1D world-space offset along
/// that axis. Returns `None` if the axis is degenerate on screen (e.g.
/// pointing directly at/away from the camera).
pub fn axis_screen_dir_and_scale(
    pos: Vec3,
    axis: u8,
    probe_len: f32,
    view_proj: &[[f32; 4]; 4],
    rect: Rect,
) -> Option<(Vec2, f32)> {
    let origin = world_to_screen(pos, view_proj, rect)?;
    let tip = world_to_screen(pos + axis_dir(axis) * probe_len, view_proj, rect)?;
    let delta = tip - origin;
    let pixel_len = delta.length();
    if pixel_len < 1e-3 {
        return None;
    }
    Some((delta / pixel_len, pixel_len / probe_len))
}

/// Finds which axis handle (if any) is under `mouse_pos`, closest first.
pub fn hit_test(
    pos: Vec3,
    handle_len: f32,
    view_proj: &[[f32; 4]; 4],
    rect: Rect,
    mouse_pos: Pos2,
) -> Option<u8> {
    let origin = world_to_screen(pos, view_proj, rect)?;
    [AXIS_X, AXIS_Y, AXIS_Z]
        .into_iter()
        .filter_map(|axis| {
            let tip = world_to_screen(pos + axis_dir(axis) * handle_len, view_proj, rect)?;
            let d = dist_to_segment(mouse_pos, origin, tip);
            (d <= HANDLE_HIT_RADIUS).then_some((axis, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(axis, _)| axis)
}

/// Draws the three translate handles, highlighting `hovered`/`dragging`.
pub fn draw(
    painter: &Painter,
    pos: Vec3,
    handle_len: f32,
    view_proj: &[[f32; 4]; 4],
    rect: Rect,
    hovered: Option<u8>,
    dragging: Option<u8>,
) {
    let Some(origin) = world_to_screen(pos, view_proj, rect) else {
        return;
    };
    for axis in [AXIS_X, AXIS_Y, AXIS_Z] {
        let Some(tip) = world_to_screen(pos + axis_dir(axis) * handle_len, view_proj, rect) else {
            continue;
        };
        let active = dragging == Some(axis) || (dragging.is_none() && hovered == Some(axis));
        let color = if active {
            Color32::WHITE
        } else {
            axis_color(axis)
        };
        let width = if active { 4.0 } else { 2.5 };
        painter.line_segment([origin, tip], Stroke::new(width, color));
        painter.circle_filled(tip, 4.0, color);
    }
}

/// The 4 far-plane corners of a camera's frustum wireframe, in world space,
/// for a fixed small visualization distance (not the camera's actual near/far).
pub fn frustum_far_corners(pos: Vec3, rotation: Quat, fov_y_radians: f32) -> [Vec3; 4] {
    let forward = rotation * Vec3::NEG_Z;
    let up = rotation * Vec3::Y;
    let right = rotation * Vec3::X;

    let half_h = FRUSTUM_VIZ_DISTANCE * (fov_y_radians * 0.5).tan();
    let half_w = half_h * FRUSTUM_DEFAULT_ASPECT;
    let center = pos + forward * FRUSTUM_VIZ_DISTANCE;

    [
        center + up * half_h - right * half_w,
        center + up * half_h + right * half_w,
        center - up * half_h + right * half_w,
        center - up * half_h - right * half_w,
    ]
}

/// Draws a camera's frustum as a wireframe pyramid from `pos` (the apex) to
/// its four far-plane corners, so Camera entities are visible/orientable in
/// the editor viewport even though they render nothing themselves.
pub fn draw_camera_frustum(
    painter: &Painter,
    pos: Vec3,
    rotation: Quat,
    fov_y_radians: f32,
    view_proj: &[[f32; 4]; 4],
    rect: Rect,
    highlighted: bool,
) {
    let Some(apex) = world_to_screen(pos, view_proj, rect) else {
        return;
    };
    let corners = frustum_far_corners(pos, rotation, fov_y_radians);
    let screen: Vec<Option<Pos2>> = corners
        .iter()
        .map(|&c| world_to_screen(c, view_proj, rect))
        .collect();

    let color = if highlighted {
        Color32::WHITE
    } else {
        Color32::from_rgb(230, 200, 90)
    };
    let stroke = Stroke::new(1.5, color);

    for sc in screen.iter().flatten() {
        painter.line_segment([apex, *sc], stroke);
    }
    for i in 0..4 {
        if let (Some(a), Some(b)) = (screen[i], screen[(i + 1) % 4]) {
            painter.line_segment([a, b], stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_view_proj() -> [[f32; 4]; 4] {
        let eye = Vec3::new(0.0, 0.0, 10.0);
        let view = Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 800.0 / 600.0, 0.1, 1000.0);
        (proj * view).to_cols_array_2d()
    }

    fn test_rect() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))
    }

    #[test]
    fn origin_projects_near_screen_center() {
        let vp = test_view_proj();
        let screen = world_to_screen(Vec3::ZERO, &vp, test_rect()).expect("in front of camera");
        assert!((screen.x - 400.0).abs() < 1.0);
        assert!((screen.y - 300.0).abs() < 1.0);
    }

    #[test]
    fn point_behind_camera_returns_none() {
        let vp = test_view_proj();
        // Camera sits at z=10 looking toward -Z; z=20 is behind it.
        let screen = world_to_screen(Vec3::new(0.0, 0.0, 20.0), &vp, test_rect());
        assert!(screen.is_none());
    }

    #[test]
    fn x_axis_moves_screen_point_horizontally() {
        let vp = test_view_proj();
        let rect = test_rect();
        let origin = world_to_screen(Vec3::ZERO, &vp, rect).unwrap();
        let tip = world_to_screen(Vec3::new(2.0, 0.0, 0.0), &vp, rect).unwrap();
        assert!((tip.x - origin.x).abs() > 5.0);
        assert!((tip.y - origin.y).abs() < 1.0);
    }

    #[test]
    fn y_axis_moves_screen_point_vertically() {
        let vp = test_view_proj();
        let rect = test_rect();
        let origin = world_to_screen(Vec3::ZERO, &vp, rect).unwrap();
        let tip = world_to_screen(Vec3::new(0.0, 2.0, 0.0), &vp, rect).unwrap();
        assert!((tip.y - origin.y).abs() > 5.0);
        assert!((tip.x - origin.x).abs() < 1.0);
    }

    #[test]
    fn dist_to_segment_endpoint_and_perpendicular() {
        let a = Pos2::new(0.0, 0.0);
        let b = Pos2::new(10.0, 0.0);
        assert!((dist_to_segment(Pos2::new(5.0, 3.0), a, b) - 3.0).abs() < 1e-4);
        assert!((dist_to_segment(Pos2::new(-2.0, 0.0), a, b) - 2.0).abs() < 1e-4);
        assert!((dist_to_segment(Pos2::new(0.0, 0.0), a, b)).abs() < 1e-4);
    }

    #[test]
    fn handle_length_scales_with_camera_distance() {
        let near = handle_length(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0));
        let far = handle_length(Vec3::ZERO, Vec3::new(0.0, 0.0, 50.0));
        assert!(far > near);
    }

    #[test]
    fn axis_screen_dir_and_scale_is_unit_and_positive() {
        let vp = test_view_proj();
        let rect = test_rect();
        let (dir, scale) =
            axis_screen_dir_and_scale(Vec3::ZERO, AXIS_X, 1.0, &vp, rect).expect("not degenerate");
        assert!((dir.length() - 1.0).abs() < 1e-3);
        assert!(scale > 0.0);
    }

    #[test]
    fn hit_test_finds_closest_handle_tip() {
        let vp = test_view_proj();
        let rect = test_rect();
        let handle_len = 2.0;
        let x_tip = world_to_screen(Vec3::new(handle_len, 0.0, 0.0), &vp, rect).unwrap();
        assert_eq!(
            hit_test(Vec3::ZERO, handle_len, &vp, rect, x_tip),
            Some(AXIS_X)
        );
    }

    #[test]
    fn hit_test_misses_far_away_point() {
        let vp = test_view_proj();
        let rect = test_rect();
        assert_eq!(
            hit_test(Vec3::ZERO, 2.0, &vp, rect, Pos2::new(10.0, 10.0)),
            None
        );
    }

    #[test]
    fn frustum_far_corners_are_in_front_of_identity_rotation() {
        let corners = frustum_far_corners(Vec3::ZERO, Quat::IDENTITY, std::f32::consts::FRAC_PI_4);
        for c in corners {
            // Identity rotation looks down -Z, so all corners should be
            // behind the origin along -Z.
            assert!(c.z < 0.0, "expected corner {:?} to have negative z", c);
        }
    }

    #[test]
    fn frustum_far_corners_are_symmetric_around_center() {
        let corners = frustum_far_corners(Vec3::ZERO, Quat::IDENTITY, std::f32::consts::FRAC_PI_4);
        let center: Vec3 = corners.iter().copied().fold(Vec3::ZERO, |a, b| a + b) / 4.0;
        assert!(
            center.x.abs() < 1e-4,
            "center.x should be ~0, got {}",
            center.x
        );
        assert!(
            center.y.abs() < 1e-4,
            "center.y should be ~0, got {}",
            center.y
        );
    }

    #[test]
    fn frustum_far_corners_scale_with_fov() {
        let narrow = frustum_far_corners(Vec3::ZERO, Quat::IDENTITY, 0.2);
        let wide = frustum_far_corners(Vec3::ZERO, Quat::IDENTITY, 1.5);
        let narrow_width = (narrow[1] - narrow[0]).length();
        let wide_width = (wide[1] - wide[0]).length();
        assert!(
            wide_width > narrow_width,
            "wider fov should produce a wider far plane"
        );
    }
}
