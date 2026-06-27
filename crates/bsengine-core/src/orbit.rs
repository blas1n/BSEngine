use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Continuous circular-orbit motion around a center point.
///
/// Each frame the locomotion system reads `position(center)` to compute
/// the entity's world position. `tick(dt)` advances `angle` by `speed * dt`.
///
/// The orbit plane is defined by `axis` (the plane normal). With the default
/// `axis = Vec3::Y` the entity orbits horizontally. Any normalised axis is
/// valid (e.g. `Vec3::Z` for a vertical loop).
///
/// `altitude` adds a fixed offset along `axis`, placing the entity above or
/// below the orbit plane without affecting the radius.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Orbit {
    /// Orbit radius in world units.
    pub radius: f32,
    /// Angular speed in radians per second (positive = counter-clockwise when
    /// viewed from above with axis=Y).
    pub speed: f32,
    /// Current angle in radians.
    pub angle: f32,
    /// Orbit-plane normal. Must be normalised; default is `Vec3::Y`.
    pub axis: Vec3,
    /// Offset along `axis` (height above the plane).
    pub altitude: f32,
    pub enabled: bool,
}

impl Orbit {
    /// Horizontal orbit (axis = Y-up).
    pub fn new(radius: f32, speed: f32) -> Self {
        Self {
            radius: radius.max(0.0),
            speed,
            angle: 0.0,
            axis: Vec3::Y,
            altitude: 0.0,
            enabled: true,
        }
    }

    pub fn with_axis(mut self, axis: Vec3) -> Self {
        self.axis = axis.normalize_or_zero();
        self
    }

    pub fn with_altitude(mut self, altitude: f32) -> Self {
        self.altitude = altitude;
        self
    }

    pub fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance the orbit angle. Call once per frame before reading `position`.
    pub fn tick(&mut self, dt: f32) {
        if self.enabled {
            self.angle += self.speed * dt;
        }
    }

    /// Compute the orbiting entity's world position relative to `center`.
    ///
    /// Builds a right/forward tangent frame from `axis`, then evaluates
    /// the circle at the current `angle`.
    pub fn position(&self, center: Vec3) -> Vec3 {
        if !self.enabled {
            return center;
        }

        // Build an orthonormal basis on the orbit plane.
        let right = self.axis.any_orthonormal_vector();
        let fwd = self.axis.cross(right).normalize_or_zero();

        center
            + right * (self.angle.cos() * self.radius)
            + fwd * (self.angle.sin() * self.radius)
            + self.axis * self.altitude
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn tick_advances_angle() {
        let mut o = Orbit::new(1.0, 1.0);
        o.tick(PI);
        assert!((o.angle - PI).abs() < 1e-5);
    }

    #[test]
    fn position_at_zero_angle() {
        let o = Orbit::new(5.0, 0.0);
        let pos = o.position(Vec3::ZERO);
        // At angle=0: right * 5.0 + fwd * 0
        // right is orthogonal to Y; pos.y should equal altitude (0).
        assert!((pos.length() - 5.0).abs() < 1e-4);
        assert!(pos.y.abs() < 1e-4);
    }

    #[test]
    fn altitude_offsets_along_axis() {
        let o = Orbit::new(5.0, 0.0).with_altitude(3.0);
        let pos = o.position(Vec3::ZERO);
        assert!((pos.y - 3.0).abs() < 1e-4);
    }

    #[test]
    fn disabled_returns_center() {
        let o = Orbit::new(10.0, 1.0).disabled();
        assert_eq!(o.position(Vec3::splat(2.0)), Vec3::splat(2.0));
    }

    #[test]
    fn tick_disabled_does_not_advance() {
        let mut o = Orbit::new(1.0, 1.0).disabled();
        o.tick(1.0);
        assert_eq!(o.angle, 0.0);
    }

    #[test]
    fn full_revolution_returns_to_start() {
        let mut o = Orbit::new(3.0, 1.0).with_angle(0.0);
        let start = o.position(Vec3::ZERO);
        o.tick(2.0 * PI); // one full revolution
        let end = o.position(Vec3::ZERO);
        assert!(close(start, end));
    }

    #[test]
    fn quarter_turn_is_orthogonal() {
        let o1 = Orbit::new(1.0, 0.0).with_angle(0.0);
        let o2 = Orbit::new(1.0, 0.0).with_angle(FRAC_PI_2);
        let p1 = o1.position(Vec3::ZERO);
        let p2 = o2.position(Vec3::ZERO);
        // p1 and p2 should be perpendicular on the XZ plane
        assert!(p1.dot(p2).abs() < 1e-4);
    }
}
