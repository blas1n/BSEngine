use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// The type of constraint applied between two entities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConstraintKind {
    /// Entities must stay exactly `distance` apart.
    Distance { distance: f32 },
    /// Entity B must stay within `min`..`max` distance from entity A.
    DistanceRange { min: f32, max: f32 },
    /// Entity B orbits entity A on a fixed axis, restricted to an angle range (radians).
    Hinge {
        axis: Vec3,
        min_angle: f32,
        max_angle: f32,
    },
    /// Entity B is locked to entity A in world position (no relative movement).
    Fixed,
    /// Entity B slides along `axis` in A's local space, within `min`..`max`.
    Slider { axis: Vec3, min: f32, max: f32 },
}

/// A physics/animation constraint linking this entity to another.
/// The physics system reads this to apply corrective impulses each frame.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Constraint {
    /// The other entity this constraint connects to.
    pub target: Entity,
    pub kind: ConstraintKind,
    /// Stiffness coefficient in [0, 1]. 1.0 = perfectly rigid; lower = springy.
    pub stiffness: f32,
    /// Damping coefficient — reduces oscillation.
    pub damping: f32,
    pub enabled: bool,
}

impl Constraint {
    pub fn new(target: Entity, kind: ConstraintKind) -> Self {
        Self {
            target,
            kind,
            stiffness: 1.0,
            damping: 0.1,
            enabled: true,
        }
    }

    pub fn fixed(target: Entity) -> Self {
        Self::new(target, ConstraintKind::Fixed)
    }

    pub fn distance(target: Entity, distance: f32) -> Self {
        Self::new(
            target,
            ConstraintKind::Distance {
                distance: distance.max(0.0),
            },
        )
    }

    pub fn with_stiffness(mut self, stiffness: f32) -> Self {
        self.stiffness = stiffness.clamp(0.0, 1.0);
        self
    }

    pub fn with_damping(mut self, damping: f32) -> Self {
        self.damping = damping.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns `true` if the constraint is rigid (stiffness == 1.0).
    pub fn is_rigid(&self) -> bool {
        (self.stiffness - 1.0).abs() < f32::EPSILON
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::World;

    fn dummy_entity() -> Entity {
        let mut world = World::new();
        world.spawn_empty().id()
    }

    #[test]
    fn constraint_fixed_defaults() {
        let e = dummy_entity();
        let c = Constraint::fixed(e);
        assert_eq!(c.kind, ConstraintKind::Fixed);
        assert!(c.is_rigid());
        assert!(c.enabled);
    }

    #[test]
    fn constraint_distance() {
        let e = dummy_entity();
        let c = Constraint::distance(e, 5.0);
        assert_eq!(c.kind, ConstraintKind::Distance { distance: 5.0 });
    }

    #[test]
    fn stiffness_clamped() {
        let e = dummy_entity();
        let c = Constraint::fixed(e).with_stiffness(2.0);
        assert_eq!(c.stiffness, 1.0);
    }

    #[test]
    fn soft_constraint_not_rigid() {
        let e = dummy_entity();
        let c = Constraint::fixed(e).with_stiffness(0.5);
        assert!(!c.is_rigid());
    }

    #[test]
    fn disabled_constraint() {
        let e = dummy_entity();
        let c = Constraint::distance(e, 3.0).disabled();
        assert!(!c.enabled);
    }
}
