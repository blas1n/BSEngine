use bevy_ecs::prelude::Component;

/// Equipment durability tracker: `condition` [0.0, 1.0] represents the
/// structural integrity of the entity's armor or equipment. Each incoming
/// hit degrades it by `degrade_per_hit`. At 0.0 the item is broken.
///
/// `hit()` subtracts `degrade_per_hit` from `condition` (floored at 0.0)
/// and fires `just_broke` on the transition that first reaches 0.0.
/// No-op when disabled.
///
/// `repair(amount)` adds to `condition` (capped at 1.0). No-op when disabled.
///
/// `tick()` clears `just_broke` each frame.
///
/// `defense_factor()` returns `condition` when enabled, or `1.0` when
/// disabled — broken equipment provides no defense, full condition provides
/// full defense. Multiply incoming damage reduction by this factor.
///
/// `is_broken()` returns `condition <= 0.0` (state is always observable,
/// regardless of `enabled`).
///
/// Distinct from `Health` (entity biological HP), `Armor` (static damage
/// reduction coefficient), and `Shield` (temporary absorb layer): Wear
/// is a **structural degradation tracker** — represents the slowly
/// diminishing protection of equipment that wears down under repeated
/// impacts and must be repaired between encounters.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wear {
    /// Current equipment condition [0.0 = broken, 1.0 = pristine].
    pub condition: f32,
    /// Condition lost per incoming hit. Clamped [0.0, 1.0].
    pub degrade_per_hit: f32,
    pub just_broke: bool,
    pub enabled: bool,
}

impl Wear {
    pub fn new(degrade_per_hit: f32) -> Self {
        Self {
            condition: 1.0,
            degrade_per_hit: degrade_per_hit.clamp(0.0, 1.0),
            just_broke: false,
            enabled: true,
        }
    }

    /// Degrade `condition` by `degrade_per_hit`, floored at 0.0. Fires
    /// `just_broke` on the first transition to 0.0. No-op when disabled.
    pub fn hit(&mut self) {
        if !self.enabled {
            return;
        }
        let was_intact = !self.is_broken();
        self.condition = (self.condition - self.degrade_per_hit).max(0.0);
        if was_intact && self.is_broken() {
            self.just_broke = true;
        }
    }

    /// Restore `condition` by `amount`, capped at 1.0. No-op when disabled
    /// or `amount ≤ 0`.
    pub fn repair(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.condition = (self.condition + amount).min(1.0);
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_broke = false;
    }

    /// `true` when condition has reached 0.0. Always observable regardless
    /// of `enabled`.
    pub fn is_broken(&self) -> bool {
        self.condition <= 0.0
    }

    /// Defense effectiveness scalar.
    /// Returns `condition` when enabled; returns `1.0` (full defense) when
    /// disabled (the component is inactive, not broken). Multiply the
    /// entity's armor damage-reduction by this factor.
    pub fn defense_factor(&self) -> f32 {
        if self.enabled {
            self.condition
        } else {
            1.0
        }
    }
}

impl Default for Wear {
    fn default() -> Self {
        Self::new(0.05)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_pristine() {
        let w = Wear::new(0.05);
        assert!((w.condition - 1.0).abs() < 1e-5);
        assert!(!w.is_broken());
    }

    #[test]
    fn hit_degrades_condition() {
        let mut w = Wear::new(0.1);
        w.hit();
        assert!((w.condition - 0.9).abs() < 1e-5);
    }

    #[test]
    fn hit_floors_at_zero() {
        let mut w = Wear::new(0.6);
        w.hit(); // 0.4
        w.hit(); // would be -0.2, floors to 0
        assert!(w.condition.abs() < 1e-5);
    }

    #[test]
    fn hit_fires_just_broke_on_transition() {
        let mut w = Wear::new(1.0);
        w.hit(); // 1.0 - 1.0 = 0.0
        assert!(w.just_broke);
        assert!(w.is_broken());
    }

    #[test]
    fn hit_no_just_broke_when_already_broken() {
        let mut w = Wear::new(1.0);
        w.hit(); // breaks
        w.tick();
        w.hit(); // already broken
        assert!(!w.just_broke);
    }

    #[test]
    fn hit_multiple_to_break() {
        let mut w = Wear::new(0.4);
        w.hit(); // 0.6
        w.hit(); // 0.2
        assert!(!w.just_broke);
        w.hit(); // 0.0 (actually max(0, 0.2-0.4) = 0)
        assert!(w.just_broke);
    }

    #[test]
    fn hit_no_op_when_disabled() {
        let mut w = Wear::new(0.1);
        w.enabled = false;
        w.hit();
        assert!((w.condition - 1.0).abs() < 1e-5);
    }

    #[test]
    fn repair_restores_condition() {
        let mut w = Wear::new(0.2);
        w.hit(); // 0.8
        w.repair(0.1);
        assert!((w.condition - 0.9).abs() < 1e-5);
    }

    #[test]
    fn repair_caps_at_one() {
        let mut w = Wear::new(0.1);
        w.hit(); // 0.9
        w.repair(0.5); // would be 1.4, caps at 1.0
        assert!((w.condition - 1.0).abs() < 1e-5);
    }

    #[test]
    fn repair_no_op_when_disabled() {
        let mut w = Wear::new(0.5);
        w.hit(); // 0.5
        w.enabled = false;
        w.repair(0.5);
        assert!((w.condition - 0.5).abs() < 1e-5);
    }

    #[test]
    fn repair_no_op_at_zero_amount() {
        let mut w = Wear::new(0.2);
        w.hit(); // 0.8
        w.repair(0.0);
        assert!((w.condition - 0.8).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_broke() {
        let mut w = Wear::new(1.0);
        w.hit();
        w.tick();
        assert!(!w.just_broke);
    }

    #[test]
    fn is_broken_true_at_zero() {
        let mut w = Wear::new(1.0);
        w.hit();
        assert!(w.is_broken());
    }

    #[test]
    fn is_broken_false_when_partial() {
        let mut w = Wear::new(0.5);
        w.hit(); // 0.5 remaining
        assert!(!w.is_broken());
    }

    #[test]
    fn defense_factor_matches_condition() {
        let mut w = Wear::new(0.2);
        w.hit(); // 0.8
        assert!((w.defense_factor() - 0.8).abs() < 1e-5);
    }

    #[test]
    fn defense_factor_one_when_disabled() {
        let mut w = Wear::new(1.0);
        w.hit(); // would be broken
        w.enabled = false;
        assert!((w.defense_factor() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn defense_factor_zero_when_broken() {
        let mut w = Wear::new(1.0);
        w.hit();
        assert!(w.defense_factor().abs() < 1e-5);
    }

    #[test]
    fn repair_after_break_allows_re_break() {
        let mut w = Wear::new(1.0);
        w.hit(); // breaks
        w.tick();
        w.repair(1.0); // restore
        w.hit(); // breaks again
        assert!(w.just_broke);
    }

    #[test]
    fn degrade_per_hit_clamped() {
        let w = Wear::new(2.0);
        assert!((w.degrade_per_hit - 1.0).abs() < 1e-5);
        let w2 = Wear::new(-0.5);
        assert_eq!(w2.degrade_per_hit, 0.0);
    }
}
