use bevy_ecs::prelude::Component;

/// Kill-value accumulator that fires a reward trigger once enough value is
/// collected. External systems feed `trove_value` via `add_value(v)`; once
/// `trove_value >= threshold` the component is considered to "have treasure"
/// (`has_treasure()` returns `true` and `just_activated` fires). Consuming
/// the treasure via `consume()` increments `trove_count` and resets the
/// value for the next cycle.
///
/// `add_value(v)` increases `trove_value` by `v`; fires `just_activated` on
/// the first reach of `threshold` (does not re-fire while already active).
/// No-op when disabled or `v <= 0`.
///
/// `consume()` resets `trove_value` to 0.0 and increments `trove_count`.
/// No-op when there is no treasure (`!has_treasure()`) or disabled.
///
/// `tick(dt)` clears `just_activated`. No-op when disabled.
///
/// `has_treasure()` returns `trove_value >= threshold && enabled`.
///
/// `fill_fraction()` returns `(trove_value / threshold).clamp(0.0, 1.0)` —
/// how full the current accumulation cycle is.
///
/// `effective_reward(base)` returns `base * reward_multiplier` when
/// `has_treasure()`; `base` otherwise (pure query, does not consume).
///
/// Distinct from `Combo` (rapid-hit multiplier that decays on inactivity),
/// `Greed` (gold/resource magnet with radius), `Leech` (incoming-damage
/// converts to health), and `Loot Table` (probability table for item drops):
/// Trove is a **threshold-based accumulator** — value feeds in freely and the
/// bonus only fires once per cycle when the threshold is crossed, giving
/// systems a clean "charged and ready" signal rather than a continuous buff.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Trove {
    /// Accumulated value in the current cycle.
    pub trove_value: f32,
    /// Value required to trigger the treasure bonus. Clamped >= 1.0.
    pub threshold: f32,
    /// Multiplier applied to rewards when treasure is available. Clamped >= 0.0.
    pub reward_multiplier: f32,
    /// Number of times treasure has been consumed.
    pub trove_count: u32,
    pub just_activated: bool,
    pub enabled: bool,
}

impl Trove {
    pub fn new(threshold: f32, reward_multiplier: f32) -> Self {
        Self {
            trove_value: 0.0,
            threshold: threshold.max(1.0),
            reward_multiplier: reward_multiplier.max(0.0),
            trove_count: 0,
            just_activated: false,
            enabled: true,
        }
    }

    /// Feed `v` value into the accumulator. Fires `just_activated` on the
    /// first tick that `trove_value` reaches `threshold`. No-op when disabled
    /// or `v <= 0`.
    pub fn add_value(&mut self, v: f32) {
        if !self.enabled || v <= 0.0 {
            return;
        }
        let was_below = self.trove_value < self.threshold;
        self.trove_value += v;
        if was_below && self.trove_value >= self.threshold {
            self.just_activated = true;
        }
    }

    /// Consume the accumulated treasure: reset `trove_value` to 0.0 and
    /// increment `trove_count`. No-op when `!has_treasure()` or disabled.
    pub fn consume(&mut self) {
        if !self.enabled || !self.has_treasure() {
            return;
        }
        self.trove_value = 0.0;
        self.trove_count += 1;
    }

    /// Clear `just_activated`. No-op when disabled.
    pub fn tick(&mut self, _dt: f32) {
        if !self.enabled {
            return;
        }
        self.just_activated = false;
    }

    /// `true` when `trove_value >= threshold` and component is enabled.
    pub fn has_treasure(&self) -> bool {
        self.trove_value >= self.threshold && self.enabled
    }

    /// Progress toward the next threshold [0.0, 1.0].
    pub fn fill_fraction(&self) -> f32 {
        (self.trove_value / self.threshold).clamp(0.0, 1.0)
    }

    /// Pure query: returns `base * reward_multiplier` when treasure is
    /// available; `base` otherwise. Does not consume or mutate state.
    pub fn effective_reward(&self, base: f32) -> f32 {
        if self.has_treasure() {
            base * self.reward_multiplier
        } else {
            base
        }
    }
}

impl Default for Trove {
    fn default() -> Self {
        Self::new(10.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let t = Trove::new(10.0, 2.0);
        assert_eq!(t.trove_value, 0.0);
        assert_eq!(t.trove_count, 0);
        assert!(!t.just_activated);
        assert!(!t.has_treasure());
    }

    #[test]
    fn add_value_increases_trove_value() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(3.0);
        assert!((t.trove_value - 3.0).abs() < 1e-5);
    }

