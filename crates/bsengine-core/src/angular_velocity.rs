use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Angular velocity in radians per second (Euler rates: x=pitch, y=yaw, z=roll).
/// `AngularVelocityPlugin` integrates this into `Transform.rotation` each frame.
/// For physics-driven rotation use `bsengine-physics` instead.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct AngularVelocity {
    pub angular: Vec3,
}

impl Default for AngularVelocity {
    fn default() -> Self {
        Self {
            angular: Vec3::ZERO,
        }
    }
}

impl AngularVelocity {
    pub fn new(pitch: f32, yaw: f32, roll: f32) -> Self {
        Self {
            angular: Vec3::new(pitch, yaw, roll),
        }
    }

    pub fn from_vec3(angular: Vec3) -> Self {
        Self { angular }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        assert_eq!(AngularVelocity::default().angular, Vec3::ZERO);
    }

    #[test]
    fn new_sets_components() {
        let av = AngularVelocity::new(1.0, 2.0, 3.0);
        assert_eq!(av.angular, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn from_vec3_stores_value() {
        let av = AngularVelocity::from_vec3(Vec3::Y * std::f32::consts::PI);
        assert_eq!(av.angular, Vec3::Y * std::f32::consts::PI);
    }

    #[test]
    fn equality() {
        assert_eq!(
            AngularVelocity::new(0.0, 1.0, 0.0),
            AngularVelocity::from_vec3(Vec3::Y)
        );
    }
}
