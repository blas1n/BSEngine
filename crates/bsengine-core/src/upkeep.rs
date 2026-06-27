use bevy_ecs::prelude::Component;

/// Recurring maintenance-cost tracker. Each frame `tick()` adds
/// `cost_per_second * dt` to `deficit`; external resource systems call
/// `pay(amount)` to reduce it. If the deficit reaches `max_deficit` the
/// entity is in default and its output is reduced proportionally. When the
/// deficit returns to 0.0 the entity is fully operational again.
///
/// `pay(amount)` reduces `deficit` by `amount` (floored at 0.0). Fires
/// `just_cleared` when deficit returns to 0.0. No-op when disabled or
/// `amount <= 0`.
///
/// `tick(dt)` clears one-frame flags first; increases `deficit` by
/// `cost_per_second * dt` (capped at `max_deficit`); fires `just_defaulted`
/// on the first reach of `max_deficit`. No-op when disabled.
///
/// `is_defaulted()` returns `deficit >= max_deficit && enabled`.
///
/// `deficit_fraction()` returns `(deficit / max_deficit).clamp(0.0, 1.0)`.
///
/// `effective_output(base)` returns
/// `(base * (1.0 - penalty * deficit_fraction())).max(0.0)` when enabled;
/// returns `base` when disabled.
///
/// Distinct from `Drain` (a transferable resource-siphon debuff from another
/// entity), `Fuel` (consumable action resource), `Mana` (spellcasting pool),
/// and `Stamina` (action-gating regen resource): Upkeep models a
/// **mandatory recurring cost** — a fixed-rate obligation that degrades
/// output when not met, independent of any action.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Upkeep {
    /// Accumulated unpaid maintenance [0.0, max_deficit].
    pub deficit: f32,
    /// Maximum deficit before no further accumulation. Clamped >= 1.0.
    pub max_deficit: f32,
    /// Cost added to deficit per second. Clamped >= 0.0.
    pub cost_per_second: f32,
    /// Maximum fractional output reduction at full deficit. Clamped [0.0, 1.0].
    pub penalty: f32,
    pub just_defaulted: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Upkeep {
    pub fn new(max_deficit: f32, cost_per_second: f32, penalty: f32) -> Self {
        Self {
            deficit: 0.0,
            max_deficit: max_deficit.max(1.0),
            cost_per_second: cost_per_second.max(0.0),
            penalty: penalty.clamp(0.0, 1.0),
            just_defaulted: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Pay toward the deficit. Fires `just_cleared` when deficit reaches 0.0.
    /// No-op when disabled or `amount <= 0`.
    pub fn pay(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_positive = self.deficit > 0.0;
        self.deficit = (self.deficit - amount).max(0.0);
        if was_positive && self.deficit == 0.0 {
            self.just_cleared = true;
        }
    }

    /// Advance one frame: clear flags; accumulate deficit; fire
    /// `just_defaulted` on first reach of `max_deficit`. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_defaulted = false;
        self.just_cleared = false;

        if !self.enabled {
            return;
        }

        let was_below = self.deficit < self.max_deficit;
        self.deficit = (self.deficit + self.cost_per_second * dt).min(self.max_deficit);
        if was_below && self.deficit >= self.max_deficit {
            self.just_defaulted = true;
        }
    }

    /// `true` when the entity is in default (deficit at max) and enabled.
    pub fn is_defaulted(&self) -> bool {
        self.deficit >= self.max_deficit && self.enabled
    }

    /// Deficit as a fraction of the maximum [0.0, 1.0].
    pub fn deficit_fraction(&self) -> f32 {
        (self.deficit / self.max_deficit).clamp(0.0, 1.0)
    }

    /// Output reduced by current deficit. Returns
    /// `(base * (1.0 - penalty * deficit_fraction())).max(0.0)` when enabled;
    /// `base` when disabled.
    pub fn effective_output(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.penalty * self.deficit_fraction())).max(0.0)
    }
}

