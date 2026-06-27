use bevy_ecs::prelude::Component;

/// Rate-limited heading tracker that wraps to [0.0, 360.0). Models a
/// character facing direction, ship bow, tank turret, or camera yaw.
///
/// `rotate(delta_deg)` shifts `heading` by `delta_deg` degrees (positive =
/// clockwise), wraps the result to [0.0, 360.0), and sets `just_rotated`.
/// No-op when disabled or `delta_deg == 0.0`.
///
/// `tick(_dt)` clears `just_rotated` only. No time-based logic.
///
/// `face(target_deg)` snaps `heading` to `target_deg` normalized to
/// [0.0, 360.0). No-op when disabled.
///
/// `angular_distance(target_deg)` returns the signed shortest-path angle
/// from `heading` to `target_deg` in [-180.0, 180.0). Positive = clockwise.
///
/// `is_facing(target_deg, tolerance_deg)` returns `true` when
/// `|angular_distance(target_deg)| <= tolerance_deg && enabled`.
///
/// `effective_turn_speed()` returns `turn_rate` when enabled; `0.0` when
/// disabled.
///
/// Default: `new(90.0)` — 90 degrees/second turn capability.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yaw {
    /// Current heading in degrees, always in [0.0, 360.0).
    pub heading: f32,
    /// Maximum turn speed in degrees/second. Clamped >= 0.0.
    pub turn_rate: f32,
    pub just_rotated: bool,
    pub enabled: bool,
}

impl Yaw {
    pub fn new(turn_rate: f32) -> Self {
        Self {
            heading: 0.0,
            turn_rate: turn_rate.max(0.0),
            just_rotated: false,
            enabled: true,
        }
    }

    fn wrap(deg: f32) -> f32 {
        ((deg % 360.0) + 360.0) % 360.0
    }

    /// Rotate heading by `delta_deg`. Wraps result to [0.0, 360.0).
    /// No-op when disabled or delta is zero.
    pub fn rotate(&mut self, delta_deg: f32) {
        if !self.enabled || delta_deg == 0.0 {
            return;
        }
        self.heading = Self::wrap(self.heading + delta_deg);
        self.just_rotated = true;
    }

    /// Advance one frame: clear `just_rotated`. No time-based logic.
    pub fn tick(&mut self, _dt: f32) {
        self.just_rotated = false;
    }

    /// Snap heading to `target_deg` (normalized to [0.0, 360.0)). No-op when
    /// disabled.
    pub fn face(&mut self, target_deg: f32) {
        if !self.enabled {
            return;
        }
        self.heading = Self::wrap(target_deg);
    }

    /// Signed shortest-path angle from current heading to `target_deg` in
    /// [-180.0, 180.0). Positive = clockwise.
    pub fn angular_distance(&self, target_deg: f32) -> f32 {
        let delta = Self::wrap(target_deg) - self.heading;
        if delta >= 180.0 {
            delta - 360.0
        } else if delta < -180.0 {
            delta + 360.0
        } else {
            delta
        }
    }

    /// `true` when within `tolerance_deg` of `target_deg` and enabled.
    pub fn is_facing(&self, target_deg: f32, tolerance_deg: f32) -> bool {
        self.enabled && self.angular_distance(target_deg).abs() <= tolerance_deg
    }

    /// Returns `turn_rate` when enabled; `0.0` when disabled.
    pub fn effective_turn_speed(&self) -> f32 {
        if self.enabled {
            self.turn_rate
        } else {
            0.0
        }
    }
}

