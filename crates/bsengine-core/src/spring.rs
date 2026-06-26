use bevy_ecs::prelude::{Component, Entity};

/// A spring-damper constraint between this entity and `target`.
/// The physics system applies a Hooke's-law force along the line connecting the two entities.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Spring {
    /// The other entity this spring is attached to.
    pub target: Entity,
    /// Natural length of the spring in world units. The spring is relaxed at this distance.
    pub rest_length: f32,
    /// Spring stiffness coefficient (k) in N/m. Higher = stiffer.
    pub stiffness: f32,
    /// Damping coefficient (c) in N·s/m. Higher = less oscillation.
    pub damping: f32,
    /// If the stretch exceeds this length in world units, the spring breaks.
    /// `None` means the spring never breaks.
    pub break_extension: Option<f32>,
    pub enabled: bool,
}

impl Spring {
    pub fn new(target: Entity, rest_length: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            target,
            rest_length: rest_length.max(0.0),
            stiffness: stiffness.max(0.0),
            damping: damping.max(0.0),
            break_extension: None,
            enabled: true,
        }
    }

    pub fn with_break_extension(mut self, max_extension: f32) -> Self {
        self.break_extension = Some(max_extension.max(0.0));
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns the spring force magnitude for the given current length.
    /// Positive = pulling together (stretched), negative = pushing apart (compressed).
    pub fn force(&self, current_length: f32) -> f32 {
        let extension = current_length - self.rest_length;
        self.stiffness * extension
    }

    /// Returns true if the spring should break given the current extension.
    pub fn should_break(&self, current_length: f32) -> bool {
        match self.break_extension {
            Some(max) => (current_length - self.rest_length).abs() > max,
            None => false,
        }
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
    fn spring_at_rest_zero_force() {
        let e = make_entity();
        let s = Spring::new(e, 2.0, 100.0, 5.0);
        assert!((s.force(2.0)).abs() < 0.001);
    }

    #[test]
    fn spring_stretched_positive_force() {
        let e = make_entity();
        let s = Spring::new(e, 1.0, 50.0, 0.0);
        let f = s.force(3.0); // extended by 2m
        assert!((f - 100.0).abs() < 0.001);
    }

    #[test]
    fn spring_break_extension() {
        let e = make_entity();
        let s = Spring::new(e, 1.0, 100.0, 5.0).with_break_extension(2.0);
        assert!(!s.should_break(2.5)); // 1.5m extension
        assert!(s.should_break(4.0)); // 3.0m extension > 2.0
    }

    #[test]
    fn spring_no_break_by_default() {
        let e = make_entity();
        let s = Spring::new(e, 1.0, 100.0, 0.0);
        assert!(!s.should_break(1000.0));
    }

    #[test]
    fn spring_negative_inputs_clamped() {
        let e = make_entity();
        let s = Spring::new(e, -5.0, -10.0, -1.0);
        assert_eq!(s.rest_length, 0.0);
        assert_eq!(s.stiffness, 0.0);
        assert_eq!(s.damping, 0.0);
    }
}
