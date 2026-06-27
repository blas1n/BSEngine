use bevy_ecs::prelude::Component;

/// Generalised suffering accumulator. Multiple systems (status effects,
/// environmental hazards, narrative events) call `afflict(amount)` to stack
/// woe on an entity; the level decays passively each frame and can be
/// alleviated manually with `soothe(amount)`. Output is reduced
/// proportionally to the current woe fraction.
///
/// `afflict(amount)` increases `woe_level` by `amount` (capped at
/// `max_woe`). Fires `just_afflicted` on the first transition from 0.0 to
/// positive. No-op when disabled or `amount <= 0`.
///
/// `soothe(amount)` decreases `woe_level` by `amount` (floored at 0.0).
/// Fires `just_overcome` when the level returns to 0.0. No-op when disabled
/// or `amount <= 0`.
///
/// `tick(dt)` clears one-frame flags first; decays `woe_level` by
/// `decay_rate * dt` (floored at 0.0); fires `just_overcome` when passive
/// decay reaches 0.0. No-op when disabled.
///
/// `is_suffering()` returns `woe_level > 0.0 && enabled`.
///
/// `woe_fraction()` returns `(woe_level / max_woe).clamp(0.0, 1.0)`.
///
/// `effective_output(base)` returns
/// `(base * (1.0 - suffering_penalty * woe_fraction())).max(0.0)` when
/// enabled; returns `base` when disabled. Pure query.
///
/// Distinct from `Unrest` (civic/morale threshold that fires when restless),
/// `Dread` (fear escalation), `Curse` (supernatural debuff with specific
/// triggers), `Lament` (grief-regen interaction), and `Doom` (terminal
/// countdown): Woe is an **aggregated general-purpose suffering level** — any
/// number of unrelated sources add to the same pool, which decays uniformly
/// and penalises output continuously.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Woe {
    /// Current suffering level [0.0, max_woe].
    pub woe_level: f32,
    /// Maximum suffering level. Clamped >= 1.0.
    pub max_woe: f32,
    /// Passive decay per second. Clamped >= 0.0.
    pub decay_rate: f32,
    /// Maximum fractional output reduction at full woe. Clamped [0.0, 1.0].
    pub suffering_penalty: f32,
    pub just_afflicted: bool,
    pub just_overcome: bool,
    pub enabled: bool,
}

impl Woe {
    pub fn new(max_woe: f32, decay_rate: f32, suffering_penalty: f32) -> Self {
        Self {
            woe_level: 0.0,
            max_woe: max_woe.max(1.0),
            decay_rate: decay_rate.max(0.0),
            suffering_penalty: suffering_penalty.clamp(0.0, 1.0),
            just_afflicted: false,
            just_overcome: false,
            enabled: true,
        }
    }

    /// Add woe from any source. Fires `just_afflicted` on first positive
    /// transition from 0.0. No-op when disabled or `amount <= 0`.
    pub fn afflict(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_zero = self.woe_level == 0.0;
        self.woe_level = (self.woe_level + amount).min(self.max_woe);
        if was_zero && self.woe_level > 0.0 {
            self.just_afflicted = true;
        }
    }

    /// Reduce woe manually. Fires `just_overcome` when level reaches 0.0.
    /// No-op when disabled or `amount <= 0`.
    pub fn soothe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_positive = self.woe_level > 0.0;
        self.woe_level = (self.woe_level - amount).max(0.0);
        if was_positive && self.woe_level == 0.0 {
            self.just_overcome = true;
        }
    }

    /// Advance one frame: clear flags; apply passive decay; fire
    /// `just_overcome` when decay reaches 0.0. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_afflicted = false;
        self.just_overcome = false;

        if !self.enabled {
            return;
        }

        if self.woe_level > 0.0 && self.decay_rate > 0.0 {
            let was_positive = self.woe_level > 0.0;
            self.woe_level = (self.woe_level - self.decay_rate * dt).max(0.0);
            if was_positive && self.woe_level == 0.0 {
                self.just_overcome = true;
            }
        }
    }

    /// `true` when the entity is suffering (woe_level > 0.0) and enabled.
    pub fn is_suffering(&self) -> bool {
        self.woe_level > 0.0 && self.enabled
    }

    /// Woe as a fraction of the maximum [0.0, 1.0].
    pub fn woe_fraction(&self) -> f32 {
        (self.woe_level / self.max_woe).clamp(0.0, 1.0)
    }

    /// Output reduced by current woe. Pure query.
    pub fn effective_output(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.suffering_penalty * self.woe_fraction())).max(0.0)
    }
}

