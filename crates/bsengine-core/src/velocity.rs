use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::Vec3;

/// Kinematic linear velocity in world units per second.
/// `VelocityPlugin` integrates this into `Transform.translation` each frame.
/// For physics-driven motion use `bsengine-physics` instead.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct Velocity {
    /// World-space linear velocity, in units per second.
    pub linear: ReflectVec3,
}

impl Default for Velocity {
    fn default() -> Self {
        Self {
            linear: Vec3::ZERO.into(),
        }
    }
}

impl Velocity {
    /// Creates a velocity from individual X/Y/Z components, in units per second.
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            linear: Vec3::new(x, y, z).into(),
        }
    }

    /// Creates a velocity from a `glam::Vec3`.
    pub fn from_vec3(linear: Vec3) -> Self {
        Self {
            linear: linear.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        assert_eq!(Velocity::default().linear, Vec3::ZERO.into());
    }

    #[test]
    fn new_sets_components() {
        let v = Velocity::new(1.0, 2.0, 3.0);
        assert_eq!(v.linear, Vec3::new(1.0, 2.0, 3.0).into());
    }

    #[test]
    fn from_vec3_stores_value() {
        let v = Velocity::from_vec3(Vec3::X * 5.0);
        assert_eq!(v.linear, (Vec3::X * 5.0).into());
    }

    #[test]
    fn equality() {
        assert_eq!(Velocity::new(1.0, 0.0, 0.0), Velocity::from_vec3(Vec3::X));
    }
}
