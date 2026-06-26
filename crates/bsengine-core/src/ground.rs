use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Ground-detection state written by the physics/character-controller system.
///
/// Query `Ground` to determine if an entity is standing on a surface and at
/// what angle, instead of duplicating this logic across movement, animation,
/// and ability systems.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ground {
    /// Whether the entity is currently touching ground.
    pub is_grounded: bool,
    /// Whether the entity was grounded on the previous frame (for edge detection).
    pub was_grounded: bool,
    /// Outward normal of the surface the entity is standing on.
    /// `Vec3::Y` when airborne (default up).
    pub normal: Vec3,
    /// Slope angle in radians relative to the world-up axis.
    /// 0 = flat, π/2 = vertical wall.
    pub slope_angle: f32,
    /// Maximum slope angle (radians) the entity can walk on without sliding.
    pub max_slope: f32,
    /// Distance from the entity's feet to the ground surface.
    /// 0 when grounded; may be positive when in the air.
    pub distance: f32,
    pub enabled: bool,
}

impl Ground {
    pub fn new() -> Self {
        Self {
            is_grounded: false,
            was_grounded: false,
            normal: Vec3::Y,
            slope_angle: 0.0,
            max_slope: std::f32::consts::FRAC_PI_4, // 45°
            distance: 0.0,
            enabled: true,
        }
    }

    pub fn with_max_slope_degrees(mut self, degrees: f32) -> Self {
        self.max_slope = degrees.to_radians().clamp(0.0, std::f32::consts::FRAC_PI_2);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Update grounding state. Call from the physics/controller system each frame.
    pub fn set_grounded(&mut self, normal: Vec3, distance: f32) {
        self.was_grounded = self.is_grounded;
        self.is_grounded = true;
        self.normal = normal.normalize_or(Vec3::Y);
        self.distance = distance.max(0.0);
        self.slope_angle = Vec3::Y.angle_between(self.normal);
    }

    /// Mark as airborne. Call when the cast finds no ground within range.
    pub fn set_airborne(&mut self, distance: f32) {
        self.was_grounded = self.is_grounded;
        self.is_grounded = false;
        self.normal = Vec3::Y;
        self.slope_angle = 0.0;
        self.distance = distance.max(0.0);
    }

    /// True on the first frame the entity leaves the ground.
    pub fn just_left_ground(&self) -> bool {
        self.was_grounded && !self.is_grounded
    }

    /// True on the first frame the entity lands.
    pub fn just_landed(&self) -> bool {
        !self.was_grounded && self.is_grounded
    }

    /// Whether the current slope exceeds `max_slope` (entity should slide).
    pub fn is_too_steep(&self) -> bool {
        self.is_grounded && self.slope_angle > self.max_slope
    }
}

impl Default for Ground {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_airborne() {
        let g = Ground::new();
        assert!(!g.is_grounded);
    }

    #[test]
    fn set_grounded_updates_state() {
        let mut g = Ground::new();
        g.set_grounded(Vec3::Y, 0.0);
        assert!(g.is_grounded);
        assert!((g.slope_angle).abs() < 0.001);
    }

    #[test]
    fn just_landed_fires_on_transition() {
        let mut g = Ground::new();
        assert!(!g.just_landed());
        g.set_grounded(Vec3::Y, 0.0);
        assert!(g.just_landed());
        g.set_grounded(Vec3::Y, 0.0);
        assert!(!g.just_landed());
    }

    #[test]
    fn just_left_ground_fires_on_transition() {
        let mut g = Ground::new();
        g.set_grounded(Vec3::Y, 0.0);
        g.set_airborne(1.5);
        assert!(g.just_left_ground());
    }

    #[test]
    fn steep_slope_detected() {
        let mut g = Ground::new().with_max_slope_degrees(30.0);
        // Normal tilted 60° from up = steep
        let steep = Vec3::new(0.866, 0.5, 0.0).normalize();
        g.set_grounded(steep, 0.0);
        assert!(g.is_too_steep());
    }
}
