use bevy_ecs::prelude::Component;

/// Physics body simulation mode.
///
/// - `Dynamic` — fully simulated; gravity, forces, and impulses apply
/// - `Kinematic` — moved by code only; not affected by forces or gravity,
///   but still participates in collision response as a solid obstacle
/// - `Static` — immovable; zero mass, never moves, cheapest to simulate
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RigidBody {
    #[default]
    Dynamic,
    Kinematic,
    Static,
}

impl RigidBody {
    pub fn is_dynamic(&self) -> bool {
        matches!(self, RigidBody::Dynamic)
    }

    pub fn is_kinematic(&self) -> bool {
        matches!(self, RigidBody::Kinematic)
    }

    pub fn is_static(&self) -> bool {
        matches!(self, RigidBody::Static)
    }

    /// True if the body can be affected by forces (only Dynamic).
    pub fn is_affected_by_forces(&self) -> bool {
        self.is_dynamic()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_dynamic() {
        assert_eq!(RigidBody::default(), RigidBody::Dynamic);
    }

    #[test]
    fn dynamic_flags() {
        let b = RigidBody::Dynamic;
        assert!(b.is_dynamic());
        assert!(!b.is_kinematic());
        assert!(!b.is_static());
        assert!(b.is_affected_by_forces());
    }

    #[test]
    fn kinematic_flags() {
        let b = RigidBody::Kinematic;
        assert!(!b.is_dynamic());
        assert!(b.is_kinematic());
        assert!(!b.is_static());
        assert!(!b.is_affected_by_forces());
    }

    #[test]
    fn static_flags() {
        let b = RigidBody::Static;
        assert!(!b.is_dynamic());
        assert!(!b.is_kinematic());
        assert!(b.is_static());
        assert!(!b.is_affected_by_forces());
    }
}
