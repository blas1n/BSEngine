use bevy_ecs::prelude::{Component, Entity};

/// Connects this entity to an `anchor` entity via a soft or hard distance constraint.
///
/// Unlike `Rope` (multi-segment) or `Constraint` (rigid physics joint), `Tether`
/// is a single-segment elastic/inelastic link: beyond `max_length` the physics
/// system should apply a restoring force or teleport the entity back.
///
/// Common uses: pets leashing to an owner, enemies leashing back to their spawn
/// point, a balloon tied to a character's hand, a prisoner chained to a post.
///
/// The physics/movement system reads `excess_distance()` each frame and applies
/// a spring force scaled by `spring_strength` (soft tether), or if `hard` is
/// true, clamps the position instead (hard cap). `just_pulled_taut` fires once
/// when the entity transitions from within range to beyond `max_length`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Tether {
    /// The entity this tether is anchored to.
    pub anchor: Option<Entity>,
    /// Maximum allowed distance between this entity and the anchor.
    pub max_length: f32,
    /// If true, the physics system hard-clamps position rather than applying force.
    pub hard: bool,
    /// Spring constant used when `hard` is false (force = excess * spring_strength).
    pub spring_strength: f32,
    /// True this frame if the entity just exceeded `max_length` for the first time.
    pub just_pulled_taut: bool,
    /// True this frame if the entity returned inside `max_length`.
    pub just_slack: bool,
    /// Whether the tether is currently taut (entity is at or beyond max_length).
    pub is_taut: bool,
    pub enabled: bool,
}

impl Tether {
    pub fn new(anchor: Entity, max_length: f32) -> Self {
        Self {
            anchor: Some(anchor),
            max_length: max_length.max(0.0),
            hard: false,
            spring_strength: 10.0,
            just_pulled_taut: false,
            just_slack: false,
            is_taut: false,
            enabled: true,
        }
    }

    pub fn hard(mut self) -> Self {
        self.hard = true;
        self
    }

    pub fn with_spring(mut self, strength: f32) -> Self {
        self.spring_strength = strength.max(0.0);
        self
    }

    /// Called by the movement/physics system with the current distance to the anchor.
    /// Updates `is_taut`, `just_pulled_taut`, `just_slack`.
    pub fn update_distance(&mut self, distance: f32) {
        let was_taut = self.is_taut;
        self.is_taut = distance > self.max_length;
        self.just_pulled_taut = self.is_taut && !was_taut;
        self.just_slack = !self.is_taut && was_taut;
    }

    /// How far beyond `max_length` the entity currently is (0 if within range).
    pub fn excess_distance(&self, distance: f32) -> f32 {
        (distance - self.max_length).max(0.0)
    }

    /// The spring force magnitude to apply back toward the anchor (soft tether).
    pub fn spring_force(&self, distance: f32) -> f32 {
        self.excess_distance(distance) * self.spring_strength
    }

    pub fn detach(&mut self) {
        self.anchor = None;
        self.is_taut = false;
        self.just_pulled_taut = false;
        self.just_slack = false;
    }

    pub fn is_attached(&self) -> bool {
        self.anchor.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;

    fn dummy_entity() -> Entity {
        Entity::from_raw(0)
    }

    #[test]
    fn new_creates_soft_tether() {
        let t = Tether::new(dummy_entity(), 5.0);
        assert!(!t.hard);
        assert!(t.is_attached());
        assert!((t.max_length - 5.0).abs() < 1e-5);
    }

    #[test]
    fn update_distance_within_range() {
        let mut t = Tether::new(dummy_entity(), 5.0);
        t.update_distance(3.0);
        assert!(!t.is_taut);
        assert!(!t.just_pulled_taut);
    }

    #[test]
    fn update_distance_goes_taut() {
        let mut t = Tether::new(dummy_entity(), 5.0);
        t.update_distance(6.0);
        assert!(t.is_taut);
        assert!(t.just_pulled_taut);
        assert!(!t.just_slack);
    }

    #[test]
    fn update_distance_returns_slack() {
        let mut t = Tether::new(dummy_entity(), 5.0);
        t.update_distance(6.0);
        t.update_distance(3.0);
        assert!(!t.is_taut);
        assert!(t.just_slack);
        assert!(!t.just_pulled_taut);
    }

    #[test]
    fn excess_distance_zero_within_range() {
        let t = Tether::new(dummy_entity(), 5.0);
        assert!((t.excess_distance(3.0)).abs() < 1e-5);
    }

    #[test]
    fn excess_distance_positive_beyond_range() {
        let t = Tether::new(dummy_entity(), 5.0);
        assert!((t.excess_distance(8.0) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn spring_force_scales_with_excess() {
        let t = Tether::new(dummy_entity(), 5.0).with_spring(2.0);
        let force = t.spring_force(7.0); // excess = 2.0
        assert!((force - 4.0).abs() < 1e-5);
    }

    #[test]
    fn detach_removes_anchor() {
        let mut t = Tether::new(dummy_entity(), 5.0);
        t.detach();
        assert!(!t.is_attached());
    }

    #[test]
    fn hard_tether_flag() {
        let t = Tether::new(dummy_entity(), 5.0).hard();
        assert!(t.hard);
    }
}