impl Default for Upkeep {
    fn default() -> Self {
        Self::new(10.0, 2.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_solvent() {
        let u = Upkeep::new(10.0, 2.0, 0.5);
        assert_eq!(u.deficit, 0.0);
        assert!(!u.is_defaulted());
        assert!(!u.just_defaulted);
    }

    #[test]
    fn tick_accumulates_deficit() {
        let mut u = Upkeep::new(10.0, 3.0, 0.5);
        u.tick(1.0);
        assert!((u.deficit - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_max_deficit() {
        let mut u = Upkeep::new(10.0, 20.0, 0.5);
        u.tick(1.0);
        assert!((u.deficit - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_defaulted_on_first_reach() {
        let mut u = Upkeep::new(10.0, 10.0, 0.5);
        u.tick(1.0);
        assert!(u.just_defaulted);
        assert!(u.is_defaulted());
    }

    #[test]
    fn tick_no_just_defaulted_when_already_at_max() {
        let mut u = Upkeep::new(10.0, 10.0, 0.5);
        u.tick(1.0); // just_defaulted fires
        u.tick(1.0); // already at max, flag cleared
        assert!(!u.just_defaulted);
    }

    #[test]
    fn tick_no_just_defaulted_below_max() {
        let mut u = Upkeep::new(10.0, 3.0, 0.5);
        u.tick(1.0); // 3.0, below max 10
        assert!(!u.just_defaulted);
    }

    #[test]
    fn tick_clears_just_defaulted_next_frame() {
        let mut u = Upkeep::new(10.0, 10.0, 0.5);
        u.tick(1.0); // just_defaulted = true
        u.tick(0.016);
        assert!(!u.just_defaulted);
    }

    #[test]
    fn tick_clears_just_cleared_next_frame() {
        let mut u = Upkeep::new(10.0, 2.0, 0.5);
        u.tick(1.0); // deficit = 2.0
        u.pay(2.0); // just_cleared = true
        u.tick(0.016);
        assert!(!u.just_cleared);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut u = Upkeep::new(10.0, 3.0, 0.5);
        u.enabled = false;
        u.tick(1.0);
        assert_eq!(u.deficit, 0.0);
    }

    #[test]
    fn pay_reduces_deficit() {
        let mut u = Upkeep::new(10.0, 5.0, 0.5);
        u.tick(1.0); // deficit = 5
        u.pay(3.0);
        assert!((u.deficit - 2.0).abs() < 1e-5);
    }

    #[test]
    fn pay_floors_at_zero() {
        let mut u = Upkeep::new(10.0, 3.0, 0.5);
        u.tick(1.0); // deficit = 3
        u.pay(10.0);
        assert_eq!(u.deficit, 0.0);
    }

    #[test]
    fn pay_fires_just_cleared_when_reaching_zero() {
        let mut u = Upkeep::new(10.0, 5.0, 0.5);
        u.tick(1.0); // deficit = 5
        u.pay(5.0);
        assert!(u.just_cleared);
        assert_eq!(u.deficit, 0.0);
    }

    #[test]
    fn pay_no_just_cleared_when_still_positive() {
        let mut u = Upkeep::new(10.0, 6.0, 0.5);
        u.tick(1.0); // deficit = 6
        u.pay(3.0); // 6 → 3, still positive
        assert!(!u.just_cleared);
    }

    #[test]
    fn pay_no_op_when_disabled() {
        let mut u = Upkeep::new(10.0, 5.0, 0.5);
        u.tick(1.0); // deficit = 5
        u.enabled = false;
        u.pay(5.0);
        assert!((u.deficit - 5.0).abs() < 1e-5);
    }

    #[test]
    fn pay_no_op_when_amount_zero() {
        let mut u = Upkeep::new(10.0, 4.0, 0.5);
        u.tick(1.0); // deficit = 4
        u.pay(0.0);
        assert!((u.deficit - 4.0).abs() < 1e-5);
    }

    #[test]
    fn pay_no_op_when_amount_negative() {
        let mut u = Upkeep::new(10.0, 4.0, 0.5);
        u.tick(1.0); // deficit = 4
        u.pay(-1.0);
        assert!((u.deficit - 4.0).abs() < 1e-5);
    }

    #[test]
    fn pay_no_op_when_already_zero() {
        let mut u = Upkeep::new(10.0, 2.0, 0.5);
        u.pay(5.0); // deficit already 0, no just_cleared
        assert!(!u.just_cleared);
        assert_eq!(u.deficit, 0.0);
    }

    #[test]
    fn is_defaulted_true_at_max() {
        let mut u = Upkeep::new(10.0, 10.0, 0.5);
        u.tick(1.0);
        assert!(u.is_defaulted());
    }

    #[test]
    fn is_defaulted_false_below_max() {
        let u = Upkeep::new(10.0, 2.0, 0.5);
        assert!(!u.is_defaulted());
    }

    #[test]
    fn is_defaulted_false_when_disabled() {
        let mut u = Upkeep::new(10.0, 10.0, 0.5);
        u.tick(1.0);
        u.enabled = false;
        assert!(!u.is_defaulted());
    }

    #[test]
    fn deficit_fraction_zero_when_solvent() {
        let u = Upkeep::new(10.0, 2.0, 0.5);
        assert_eq!(u.deficit_fraction(), 0.0);
    }

    #[test]
    fn deficit_fraction_half_at_midpoint() {
        let mut u = Upkeep::new(10.0, 5.0, 0.5);
        u.tick(1.0); // deficit = 5
        assert!((u.deficit_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn deficit_fraction_one_at_max() {
        let mut u = Upkeep::new(10.0, 10.0, 0.5);
        u.tick(1.0);
        assert!((u.deficit_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_full_when_solvent() {
        let u = Upkeep::new(10.0, 2.0, 0.5);
        assert!((u.effective_output(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_at_half_deficit() {
        let mut u = Upkeep::new(10.0, 5.0, 0.4);
        u.tick(1.0); // deficit = 5, fraction = 0.5
                     // 100 * (1 - 0.4 * 0.5) = 80
        assert!((u.effective_output(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_at_full_deficit() {
        let mut u = Upkeep::new(10.0, 10.0, 0.5);
        u.tick(1.0); // deficit = 10, fraction = 1.0
                     // 100 * (1 - 0.5 * 1.0) = 50
        assert!((u.effective_output(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_floored_at_zero() {
        let mut u = Upkeep::new(10.0, 10.0, 1.0);
        u.tick(1.0); // full deficit, full penalty
        assert_eq!(u.effective_output(100.0), 0.0);
    }

    #[test]
    fn effective_output_base_when_disabled() {
        let mut u = Upkeep::new(10.0, 10.0, 0.5);
        u.tick(1.0);
        u.enabled = false;
        assert!((u.effective_output(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn max_deficit_clamped_to_one() {
        let u = Upkeep::new(0.0, 2.0, 0.5);
        assert!((u.max_deficit - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cost_per_second_clamped_to_zero() {
        let u = Upkeep::new(10.0, -1.0, 0.5);
        assert_eq!(u.cost_per_second, 0.0);
    }

    #[test]
    fn penalty_clamped_to_one() {
        let u = Upkeep::new(10.0, 2.0, 2.0);
        assert!((u.penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn penalty_clamped_to_zero() {
        let u = Upkeep::new(10.0, 2.0, -0.5);
        assert_eq!(u.penalty, 0.0);
    }

    #[test]
    fn accumulate_then_pay_clears() {
        let mut u = Upkeep::new(10.0, 3.0, 0.5);
        u.tick(1.0); // 3.0
        u.tick(1.0); // 6.0
        u.pay(6.0);
        assert!(u.just_cleared);
        assert!(!u.is_defaulted());
    }

    #[test]
    fn partial_payment_then_default() {
        let mut u = Upkeep::new(10.0, 4.0, 0.5);
        u.tick(1.0); // 4.0
        u.pay(2.0); // 2.0
        u.tick(1.0); // 6.0
        u.tick(1.0); // 10.0 → defaulted
        assert!(u.just_defaulted);
        assert!(u.is_defaulted());
    }

    #[test]
    fn zero_cost_never_defaults() {
        let mut u = Upkeep::new(10.0, 0.0, 0.5);
        u.tick(100.0);
        assert!(!u.is_defaulted());
        assert_eq!(u.deficit, 0.0);
    }

    #[test]
    fn repeated_pay_down_to_zero() {
        let mut u = Upkeep::new(10.0, 2.0, 0.5);
        u.tick(1.0); // 2.0
        u.pay(1.0); // 1.0
        u.pay(1.0); // 0.0, just_cleared
        assert!(u.just_cleared);
    }
}
