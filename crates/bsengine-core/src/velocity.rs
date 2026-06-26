use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Kinematic linear velocity in world units per second.
/// `VelocityPlugin` integrates this into `Transform.translation` each frame.
/// For physics-driven motion use `bsengine-physics` instead.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Velocity {
    pub linear: Vec3,
}

impl Default for Velocity {
    fn default() -> Self {
        Self { linear: Vec3::ZERO }
    }
}

impl Velocity {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            linear: Vec3::new(x, y, z),
        }
    }

    pub fn from_vec3(linear: Vec3) -> Self {
        Self { linear }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        assert_eq!(Velocity::default().linear, Vec3::ZERO);
    }

    #[test]
    fn new_sets_components() {
        let v = Velocity::new(1.0, 2.0, 3.0);
        assert_eq!(v.linear, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn from_vec3_stores_value() {
        let v = Velocity::from_vec3(Vec3::X * 5.0);
        assert_eq!(v.linear, Vec3::X * 5.0);
    }

    #[test]
    fn equality() {
        assert_eq!(Velocity::new(1.0, 0.0, 0.0), Velocity::from_vec3(Vec3::X));
    }
}
