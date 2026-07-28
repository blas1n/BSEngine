use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::Vec3;

/// Accumulated impulses (kg·m/s) to apply to this entity during the physics step.
/// The `ExternalImpulsePlugin` reads these, converts to velocity changes via mass,
/// then zeroes the values — so impulses last exactly one frame.
///
/// Add individual impulses by calling `.add_linear()` / `.add_angular()` rather than
/// assigning directly, so multiple systems can contribute without clobbering each other.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default, Reflect)]
#[reflect(Component, Default)]
pub struct ExternalImpulse {
    /// Pending linear impulse (kg·m/s), applied then cleared each physics step.
    pub linear: ReflectVec3,
    /// Pending angular impulse (kg·m²/s), applied then cleared each physics step.
    pub angular: ReflectVec3,
}

impl ExternalImpulse {
    /// Creates an impulse with only a linear component set.
    pub fn linear(linear: Vec3) -> Self {
        Self {
            linear: linear.into(),
            angular: Vec3::ZERO.into(),
        }
    }

    /// Creates an impulse with only an angular component set.
    pub fn angular(angular: Vec3) -> Self {
        Self {
            linear: Vec3::ZERO.into(),
            angular: angular.into(),
        }
    }

    /// Accumulates an additional linear impulse onto the pending value.
    pub fn add_linear(&mut self, impulse: Vec3) {
        self.linear.0 += impulse;
    }

    /// Accumulates an additional angular impulse onto the pending value.
    pub fn add_angular(&mut self, impulse: Vec3) {
        self.angular.0 += impulse;
    }

    /// Resets both linear and angular impulses to zero.
    pub fn clear(&mut self) {
        self.linear = Vec3::ZERO.into();
        self.angular = Vec3::ZERO.into();
    }

    /// Returns true if both linear and angular impulses are exactly zero.
    pub fn is_zero(&self) -> bool {
        self.linear.0 == Vec3::ZERO && self.angular.0 == Vec3::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let i = ExternalImpulse::default();
        assert!(i.is_zero());
    }

    #[test]
    fn add_linear_accumulates() {
        let mut i = ExternalImpulse::default();
        i.add_linear(Vec3::X);
        i.add_linear(Vec3::X);
        assert_eq!(i.linear, Vec3::new(2.0, 0.0, 0.0).into());
    }

    #[test]
    fn add_angular_accumulates() {
        let mut i = ExternalImpulse::default();
        i.add_angular(Vec3::Y);
        i.add_angular(Vec3::Y);
        assert_eq!(i.angular, Vec3::new(0.0, 2.0, 0.0).into());
    }

    #[test]
    fn clear_resets_to_zero() {
        let mut i = ExternalImpulse::linear(Vec3::ONE);
        i.clear();
        assert!(i.is_zero());
    }

    #[test]
    fn linear_ctor_sets_only_linear() {
        let i = ExternalImpulse::linear(Vec3::Z);
        assert_eq!(i.linear, Vec3::Z.into());
        assert_eq!(i.angular, Vec3::ZERO.into());
    }

    #[test]
    fn angular_ctor_sets_only_angular() {
        let i = ExternalImpulse::angular(Vec3::Y);
        assert_eq!(i.linear, Vec3::ZERO.into());
        assert_eq!(i.angular, Vec3::Y.into());
    }
}