impl Default for Woe {
    fn default() -> Self {
        Self::new(10.0, 1.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_peace() {
        let w = Woe::new(10.0, 1.0, 0.5);
        assert_eq!(w.woe_level, 0.0);
        assert!(!w.is_suffering());
        assert!(!w.just_afflicted);
    }

    #[test]
    fn afflict_increases_level() {
        let mut w = Woe::new(10.0, 1.0, 0.5);
        w.afflict(4.0);
        assert!((w.woe_level - 4.0).abs() < 1e-5);
    }

    #[test]
    fn afflict_caps_at_max_woe() {
        let mut w = Woe::new(10.0, 1.0, 0.5);
        w.afflict(20.0);
        assert!((w.woe_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn afflict_fires_just_afflicted_from_zero() {
        let mut w = Woe::new(10.0, 1.0, 0.5);
        w.afflict(3.0);
        assert!(w.just_afflicted);
        assert!(w.is_suffering());
    }

    #[test]
    fn afflict_no_just_afflicted_when_already_positive() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(3.0); // just_afflicted fires
        w.tick(0.016); // clear flags
        w.afflict(2.0); // already positive, no re-fire
        assert!(!w.just_afflicted);
    }

    #[test]
    fn afflict_no_op_when_disabled() {
        let mut w = Woe::new(10.0, 1.0, 0.5);
        w.enabled = false;
        w.afflict(5.0);
        assert_eq!(w.woe_level, 0.0);
    }

    #[test]
    fn afflict_no_op_when_amount_zero() {
        let mut w = Woe::new(10.0, 1.0, 0.5);
        w.afflict(0.0);
        assert_eq!(w.woe_level, 0.0);
    }

    #[test]
    fn afflict_no_op_when_amount_negative() {
        let mut w = Woe::new(10.0, 1.0, 0.5);
        w.afflict(-2.0);
        assert_eq!(w.woe_level, 0.0);
    }

    #[test]
    fn soothe_decreases_level() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(8.0);
        w.soothe(3.0);
        assert!((w.woe_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn soothe_floors_at_zero() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(4.0);
        w.soothe(10.0);
        assert_eq!(w.woe_level, 0.0);
    }

    #[test]
    fn soothe_fires_just_overcome_when_reaching_zero() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(6.0);
        w.tick(0.016); // clear flags
        w.soothe(6.0);
        assert!(w.just_overcome);
        assert!(!w.is_suffering());
    }

    #[test]
    fn soothe_no_just_overcome_when_still_positive() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(8.0);
        w.tick(0.016);
        w.soothe(3.0); // 8 → 5, still positive
        assert!(!w.just_overcome);
    }

    #[test]
    fn soothe_no_op_when_disabled() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(8.0);
        w.enabled = false;
        w.soothe(5.0);
        assert!((w.woe_level - 8.0).abs() < 1e-5);
    }

    #[test]
    fn soothe_no_op_when_amount_zero() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(5.0);
        w.soothe(0.0);
        assert!((w.woe_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn soothe_no_op_when_amount_negative() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(5.0);
        w.soothe(-1.0);
        assert!((w.woe_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_woe_level() {
        let mut w = Woe::new(10.0, 2.0, 0.5);
        w.afflict(8.0);
        w.tick(1.0); // -2.0 = 6.0
        assert!((w.woe_level - 6.0).abs() < 1e-5);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut w = Woe::new(10.0, 10.0, 0.5);
        w.afflict(3.0);
        w.tick(1.0); // -10 → 0
        assert_eq!(w.woe_level, 0.0);
    }

    #[test]
    fn tick_fires_just_overcome_when_decay_reaches_zero() {
        let mut w = Woe::new(10.0, 5.0, 0.5);
        w.afflict(3.0);
        w.tick(0.016); // clear flags, small decay
        w.tick(1.0); // -5 → 0
        assert!(w.just_overcome);
        assert!(!w.is_suffering());
    }

    #[test]
    fn tick_no_just_overcome_when_not_at_zero() {
        let mut w = Woe::new(10.0, 1.0, 0.5);
        w.afflict(8.0);
        w.tick(0.016);
        w.tick(1.0); // 8 - 1 = 7, not zero
        assert!(!w.just_overcome);
    }

    #[test]
    fn tick_clears_just_afflicted() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(5.0); // just_afflicted = true
        w.tick(0.016);
        assert!(!w.just_afflicted);
    }

    #[test]
    fn tick_clears_just_overcome() {
        let mut w = Woe::new(10.0, 10.0, 0.5);
        w.afflict(2.0);
        w.tick(0.016);
        w.tick(1.0); // just_overcome = true
        w.tick(0.016);
        assert!(!w.just_overcome);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = Woe::new(10.0, 5.0, 0.5);
        w.afflict(8.0);
        w.enabled = false;
        w.tick(2.0);
        assert!((w.woe_level - 8.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_change_when_already_zero() {
        let mut w = Woe::new(10.0, 5.0, 0.5);
        w.tick(1.0); // nothing to decay
        assert_eq!(w.woe_level, 0.0);
        assert!(!w.just_overcome);
    }

    #[test]
    fn is_suffering_true_when_positive() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(1.0);
        assert!(w.is_suffering());
    }

    #[test]
    fn is_suffering_false_at_zero() {
        let w = Woe::new(10.0, 0.0, 0.5);
        assert!(!w.is_suffering());
    }

    #[test]
    fn is_suffering_false_when_disabled() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(5.0);
        w.enabled = false;
        assert!(!w.is_suffering());
    }

    #[test]
    fn woe_fraction_zero_at_start() {
        let w = Woe::new(10.0, 0.0, 0.5);
        assert_eq!(w.woe_fraction(), 0.0);
    }

    #[test]
    fn woe_fraction_half_at_midpoint() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(5.0);
        assert!((w.woe_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn woe_fraction_one_at_max() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(10.0);
        assert!((w.woe_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_full_at_peace() {
        let w = Woe::new(10.0, 0.0, 0.5);
        assert!((w.effective_output(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_at_half_woe() {
        let mut w = Woe::new(10.0, 0.0, 0.4);
        w.afflict(5.0); // 50% woe
                        // 100 * (1 - 0.4 * 0.5) = 80
        assert!((w.effective_output(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_at_full_woe() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(10.0); // 100% woe
                         // 100 * (1 - 0.5 * 1.0) = 50
        assert!((w.effective_output(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_floored_at_zero() {
        let mut w = Woe::new(10.0, 0.0, 1.0);
        w.afflict(10.0); // full penalty
        assert_eq!(w.effective_output(100.0), 0.0);
    }

    #[test]
    fn effective_output_base_when_disabled() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(10.0);
        w.enabled = false;
        assert!((w.effective_output(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn max_woe_clamped_to_one() {
        let w = Woe::new(0.0, 1.0, 0.5);
        assert!((w.max_woe - 1.0).abs() < 1e-5);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let w = Woe::new(10.0, -1.0, 0.5);
        assert_eq!(w.decay_rate, 0.0);
    }

    #[test]
    fn suffering_penalty_clamped_to_one() {
        let w = Woe::new(10.0, 1.0, 2.0);
        assert!((w.suffering_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn suffering_penalty_clamped_to_zero() {
        let w = Woe::new(10.0, 1.0, -0.5);
        assert_eq!(w.suffering_penalty, 0.0);
    }

    #[test]
    fn multiple_sources_stack() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(2.0);
        w.afflict(3.0);
        w.afflict(1.5);
        assert!((w.woe_level - 6.5).abs() < 1e-4);
        assert!(w.is_suffering());
    }

    #[test]
    fn afflict_soothe_cycle() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(7.0); // just_afflicted
        w.tick(0.016); // clear
        w.soothe(7.0); // just_overcome
        assert!(w.just_overcome);
        w.tick(0.016); // clear
        w.afflict(4.0); // just_afflicted again
        assert!(w.just_afflicted);
    }

    #[test]
    fn zero_decay_holds_level() {
        let mut w = Woe::new(10.0, 0.0, 0.5);
        w.afflict(5.0);
        w.tick(100.0); // no decay
        assert!((w.woe_level - 5.0).abs() < 1e-4);
    }
}
