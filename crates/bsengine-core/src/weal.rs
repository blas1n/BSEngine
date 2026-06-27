use bevy_ecs::prelude::Component;

/// Well-being accumulator that grows passively through regeneration and
/// through explicit nourishment, then scales output upward proportionally.
/// Designed as the positive counterpart to `Woe` (suffering that reduces
/// output): Weal enhances an entity's effectiveness as it flourishes.
///
/// `nourish(amount)` increases `weal_level` by `amount` (capped at
/// `max_weal`), firing `just_flourished` when first reaching `max_weal`
/// from below. No-op when disabled or `amount <= 0.0`.
///
/// `deplete(amount)` decreases `weal_level` by `amount` (floored 0), firing
/// `just_diminished` when falling from `max_weal` to below. No-op when
/// disabled or `amount <= 0.0`.
///
/// `tick(dt)` clears one-frame flags, then increases `weal_level` by
/// `regen_rate * dt` (capped at `max_weal`), firing `just_flourished` on
/// first reach of `max_weal`. No-op when disabled.
///
/// `is_flourishing()` returns `weal_level >= max_weal && enabled`.
///
/// `weal_fraction()` returns `(weal_level / max_weal).clamp(0.0, 1.0)`.
///
/// `effective_output(base)` returns
/// `base * (1.0 + bonus * weal_fraction())` when enabled; returns `base`
/// unchanged otherwise.
///
/// Distinct from `Regen` (health point recovery per second), `Morale`
/// (combat-cohesion modifier driven by battle events), and `Stamina`
/// (action-budget resource that depletes on use): Weal models **generalised
/// entity well-being** — a slow-building, passive quality-of-life reserve
/// that amplifies all output proportionally, pairing naturally with `Woe`
/// as the two poles of an entity's condition.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weal {
    /// Current well-being [0.0, max_weal].
    pub weal_level: f32,
    /// Maximum well-being. Clamped >= 1.0.
    pub max_weal: f32,
    /// Passive well-being recovery per second. Clamped >= 0.0.
    pub regen_rate: f32,
    /// Output amplification at full well-being. Clamped >= 0.0.
    pub bonus: f32,
    pub just_flourished: bool,
    pub just_diminished: bool,
    pub enabled: bool,
}

impl Weal {
    pub fn new(max_weal: f32, regen_rate: f32, bonus: f32) -> Self {
        Self {
            weal_level: 0.0,
            max_weal: max_weal.max(1.0),
            regen_rate: regen_rate.max(0.0),
            bonus: bonus.max(0.0),
            just_flourished: false,
            just_diminished: false,
            enabled: true,
        }
    }

    /// Add `amount` to well-being (cap `max_weal`). Fires `just_flourished`
    /// on first reach of `max_weal`. No-op when disabled or `amount <= 0.0`.
    pub fn nourish(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below_max = self.weal_level < self.max_weal;
        self.weal_level = (self.weal_level + amount).min(self.max_weal);
        if was_below_max && self.weal_level >= self.max_weal {
            self.just_flourished = true;
        }
    }

    /// Subtract `amount` from well-being (floor 0). Fires `just_diminished`
    /// when falling from `max_weal`. No-op when disabled or `amount <= 0.0`.
    pub fn deplete(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_at_max = self.weal_level >= self.max_weal;
        self.weal_level = (self.weal_level - amount).max(0.0);
        if was_at_max && self.weal_level < self.max_weal {
            self.just_diminished = true;
        }
    }

    /// Advance one frame: clear flags, then regenerate well-being. No-op when
    /// disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_flourished = false;
        self.just_diminished = false;

        if !self.enabled {
            return;
        }
        let was_below_max = self.weal_level < self.max_weal;
        self.weal_level = (self.weal_level + self.regen_rate * dt).min(self.max_weal);
        if was_below_max && self.weal_level >= self.max_weal {
            self.just_flourished = true;
        }
    }

    /// `true` when well-being is at maximum and the component is enabled.
    pub fn is_flourishing(&self) -> bool {
        self.weal_level >= self.max_weal && self.enabled
    }

    /// Well-being as a fraction of maximum [0.0, 1.0].
    pub fn weal_fraction(&self) -> f32 {
        (self.weal_level / self.max_weal).clamp(0.0, 1.0)
    }

    /// Scale output `base` by current well-being. Returns
    /// `base * (1 + bonus * fraction)` when enabled; `base` otherwise.
    pub fn effective_output(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.bonus * self.weal_fraction())
    }
}

