use bevy_ecs::prelude::{Component, Resource};
use glam::Vec3;

/// Global gravitational acceleration applied to all entities with `Velocity`.
/// Insert this resource to override the default (9.81 m/s² downward).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Gravity {
    pub acceleration: Vec3,
}

impl Default for Gravity {
    fn default() -> Self {
        Self {
            acceleration: Vec3::new(0.0, -9.81, 0.0),
        }
    }
}

impl Gravity {
    pub fn new(acceleration: Vec3) -> Self {
        Self { acceleration }
    }

    pub fn zero() -> Self {
        Self {
            acceleration: Vec3::ZERO,
        }
    }
}

/// Per-entity gravity multiplier. Use 0.0 for gravity-immune entities,
/// negative values for reverse gravity, >1.0 for heavy objects.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GravityScale(pub f32);

impl Default for GravityScale {
    fn default() -> Self {
        Self(1.0)
    }
}

impl GravityScale {
    pub fn new(scale: f32) -> Self {
        Self(scale)
    }

    pub fn value(self) -> f32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_gravity_is_downward() {
        let g = Gravity::default();
        assert!(g.acceleration.y < 0.0);
        assert_eq!(g.acceleration.x, 0.0);
        assert_eq!(g.acceleration.z, 0.0);
    }

    #[test]
    fn zero_gravity() {
        let g = Gravity::zero();
        assert_eq!(g.acceleration, Vec3::ZERO);
    }

    #[test]
    fn custom_gravity() {
        let g = Gravity::new(Vec3::new(0.0, -20.0, 0.0));
        assert_eq!(g.acceleration.y, -20.0);
    }

    #[test]
    fn gravity_scale_default_is_one() {
        assert_eq!(GravityScale::default().value(), 1.0);
    }

    #[test]
    fn gravity_scale_stores_value() {
        assert_eq!(GravityScale::new(2.5).value(), 2.5);
    }

    #[test]
    fn gravity_scale_zero_immune() {
        assert_eq!(GravityScale::new(0.0).value(), 0.0);
    }
}