impl Default for Yaw {
    fn default() -> Self {
        Self::new(90.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yaw {
        Yaw::new(90.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_at_zero_heading() {
        let y = y();
        assert_eq!(y.heading, 0.0);
        assert!(!y.just_rotated);
        assert!(y.enabled);
    }

    #[test]
    fn turn_rate_clamped_to_zero() {
        let y = Yaw::new(-45.0);
        assert_eq!(y.turn_rate, 0.0);
    }

    // --- rotate ---

    #[test]
    fn rotate_clockwise() {
        let mut y = y();
        y.rotate(90.0);
        assert!((y.heading - 90.0).abs() < 1e-4);
    }

    #[test]
    fn rotate_counter_clockwise() {
        let mut y = y();
        y.rotate(-90.0);
        assert!((y.heading - 270.0).abs() < 1e-4);
    }

    #[test]
    fn rotate_wraps_past_360() {
        let mut y = y();
        y.rotate(370.0);
        assert!((y.heading - 10.0).abs() < 1e-4);
    }

    #[test]
    fn rotate_wraps_below_zero() {
        let mut y = y();
        y.rotate(-10.0);
        assert!((y.heading - 350.0).abs() < 1e-4);
    }

    #[test]
    fn rotate_sets_just_rotated() {
        let mut y = y();
        y.rotate(45.0);
        assert!(y.just_rotated);
    }

    #[test]
    fn rotate_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.rotate(45.0);
        assert_eq!(y.heading, 0.0);
        assert!(!y.just_rotated);
    }

    #[test]
    fn rotate_no_op_when_delta_zero() {
        let mut y = y();
        y.rotate(0.0);
        assert_eq!(y.heading, 0.0);
        assert!(!y.just_rotated);
    }

    #[test]
    fn rotate_accumulates() {
        let mut y = y();
        y.rotate(90.0);
        y.rotate(90.0);
        assert!((y.heading - 180.0).abs() < 1e-4);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_rotated() {
        let mut y = y();
        y.rotate(45.0);
        y.tick(0.016);
        assert!(!y.just_rotated);
    }

    #[test]
    fn tick_does_not_change_heading() {
        let mut y = y();
        y.rotate(45.0);
        y.tick(1.0);
        assert!((y.heading - 45.0).abs() < 1e-4);
    }

    // --- face ---

    #[test]
    fn face_snaps_to_target() {
        let mut y = y();
        y.face(270.0);
        assert!((y.heading - 270.0).abs() < 1e-4);
    }

    #[test]
    fn face_normalizes_target() {
        let mut y = y();
        y.face(450.0); // 450 → 90
        assert!((y.heading - 90.0).abs() < 1e-4);
    }

    #[test]
    fn face_no_op_when_disabled() {
        let mut y = y();
        y.rotate(45.0);
        y.enabled = false;
        y.face(270.0);
        assert!((y.heading - 45.0).abs() < 1e-4);
    }

    // --- angular_distance ---

    #[test]
    fn angular_distance_clockwise() {
        let y = y(); // heading=0
        assert!((y.angular_distance(90.0) - 90.0).abs() < 1e-4);
    }

    #[test]
    fn angular_distance_counter_clockwise() {
        let y = y(); // heading=0
        assert!((y.angular_distance(270.0) - (-90.0)).abs() < 1e-4);
    }

    #[test]
    fn angular_distance_zero_at_same() {
        let y = y();
        assert!((y.angular_distance(0.0)).abs() < 1e-4);
    }

    #[test]
    fn angular_distance_exactly_180() {
        let y = y(); // heading=0
        let d = y.angular_distance(180.0);
        assert!((d.abs() - 180.0).abs() < 1e-4);
    }

    #[test]
    fn angular_distance_wraps_target() {
        let y = y(); // heading=0
        assert!((y.angular_distance(360.0)).abs() < 1e-4);
    }

    // --- is_facing ---

    #[test]
    fn is_facing_true_within_tolerance() {
        let mut y = y();
        y.face(90.0);
        assert!(y.is_facing(92.0, 5.0));
    }

    #[test]
    fn is_facing_false_outside_tolerance() {
        let mut y = y();
        y.face(90.0);
        assert!(!y.is_facing(100.0, 5.0));
    }

    #[test]
    fn is_facing_false_when_disabled() {
        let mut y = y();
        y.face(90.0);
        y.enabled = false;
        assert!(!y.is_facing(90.0, 1.0));
    }

    // --- effective_turn_speed ---

    #[test]
    fn effective_turn_speed_when_enabled() {
        let y = y();
        assert!((y.effective_turn_speed() - 90.0).abs() < 1e-4);
    }

    #[test]
    fn effective_turn_speed_zero_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert_eq!(y.effective_turn_speed(), 0.0);
    }

    // --- full rotation cycle ---

    #[test]
    fn full_rotation_returns_to_origin() {
        let mut y = y();
        y.rotate(360.0);
        assert!(y.heading < 1e-4);
    }
}
