use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// The type of physics constraint between two entities.
#[derive(Debug, Clone, PartialEq)]
pub enum JointType {
    /// No relative movement permitted.
    Fixed,
    /// Rotation around a single axis, with optional angular limits.
    Hinge {
        axis: Vec3,
        /// Minimum angle in radians (negative = behind axis).
        angle_min: f32,
        /// Maximum angle in radians.
        angle_max: f32,
    },
    /// Linear sliding along an axis, with optional position limits.
    Slider {
        axis: Vec3,
        /// Minimum displacement in world units.
        limit_min: f32,
        /// Maximum displacement in world units.
        limit_max: f32,
    },
    /// Free rotation within a cone (ball-and-socket).
    Ball {
        /// Half-angle of the swing cone in radians.
        cone_angle: f32,
    },
}

/// A physics constraint that links this entity to `target`.
/// The physics solver enforces the joint each simulation step.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Joint {
    /// The other entity to constrain against. Must also have a `RigidBody`.
    pub target: Entity,
    pub joint_type: JointType,
    /// Maximum impulse magnitude before the joint breaks. 0.0 = unbreakable.
    pub break_force: f32,
    /// Whether this joint is currently active.
    pub enabled: bool,
}

impl Joint {
    pub fn fixed(target: Entity) -> Self {
        Self {
            target,
            joint_type: JointType::Fixed,
            break_force: 0.0,
            enabled: true,
        }
    }

    pub fn hinge(target: Entity, axis: Vec3, angle_min: f32, angle_max: f32) -> Self {
        Self {
            target,
            joint_type: JointType::Hinge {
                axis: axis.normalize_or_zero(),
                angle_min: angle_min.min(angle_max),
                angle_max: angle_max.max(angle_min),
            },
            break_force: 0.0,
            enabled: true,
        }
    }

    pub fn slider(target: Entity, axis: Vec3, limit_min: f32, limit_max: f32) -> Self {
        Self {
            target,
            joint_type: JointType::Slider {
                axis: axis.normalize_or_zero(),
                limit_min: limit_min.min(limit_max),
                limit_max: limit_max.max(limit_min),
            },
            break_force: 0.0,
            enabled: true,
        }
    }

    pub fn ball(target: Entity, cone_angle: f32) -> Self {
        Self {
            target,
            joint_type: JointType::Ball {
                cone_angle: cone_angle.clamp(0.0, std::f32::consts::PI),
            },
            break_force: 0.0,
            enabled: true,
        }
    }

    pub fn with_break_force(mut self, force: f32) -> Self {
        self.break_force = force.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn is_breakable(&self) -> bool {
        self.break_force > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    fn make_entity() -> Entity {
        World::new().spawn_empty().id()
    }

    #[test]
    fn fixed_joint_defaults() {
        let e = make_entity();
        let j = Joint::fixed(e);
        assert_eq!(j.joint_type, JointType::Fixed);
        assert_eq!(j.break_force, 0.0);
        assert!(!j.is_breakable());
        assert!(j.enabled);
    }

    #[test]
    fn hinge_joint_sorts_angles() {
        let e = make_entity();
        let j = Joint::hinge(e, Vec3::Y, 0.5, -0.5);
        if let JointType::Hinge {
            angle_min,
            angle_max,
            ..
        } = j.joint_type
        {
            assert!(angle_min <= angle_max);
        } else {
            panic!("Expected Hinge");
        }
    }

    #[test]
    fn ball_joint_clamps_cone_angle() {
        let e = make_entity();
        let j = Joint::ball(e, 10.0); // 10 rad > PI
        if let JointType::Ball { cone_angle } = j.joint_type {
            assert!(cone_angle <= std::f32::consts::PI);
        }
    }

    #[test]
    fn break_force_and_disabled() {
        let e = make_entity();
        let j = Joint::fixed(e).with_break_force(1000.0).disabled();
        assert!(j.is_breakable());
        assert!(!j.enabled);
    }
}