    #[test]
    fn add_value_fires_just_activated_on_threshold() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(10.0);
        assert!(t.just_activated);
        assert!(t.has_treasure());
    }

    #[test]
    fn add_value_no_just_activated_before_threshold() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(5.0);
        assert!(!t.just_activated);
    }

    #[test]
    fn add_value_no_just_activated_when_already_active() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(10.0);
        t.tick(0.016); // clear flag
        t.add_value(5.0); // already above threshold
        assert!(!t.just_activated);
    }

    #[test]
    fn add_value_no_op_when_disabled() {
        let mut t = Trove::new(10.0, 2.0);
        t.enabled = false;
        t.add_value(10.0);
        assert_eq!(t.trove_value, 0.0);
        assert!(!t.just_activated);
    }

    #[test]
    fn add_value_no_op_when_value_zero() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(0.0);
        assert_eq!(t.trove_value, 0.0);
    }

    #[test]
    fn add_value_no_op_when_value_negative() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(-5.0);
        assert_eq!(t.trove_value, 0.0);
    }

    #[test]
    fn add_value_accumulates_across_calls() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(3.0);
        t.add_value(4.0);
        t.add_value(3.0);
        assert!((t.trove_value - 10.0).abs() < 1e-5);
        assert!(t.just_activated);
    }

    #[test]
    fn consume_resets_value() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(10.0);
        t.consume();
        assert_eq!(t.trove_value, 0.0);
    }

    #[test]
    fn consume_increments_trove_count() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(10.0);
        t.consume();
        assert_eq!(t.trove_count, 1);
    }

    #[test]
    fn consume_multiple_times_accumulates_count() {
        let mut t = Trove::new(5.0, 2.0);
        for _ in 0..3 {
            t.add_value(5.0);
            t.consume();
        }
        assert_eq!(t.trove_count, 3);
    }

    #[test]
    fn consume_no_op_when_no_treasure() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(5.0); // below threshold
        t.consume();
        assert_eq!(t.trove_count, 0);
        assert!((t.trove_value - 5.0).abs() < 1e-5);
    }

    #[test]
    fn consume_no_op_when_disabled() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(10.0);
        t.enabled = false;
        t.consume();
        assert!((t.trove_value - 10.0).abs() < 1e-5); // value unchanged
        assert_eq!(t.trove_count, 0);
    }

    #[test]
    fn tick_clears_just_activated() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(10.0);
        assert!(t.just_activated);
        t.tick(0.016);
        assert!(!t.just_activated);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut t = Trove::new(10.0, 2.0);
        t.just_activated = true; // manually set
        t.enabled = false;
        t.tick(0.016);
        assert!(t.just_activated); // not cleared
    }

    #[test]
    fn has_treasure_true_at_threshold() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(10.0);
        assert!(t.has_treasure());
    }

    #[test]
    fn has_treasure_true_above_threshold() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(15.0);
        assert!(t.has_treasure());
    }

    #[test]
    fn has_treasure_false_below_threshold() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(9.9);
        assert!(!t.has_treasure());
    }

    #[test]
    fn has_treasure_false_when_disabled() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(10.0);
        t.enabled = false;
        assert!(!t.has_treasure());
    }

    #[test]
    fn fill_fraction_zero_when_empty() {
        let t = Trove::new(10.0, 2.0);
        assert_eq!(t.fill_fraction(), 0.0);
    }

    #[test]
    fn fill_fraction_half_at_midpoint() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(5.0);
        assert!((t.fill_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn fill_fraction_one_at_threshold() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(10.0);
        assert!((t.fill_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn fill_fraction_clamped_above_threshold() {
        let mut t = Trove::new(10.0, 2.0);
        t.add_value(20.0);
        assert!((t.fill_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_reward_multiplied_with_treasure() {
        let mut t = Trove::new(10.0, 3.0);
        t.add_value(10.0);
        assert!((t.effective_reward(50.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_reward_base_without_treasure() {
        let t = Trove::new(10.0, 3.0);
        assert!((t.effective_reward(50.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn effective_reward_base_when_disabled() {
        let mut t = Trove::new(10.0, 3.0);
        t.add_value(10.0);
        t.enabled = false;
        assert!((t.effective_reward(50.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn effective_reward_does_not_consume() {
        let mut t = Trove::new(10.0, 3.0);
        t.add_value(10.0);
        t.effective_reward(50.0);
        assert!(t.has_treasure()); // still has treasure
        assert_eq!(t.trove_count, 0);
    }

    #[test]
    fn threshold_clamped_to_one() {
        let t = Trove::new(0.0, 2.0);
        assert!((t.threshold - 1.0).abs() < 1e-5);
    }

    #[test]
    fn reward_multiplier_clamped_to_zero() {
        let t = Trove::new(10.0, -1.0);
        assert_eq!(t.reward_multiplier, 0.0);
    }

    #[test]
    fn full_cycle_consume_then_refill() {
        let mut t = Trove::new(5.0, 2.0);
        t.add_value(5.0);
        assert!(t.has_treasure());
        t.consume();
        assert!(!t.has_treasure());
        assert_eq!(t.trove_count, 1);
        t.add_value(5.0);
        assert!(t.has_treasure());
        assert!(t.just_activated);
    }
}
