use bevy_ecs::prelude::Component;

/// Probability-based partial-damage mitigation: when a projectile would deal
/// full damage, the combat system rolls a random value and calls
/// `test_graze(roll)` — if the roll falls below `graze_chance` the entity
/// takes only `graze_fraction` of the incoming damage instead.
///
/// `test_graze(roll)` accepts a caller-supplied random value in [0.0, 1.0],
/// returns `true` when a graze triggers, fires `just_grazed`, and increments
/// `hit_count`. Returns `false` and is otherwise a no-op when disabled.
///
/// `graze_damage(incoming)` returns the reduced damage amount to apply after
/// a successful `test_graze()` call.
///
/// `tick()` clears `just_grazed` at the start of each frame.
///
/// Distinct from `Parry` (active directional block), `Deflect` (reflects the
/// projectile back), `Dodge` (full-evasion chance), and `Shield` (HP-like
/// damage buffer): Graze is a **probability-based partial-damage mitigation**
/// — the projectile still connects but clips the edge, dealing reduced rather
/// than zero or full damage.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Graze {
    /// Probability that an incoming hit is a graze [0.0, 1.0]. Clamped.
    pub graze_chance: f32,
    /// Fraction of incoming damage taken on a graze hit [0.0, 1.0]. Clamped.
    pub graze_fraction: f32,
    /// Cumulative number of successful graze events.
    pub hit_count: u32,
    pub just_grazed: bool,
    pub enabled: bool,
}

impl Graze {
    pub fn new(graze_chance: f32, graze_fraction: f32) -> Self {
        Self {
            graze_chance: graze_chance.clamp(0.0, 1.0),
            graze_fraction: graze_fraction.clamp(0.0, 1.0),
            hit_count: 0,
            just_grazed: false,
            enabled: true,
        }
    }

    /// Test whether an incoming hit grazes. `roll` must be a random value in
    /// [0.0, 1.0] supplied by the caller. Returns `true` (graze triggers)
    /// when `roll < graze_chance`. Fires `just_grazed` and increments
    /// `hit_count` on success. No-op (returns `false`) when disabled.
    pub fn test_graze(&mut self, roll: f32) -> bool {
        if !self.enabled {
            return false;
        }
        if roll < self.graze_chance {
            self.just_grazed = true;
            self.hit_count += 1;
            true
        } else {
            false
        }
    }

    /// Damage taken after a graze: `incoming * graze_fraction`.
    /// Call this only after `test_graze()` returned `true`.
    pub fn graze_damage(&self, incoming: f32) -> f32 {
        incoming * self.graze_fraction
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_grazed = false;
    }

    /// `true` when the component is enabled and graze_chance > 0.
    pub fn can_graze(&self) -> bool {
        self.enabled && self.graze_chance > 0.0
    }
}

impl Default for Graze {
    fn default() -> Self {
        Self::new(0.2, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_correct_fields() {
        let g = Graze::new(0.2, 0.3);
        assert!((g.graze_chance - 0.2).abs() < 1e-5);
        assert!((g.graze_fraction - 0.3).abs() < 1e-5);
        assert_eq!(g.hit_count, 0);
        assert!(!g.just_grazed);
    }

    #[test]
    fn test_graze_triggers_on_low_roll() {
        let mut g = Graze::new(0.5, 0.3);
        assert!(g.test_graze(0.0));
        assert!(g.just_grazed);
        assert_eq!(g.hit_count, 1);
    }

    #[test]
    fn test_graze_triggers_just_below_chance() {
        let mut g = Graze::new(0.5, 0.3);
        assert!(g.test_graze(0.499));
        assert!(g.just_grazed);
    }

    #[test]
    fn test_graze_no_trigger_on_high_roll() {
        let mut g = Graze::new(0.5, 0.3);
        assert!(!g.test_graze(0.5));
        assert!(!g.just_grazed);
        assert_eq!(g.hit_count, 0);
    }

    #[test]
    fn test_graze_no_trigger_on_roll_one() {
        let mut g = Graze::new(0.5, 0.3);
        assert!(!g.test_graze(1.0));
    }

    #[test]
    fn test_graze_accumulates_hit_count() {
        let mut g = Graze::new(1.0, 0.3); // always grazes
        g.test_graze(0.0);
        g.test_graze(0.0);
        g.test_graze(0.0);
        assert_eq!(g.hit_count, 3);
    }

    #[test]
    fn test_graze_no_op_when_disabled() {
        let mut g = Graze::new(1.0, 0.3);
        g.enabled = false;
        assert!(!g.test_graze(0.0));
        assert!(!g.just_grazed);
        assert_eq!(g.hit_count, 0);
    }

    #[test]
    fn graze_damage_returns_fraction() {
        let g = Graze::new(0.5, 0.4);
        // 100 * 0.4 = 40
        assert!((g.graze_damage(100.0) - 40.0).abs() < 1e-3);
    }

    #[test]
    fn graze_damage_full_at_fraction_one() {
        let g = Graze::new(0.5, 1.0);
        assert!((g.graze_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn graze_damage_zero_at_fraction_zero() {
        let g = Graze::new(0.5, 0.0);
        assert!((g.graze_damage(100.0)).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_grazed() {
        let mut g = Graze::new(1.0, 0.3);
        g.test_graze(0.0);
        g.tick();
        assert!(!g.just_grazed);
    }

    #[test]
    fn can_graze_true_when_chance_positive() {
        let g = Graze::new(0.5, 0.3);
        assert!(g.can_graze());
    }

    #[test]
    fn can_graze_false_when_chance_zero() {
        let g = Graze::new(0.0, 0.3);
        assert!(!g.can_graze());
    }

    #[test]
    fn can_graze_false_when_disabled() {
        let mut g = Graze::new(0.5, 0.3);
        g.enabled = false;
        assert!(!g.can_graze());
    }

    #[test]
    fn graze_chance_clamped_at_one() {
        let g = Graze::new(2.0, 0.3);
        assert!((g.graze_chance - 1.0).abs() < 1e-5);
    }

    #[test]
    fn graze_chance_clamped_at_zero() {
        let g = Graze::new(-0.5, 0.3);
        assert_eq!(g.graze_chance, 0.0);
    }

    #[test]
    fn graze_fraction_clamped_at_one() {
        let g = Graze::new(0.5, 1.5);
        assert!((g.graze_fraction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn graze_fraction_clamped_at_zero() {
        let g = Graze::new(0.5, -0.3);
        assert_eq!(g.graze_fraction, 0.0);
    }

    #[test]
    fn just_grazed_false_after_failed_test() {
        let mut g = Graze::new(0.5, 0.3);
        g.test_graze(0.9); // no graze
        assert!(!g.just_grazed);
    }
}
