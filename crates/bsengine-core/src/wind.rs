use bevy_ecs::prelude::Component;
use glam::Vec3;

/// A scene-wide wind force that influences foliage, cloth, and particle effects.
/// Attach to any entity (typically a singleton); the affected systems query for
/// the nearest `Wind` in range or the globally strongest one.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Wind {
    /// Primary wind direction and speed encoded as a single vector.
    /// Magnitude = speed in world units per second; direction = normalised.
    pub velocity: Vec3,
    /// Random gusting amplitude layered on top of the main velocity.
    /// 0 = steady; 1 = gust magnitude equals the main speed.
    pub turbulence: f32,
    /// How quickly turbulence changes (Hz — oscillations per second).
    pub turbulence_frequency: f32,
    /// Optional radius of influence. `0.0` = globally applied.
    pub radius: f32,
}

impl Wind {
    /// Create a directional wind with the given velocity vector.
    pub fn new(velocity: Vec3) -> Self {
        Self {
            velocity,
            turbulence: 0.0,
            turbulence_frequency: 1.0,
            radius: 0.0,
        }
    }

    /// Shorthand: create wind blowing along +X at `speed` units/sec.
    pub fn eastward(speed: f32) -> Self {
        Self::new(Vec3::X * speed.max(0.0))
    }

    pub fn with_turbulence(mut self, turbulence: f32) -> Self {
        self.turbulence = turbulence.max(0.0);
        self
    }

    pub fn with_turbulence_frequency(mut self, hz: f32) -> Self {
        self.turbulence_frequency = hz.max(0.0);
        self
    }

    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(0.0);
        self
    }

    /// Wind speed (magnitude of the velocity vector).
    pub fn speed(&self) -> f32 {
        self.velocity.length()
    }

    /// Returns `true` if this wind component applies globally (no radius limit).
    pub fn is_global(&self) -> bool {
        self.radius == 0.0
    }
}

impl Default for Wind {
    fn default() -> Self {
        Self::new(Vec3::X * 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wind_defaults() {
        let w = Wind::default();
        assert!((w.speed() - 3.0).abs() < 0.001);
        assert_eq!(w.turbulence, 0.0);
        assert!(w.is_global());
    }

    #[test]
    fn eastward_speed() {
        let w = Wind::eastward(10.0);
        assert!((w.speed() - 10.0).abs() < 0.001);
        assert!((w.velocity.x - 10.0).abs() < 0.001);
    }

    #[test]
    fn turbulence_clamped() {
        let w = Wind::new(Vec3::X).with_turbulence(-2.0);
        assert_eq!(w.turbulence, 0.0);
    }

    #[test]
    fn radius_makes_local() {
        let w = Wind::eastward(5.0).with_radius(20.0);
        assert!(!w.is_global());
        assert!((w.radius - 20.0).abs() < 0.001);
    }

    #[test]
    fn speed_from_velocity() {
        let w = Wind::new(Vec3::new(3.0, 4.0, 0.0));
        assert!((w.speed() - 5.0).abs() < 0.001);
    }
}
