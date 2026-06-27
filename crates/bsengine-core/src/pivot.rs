use bevy_ecs::prelude::Component;
use glam::{Quat, Vec3};

/// Controls how the pivot point is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PivotSpace {
    /// Offset is in the entity's local space.
    Local,
    /// Offset is in world space (fixed world-space anchor).
    World,
}

/// Pivot / hinge-point component — rotate an entity around a point that is
/// NOT its origin.
///
/// Used for: hinged doors (rotate around the hinge edge), crank handles,
/// planet orbits, spinning tops, or any object whose visual center differs
/// from its rotation center.
///
/// The physics / transform system reads `offset` and `space` to compute
/// the effective rotation: translate by `-offset`, rotate, translate back.
///
/// Optional auto-spin: set `angular_velocity` (radians/second around `axis`)
/// and call `tick(dt)` to accumulate `angle` automatically.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Pivot {
    /// Offset from the entity origin to the pivot point.
    pub offset: Vec3,
    pub space: PivotSpace,
    /// Axis of rotation (should be normalised).
    pub axis: Vec3,
    /// Current rotation angle around `axis` (radians).
    pub angle: f32,
    /// Auto-spin rate in radians per second (0 = no auto-spin).
    pub angular_velocity: f32,
    /// Optional angle limits [min, max] in radians (same as `None` when both are 0).
    pub min_angle: f32,
    pub max_angle: f32,
    /// Whether angle limits are enforced (false = free spin).
    pub clamped: bool,
    pub enabled: bool,
}

impl Pivot {
    /// Pivot offset in local space, rotating around `axis`.
    pub fn new(offset: Vec3, axis: Vec3) -> Self {
        Self {
            offset,
            space: PivotSpace::Local,
            axis: axis.normalize_or_zero(),
            angle: 0.0,
            angular_velocity: 0.0,
            min_angle: 0.0,
            max_angle: 0.0,
            clamped: false,
            enabled: true,
        }
    }

    /// Hinge that swings between `min_angle` and `max_angle`.
    pub fn hinge(offset: Vec3, axis: Vec3, min_angle: f32, max_angle: f32) -> Self {
        Self {
            offset,
            space: PivotSpace::Local,
            axis: axis.normalize_or_zero(),
            angle: min_angle,
            angular_velocity: 0.0,
            min_angle,
            max_angle,
            clamped: true,
            enabled: true,
        }
    }

    pub fn in_world_space(mut self) -> Self {
        self.space = PivotSpace::World;
        self
    }

    pub fn with_spin(mut self, radians_per_second: f32) -> Self {
        self.angular_velocity = radians_per_second;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Apply `delta` radians to `angle`, respecting clamp if enabled.
    pub fn rotate(&mut self, delta: f32) {
        if !self.enabled {
            return;
        }
        self.angle += delta;
        if self.clamped {
            self.angle = self.angle.clamp(self.min_angle, self.max_angle);
        }
    }

    /// Advance auto-spin by one frame. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        if self.enabled && self.angular_velocity != 0.0 {
            self.rotate(self.angular_velocity * dt);
        }
    }

    /// Compute the rotation quaternion for the current `angle` around `axis`.
    pub fn rotation(&self) -> Quat {
        Quat::from_axis_angle(self.axis, self.angle)
    }

    /// Fraction [0, 1] of travel within the clamped range (0 when unclamped).
    pub fn travel_fraction(&self) -> f32 {
        let range = self.max_angle - self.min_angle;
        if self.clamped && range > 0.0 {
            ((self.angle - self.min_angle) / range).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn is_at_min(&self) -> bool {
        self.clamped && self.angle <= self.min_angle
    }

    pub fn is_at_max(&self) -> bool {
        self.clamped && self.angle >= self.max_angle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn rotate_accumulates_angle() {
        let mut p = Pivot::new(Vec3::ZERO, Vec3::Y);
        p.rotate(FRAC_PI_2);
        p.rotate(FRAC_PI_2);
        assert!((p.angle - PI).abs() < 1e-5);
    }

    #[test]
    fn hinge_clamps_at_limits() {
        let mut p = Pivot::hinge(Vec3::ZERO, Vec3::Y, 0.0, FRAC_PI_2);
        p.rotate(-1.0);
        assert!((p.angle - 0.0).abs() < 1e-5);
        p.rotate(PI);
        assert!((p.angle - FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn travel_fraction_midpoint() {
        let mut p = Pivot::hinge(Vec3::ZERO, Vec3::Y, 0.0, PI);
        p.rotate(FRAC_PI_2);
        assert!((p.travel_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn auto_spin_tick_advances_angle() {
        let mut p = Pivot::new(Vec3::ZERO, Vec3::Y).with_spin(PI); // 180°/s
        p.tick(0.5);
        assert!((p.angle - FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn rotation_quat_is_identity_at_zero() {
        let p = Pivot::new(Vec3::ZERO, Vec3::Y);
        let q = p.rotation();
        assert!((q.dot(Quat::IDENTITY) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_ignores_rotate() {
        let mut p = Pivot::new(Vec3::ZERO, Vec3::Y).disabled();
        p.rotate(PI);
        assert_eq!(p.angle, 0.0);
    }
}