impl Default for Weal {
    fn default() -> Self {
        Self::new(10.0, 1.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Weal {
        Weal::new(10.0, 2.0, 0.5)
    }

    #[test]
    fn new_starts_empty() {
        let w = Weal::new(10.0, 2.0, 0.5);
        assert_eq!(w.weal_level, 0.0);
        assert!(!w.just_flourished);
        assert!(!w.just_diminished);
        assert!(!w.is_flourishing());
    }

    // --- nourish ---

    #[test]
    fn nourish_increases_weal() {
        let mut w = w();
        w.nourish(4.0);
        assert!((w.weal_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn nourish_caps_at_max() {
        let mut w = w();
        w.nourish(100.0);
        assert!((w.weal_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn nourish_fires_just_flourished_on_first_max() {
        let mut w = w();
        w.nourish(10.0);
        assert!(w.just_flourished);
    }

    #[test]
    fn nourish_no_just_flourished_when_already_at_max() {
        let mut w = w();
        w.nourish(10.0); // just_flourished fires
        w.nourish(1.0); // already at max
                        // flag is NOT cleared by nourish — still from first call
        assert!(w.just_flourished);
    }

    #[test]
    fn nourish_no_just_flourished_below_max() {
        let mut w = w();
        w.nourish(5.0); // below max
        assert!(!w.just_flourished);
    }

    #[test]
    fn nourish_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.nourish(5.0);
        assert_eq!(w.weal_level, 0.0);
    }

    #[test]
    fn nourish_no_op_when_zero() {
        let mut w = w();
        w.nourish(0.0);
        assert_eq!(w.weal_level, 0.0);
    }

    #[test]
    fn nourish_no_op_when_negative() {
        let mut w = w();
        w.nourish(-3.0);
        assert_eq!(w.weal_level, 0.0);
    }

    // --- deplete ---

    #[test]
    fn deplete_decreases_weal() {
        let mut w = w();
        w.nourish(8.0);
        w.deplete(3.0);
        assert!((w.weal_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn deplete_floors_at_zero() {
        let mut w = w();
        w.nourish(5.0);
        w.deplete(100.0);
        assert_eq!(w.weal_level, 0.0);
    }

    #[test]
    fn deplete_fires_just_diminished_from_max() {
        let mut w = w();
        w.nourish(10.0); // at max
        w.deplete(1.0);
        assert!(w.just_diminished);
    }

    #[test]
    fn deplete_no_just_diminished_when_below_max() {
        let mut w = w();
        w.nourish(7.0); // below max
        w.deplete(2.0);
        assert!(!w.just_diminished);
    }

    #[test]
    fn deplete_no_op_when_disabled() {
        let mut w = w();
        w.nourish(5.0);
        w.enabled = false;
        w.deplete(3.0);
        assert!((w.weal_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn deplete_no_op_when_zero() {
        let mut w = w();
        w.nourish(5.0);
        w.deplete(0.0);
        assert!((w.weal_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn deplete_no_op_when_negative() {
        let mut w = w();
        w.nourish(5.0);
        w.deplete(-2.0);
        assert!((w.weal_level - 5.0).abs() < 1e-4);
    }

    // --- tick ---

    #[test]
    fn tick_regenerates_weal() {
        let mut w = w(); // regen_rate = 2.0
        w.tick(1.0); // 2.0 * 1.0 = 2.0
        assert!((w.weal_level - 2.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max() {
        let mut w = w();
        w.tick(100.0); // 2.0 * 100 → capped at 10
        assert!((w.weal_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_flourished_on_reaching_max() {
        let mut w = w();
        w.tick(5.0); // 2.0 * 5 = 10.0 → max
        assert!(w.just_flourished);
    }

    #[test]
    fn tick_no_just_flourished_when_already_at_max() {
        let mut w = w();
        w.tick(5.0); // just_flourished fires
        w.tick(0.016); // cleared; at max, no re-fire
        assert!(!w.just_flourished);
    }

    #[test]
    fn tick_no_just_flourished_when_below_max() {
        let mut w = w();
        w.tick(1.0); // 2.0 < 10.0
        assert!(!w.just_flourished);
    }

    #[test]
    fn tick_clears_flags() {
        let mut w = w();
        w.nourish(10.0); // just_flourished
        w.tick(0.016);
        assert!(!w.just_flourished);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(5.0);
        assert_eq!(w.weal_level, 0.0);
    }

    // --- is_flourishing ---

    #[test]
    fn is_flourishing_true_at_max() {
        let mut w = w();
        w.nourish(10.0);
        assert!(w.is_flourishing());
    }

    #[test]
    fn is_flourishing_false_below_max() {
        let mut w = w();
        w.nourish(5.0);
        assert!(!w.is_flourishing());
    }

    #[test]
    fn is_flourishing_false_when_disabled() {
        let mut w = w();
        w.nourish(10.0);
        w.enabled = false;
        assert!(!w.is_flourishing());
    }

    // --- weal_fraction ---

    #[test]
    fn weal_fraction_zero_when_empty() {
        let w = w();
        assert_eq!(w.weal_fraction(), 0.0);
    }

    #[test]
    fn weal_fraction_half_at_midpoint() {
        let mut w = w();
        w.nourish(5.0); // 5.0 / 10.0 = 0.5
        assert!((w.weal_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn weal_fraction_one_at_max() {
        let mut w = w();
        w.nourish(10.0);
        assert!((w.weal_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_output ---

    #[test]
    fn effective_output_base_when_empty() {
        let w = w(); // bonus = 0.5, fraction = 0
        assert!((w.effective_output(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_output_boosted_at_half_weal() {
        let mut w = Weal::new(10.0, 2.0, 0.5);
        w.nourish(5.0); // fraction = 0.5
                        // 100 * (1 + 0.5*0.5) = 125
        assert!((w.effective_output(100.0) - 125.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_fully_boosted_at_max() {
        let mut w = Weal::new(10.0, 2.0, 0.5);
        w.nourish(10.0); // fraction = 1.0
                         // 100 * (1 + 0.5*1.0) = 150
        assert!((w.effective_output(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_passthrough_when_disabled() {
        let mut w = w();
        w.nourish(10.0);
        w.enabled = false;
        assert!((w.effective_output(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_weal_clamped_to_one() {
        let w = Weal::new(0.0, 2.0, 0.5);
        assert!((w.max_weal - 1.0).abs() < 1e-5);
    }

    #[test]
    fn regen_rate_clamped_to_zero() {
        let w = Weal::new(10.0, -2.0, 0.5);
        assert_eq!(w.regen_rate, 0.0);
    }

    #[test]
    fn bonus_clamped_to_zero() {
        let w = Weal::new(10.0, 2.0, -0.5);
        assert_eq!(w.bonus, 0.0);
    }

    // --- integration scenarios ---

    #[test]
    fn nourish_deplete_cycle_at_max() {
        let mut w = w();
        w.nourish(10.0); // max, just_flourished
        assert!(w.just_flourished);
        w.deplete(1.0); // 9.0, just_diminished
        assert!(w.just_diminished);
        w.nourish(1.0); // 10.0, just_flourished again
        assert!(w.just_flourished);
    }

    #[test]
    fn regen_to_max_then_deplete_then_regen() {
        let mut w = w(); // regen=2, max=10
        w.tick(5.0); // 10.0 max
        assert!(w.is_flourishing());
        w.tick(0.016); // stays max, flag cleared
        w.deplete(4.0); // 6.0
        assert!(!w.is_flourishing());
        w.tick(2.0); // 6.0 + 4.0 = 10.0, just_flourished
        assert!(w.just_flourished);
    }

    #[test]
    fn effective_output_scales_with_large_bonus() {
        let mut w = Weal::new(10.0, 2.0, 2.0); // 200% bonus at full
        w.nourish(10.0); // fraction = 1.0
                         // 50 * (1 + 2.0*1.0) = 50 * 3.0 = 150
        assert!((w.effective_output(50.0) - 150.0).abs() < 1e-3);
    }
}
