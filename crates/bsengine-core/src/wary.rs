use bevy_ecs::prelude::Component;

/// Threat-awareness tracker that builds as the entity detects danger and
/// decays passively. Above `threshold` the entity is "wary", gaining an
/// evasion bonus scaled to the current wariness fraction.
///
/// `alert(amount)` increases `wary_level` by `amount` (capped at
/// `max_wary`). Fires `just_alerted` on the first upward crossing of
/// `threshold`. No-op when disabled or `amount <= 0`.
///
/// `settle(amount)` decreases `wary_level` by `amount` (floored at 0.0).
/// Fires `just_calmed` when the level drops below `threshold`. No-op when
/// disabled or `amount <= 0`.
///
/// `tick(dt)` clears one-frame flags first; decays `wary_level` by
/// `decay_rate * dt` (floored at 0.0); fires `just_calmed` when passive
/// decay crosses below `threshold`. No-op when disabled.
///
/// `is_wary()` returns `wary_level >= threshold && enabled`.
///
/// `wary_fraction()` returns `(wary_level / max_wary).clamp(0.0, 1.0)`.
///
/// `effective_evasion(base)` returns
/// `(base + evasion_bonus * wary_fraction()).clamp(0.0, 1.0)` when enabled;
/// returns `base` when disabled. Pure query.
///
/// Distinct from `Alarm` (triggered alert that broadcasts to nearby
/// entities), `Fear` (involuntary flee response), `Stealth` (entity hiding
/// from others), and `Notice` (detection range for spotting threats): Wary
/// models the entity's **internal threat-awareness level** — a continuous
/// vigilance state that improves defensive reflexes proportionally.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wary {
    /// Current vigilance level [0.0, max_wary].
    pub wary_level: f32,
    /// Maximum vigilance level. Clamped >= 1.0.
    pub max_wary: f32,
    /// Level at which the entity is considered wary. Clamped [0.0, max_wary].
    pub threshold: f32,
    /// Passive vigilance decay per second. Clamped >= 0.0.
    pub decay_rate: f32,
    /// Additional evasion at full wariness [0.0, 1.0].
    pub evasion_bonus: f32,
    pub just_alerted: bool,
    pub just_calmed: bool,
    pub enabled: bool,
}

impl Wary {
    pub fn new(max_wary: f32, threshold: f32, decay_rate: f32, evasion_bonus: f32) -> Self {
        let max_wary = max_wary.max(1.0);
        Self {
            wary_level: 0.0,
            max_wary,
            threshold: threshold.clamp(0.0, max_wary),
            decay_rate: decay_rate.max(0.0),
            evasion_bonus: evasion_bonus.clamp(0.0, 1.0),
            just_alerted: false,
            just_calmed: false,
            enabled: true,
        }
    }

    /// Raise vigilance by `amount`. Fires `just_alerted` on first upward
    /// threshold crossing. No-op when disabled or `amount <= 0`.
    pub fn alert(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.wary_level < self.threshold;
        self.wary_level = (self.wary_level + amount).min(self.max_wary);
        if was_below && self.wary_level >= self.threshold {
            self.just_alerted = true;
        }
    }

    /// Lower vigilance by `amount`. Fires `just_calmed` when dropping below
    /// `threshold`. No-op when disabled or `amount <= 0`.
    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_wary = self.wary_level >= self.threshold;
        self.wary_level = (self.wary_level - amount).max(0.0);
        if was_wary && self.wary_level < self.threshold {
            self.just_calmed = true;
        }
    }

    /// Advance one frame: clear flags, apply passive decay, fire `just_calmed`
    /// when decay crosses below `threshold`. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_alerted = false;
        self.just_calmed = false;

        if !self.enabled {
            return;
        }

        if self.wary_level > 0.0 && self.decay_rate > 0.0 {
            let was_wary = self.wary_level >= self.threshold;
            self.wary_level = (self.wary_level - self.decay_rate * dt).max(0.0);
            if was_wary && self.wary_level < self.threshold {
                self.just_calmed = true;
            }
        }
    }

    /// `true` when the entity is vigilant (wary_level >= threshold) and enabled.
    pub fn is_wary(&self) -> bool {
        self.wary_level >= self.threshold && self.enabled
    }

    /// Vigilance as a fraction of the maximum [0.0, 1.0].
    pub fn wary_fraction(&self) -> f32 {
        (self.wary_level / self.max_wary).clamp(0.0, 1.0)
    }

    /// Evasion chance with vigilance bonus applied, clamped [0.0, 1.0].
    /// Returns `base` when disabled.
    pub fn effective_evasion(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base + self.evasion_bonus * self.wary_fraction()).clamp(0.0, 1.0)
    }
}

impl Default for Wary {
    fn default() -> Self {
        Self::new(10.0, 5.0, 1.0, 0.25)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_calm() {
        let w = Wary::new(10.0, 5.0, 1.0, 0.25);
        assert_eq!(w.wary_level, 0.0);
        assert!(!w.is_wary());
        assert!(!w.just_alerted);
    }

    #[test]
    fn threshold_clamped_to_max_wary() {
        let w = Wary::new(10.0, 15.0, 1.0, 0.25);
        assert!((w.threshold - 10.0).abs() < 1e-5);
    }

    #[test]
    fn threshold_clamped_to_zero() {
        let w = Wary::new(10.0, -1.0, 1.0, 0.25);
        assert_eq!(w.threshold, 0.0);
    }

    #[test]
    fn alert_increases_level() {
        let mut w = Wary::new(10.0, 5.0, 1.0, 0.25);
        w.alert(3.0);
        assert!((w.wary_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn alert_caps_at_max_wary() {
        let mut w = Wary::new(10.0, 5.0, 1.0, 0.25);
        w.alert(20.0);
        assert!((w.wary_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn alert_fires_just_alerted_on_threshold_cross() {
        let mut w = Wary::new(10.0, 5.0, 1.0, 0.25);
        w.alert(5.0);
        assert!(w.just_alerted);
        assert!(w.is_wary());
    }

    #[test]
    fn alert_no_just_alerted_below_threshold() {
        let mut w = Wary::new(10.0, 5.0, 1.0, 0.25);
        w.alert(2.0);
        assert!(!w.just_alerted);
        assert!(!w.is_wary());
    }

    #[test]
    fn alert_no_just_alerted_when_already_wary() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(5.0); // crosses threshold, just_alerted
        w.tick(0.016); // clear flags
        w.alert(2.0); // already wary, no re-fire
        assert!(!w.just_alerted);
    }

    #[test]
    fn alert_no_op_when_disabled() {
        let mut w = Wary::new(10.0, 5.0, 1.0, 0.25);
        w.enabled = false;
        w.alert(8.0);
        assert_eq!(w.wary_level, 0.0);
    }

    #[test]
    fn alert_no_op_when_amount_zero() {
        let mut w = Wary::new(10.0, 5.0, 1.0, 0.25);
        w.alert(0.0);
        assert_eq!(w.wary_level, 0.0);
    }

    #[test]
    fn alert_no_op_when_amount_negative() {
        let mut w = Wary::new(10.0, 5.0, 1.0, 0.25);
        w.alert(-3.0);
        assert_eq!(w.wary_level, 0.0);
    }

    #[test]
    fn settle_decreases_level() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(8.0);
        w.settle(3.0);
        assert!((w.wary_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn settle_floors_at_zero() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(3.0);
        w.settle(10.0);
        assert_eq!(w.wary_level, 0.0);
    }

    #[test]
    fn settle_fires_just_calmed_on_threshold_drop() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(8.0); // just_alerted
        w.tick(0.016); // clear flags
        w.settle(4.0); // 8 → 4, below threshold 5
        assert!(w.just_calmed);
        assert!(!w.is_wary());
    }

    #[test]
    fn settle_no_just_calmed_when_still_wary() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(9.0);
        w.tick(0.016);
        w.settle(2.0); // 9 → 7, still above threshold 5
        assert!(!w.just_calmed);
        assert!(w.is_wary());
    }

    #[test]
    fn settle_no_op_when_disabled() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(8.0);
        w.enabled = false;
        w.settle(5.0);
        assert!((w.wary_level - 8.0).abs() < 1e-5);
    }

    #[test]
    fn settle_no_op_when_amount_zero() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(6.0);
        w.settle(0.0);
        assert!((w.wary_level - 6.0).abs() < 1e-5);
    }

    #[test]
    fn settle_no_op_when_amount_negative() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(6.0);
        w.settle(-1.0);
        assert!((w.wary_level - 6.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_wary_level() {
        let mut w = Wary::new(10.0, 5.0, 2.0, 0.25);
        w.alert(8.0);
        w.tick(1.0); // -2.0 = 6.0
        assert!((w.wary_level - 6.0).abs() < 1e-5);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut w = Wary::new(10.0, 5.0, 10.0, 0.25);
        w.alert(3.0);
        w.tick(1.0); // -10 would go to -7
        assert_eq!(w.wary_level, 0.0);
    }

    #[test]
    fn tick_fires_just_calmed_when_decay_crosses_threshold() {
        let mut w = Wary::new(10.0, 5.0, 4.0, 0.25);
        w.alert(6.0); // just_alerted, is_wary
        w.tick(0.016); // clear flags, small decay
        w.tick(1.0); // -4.0 → 6 - 4*0.016 - 4 ≈ 1.9, below threshold 5
        assert!(w.just_calmed);
        assert!(!w.is_wary());
    }

    #[test]
    fn tick_no_just_calmed_when_not_crossing_threshold() {
        let mut w = Wary::new(10.0, 3.0, 1.0, 0.25);
        w.alert(8.0);
        w.tick(0.016);
        w.tick(1.0); // 8 - 1*0.016 - 1 ≈ 6.984, still above threshold 3
        assert!(!w.just_calmed);
        assert!(w.is_wary());
    }

    #[test]
    fn tick_clears_just_alerted() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(6.0); // just_alerted = true
        w.tick(0.016);
        assert!(!w.just_alerted);
    }

    #[test]
    fn tick_clears_just_calmed() {
        let mut w = Wary::new(10.0, 5.0, 6.0, 0.25);
        w.alert(6.0);
        w.tick(0.016);
        w.tick(1.0); // just_calmed fires
        w.tick(0.016); // cleared
        assert!(!w.just_calmed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = Wary::new(10.0, 5.0, 3.0, 0.25);
        w.alert(8.0);
        w.enabled = false;
        w.tick(1.0);
        assert!((w.wary_level - 8.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_change_when_already_zero() {
        let mut w = Wary::new(10.0, 5.0, 2.0, 0.25);
        w.tick(1.0); // nothing to decay
        assert_eq!(w.wary_level, 0.0);
        assert!(!w.just_calmed);
    }

    #[test]
    fn is_wary_true_at_threshold() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(5.0);
        assert!(w.is_wary());
    }

    #[test]
    fn is_wary_false_below_threshold() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(4.9);
        assert!(!w.is_wary());
    }

    #[test]
    fn is_wary_false_when_disabled() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(8.0);
        w.enabled = false;
        assert!(!w.is_wary());
    }

    #[test]
    fn wary_fraction_zero_at_start() {
        let w = Wary::new(10.0, 5.0, 0.0, 0.25);
        assert_eq!(w.wary_fraction(), 0.0);
    }

    #[test]
    fn wary_fraction_half_at_midpoint() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(5.0);
        assert!((w.wary_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn wary_fraction_one_at_max() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(10.0);
        assert!((w.wary_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_evasion_base_when_not_wary() {
        let w = Wary::new(10.0, 5.0, 0.0, 0.25);
        assert!((w.effective_evasion(0.3) - 0.3).abs() < 1e-5);
    }

    #[test]
    fn effective_evasion_at_half_wariness() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.4);
        w.alert(5.0); // 50% wary
                      // 0.2 + 0.4 * 0.5 = 0.4
        assert!((w.effective_evasion(0.2) - 0.4).abs() < 1e-4);
    }

    #[test]
    fn effective_evasion_at_full_wariness() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.3);
        w.alert(10.0); // 100% wary
                       // 0.5 + 0.3 * 1.0 = 0.8
        assert!((w.effective_evasion(0.5) - 0.8).abs() < 1e-4);
    }

    #[test]
    fn effective_evasion_clamped_to_one() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 1.0);
        w.alert(10.0); // full
                       // 0.9 + 1.0 = 1.9 → clamped to 1.0
        assert!((w.effective_evasion(0.9) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_evasion_base_when_disabled() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(10.0);
        w.enabled = false;
        assert!((w.effective_evasion(0.3) - 0.3).abs() < 1e-5);
    }

    #[test]
    fn max_wary_clamped_to_one() {
        let w = Wary::new(0.0, 0.5, 1.0, 0.25);
        assert!((w.max_wary - 1.0).abs() < 1e-5);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let w = Wary::new(10.0, 5.0, -1.0, 0.25);
        assert_eq!(w.decay_rate, 0.0);
    }

    #[test]
    fn evasion_bonus_clamped_to_one() {
        let w = Wary::new(10.0, 5.0, 1.0, 2.0);
        assert!((w.evasion_bonus - 1.0).abs() < 1e-5);
    }

    #[test]
    fn evasion_bonus_clamped_to_zero() {
        let w = Wary::new(10.0, 5.0, 1.0, -0.5);
        assert_eq!(w.evasion_bonus, 0.0);
    }

    #[test]
    fn alert_settle_cycle() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(7.0); // just_alerted
        w.tick(0.016); // clear
        w.settle(3.0); // 7 → 4, below threshold, just_calmed
        assert!(w.just_calmed);
        w.tick(0.016); // clear
        w.alert(5.0); // 4 → 9, above threshold, just_alerted
        assert!(w.just_alerted);
    }

    #[test]
    fn additive_alerts_stack() {
        let mut w = Wary::new(10.0, 5.0, 0.0, 0.25);
        w.alert(2.0);
        w.alert(2.0);
        w.alert(2.0); // 6.0 total, crosses threshold 5
        assert!((w.wary_level - 6.0).abs() < 1e-4);
        assert!(w.is_wary());
        assert!(w.just_alerted);
    }

    #[test]
    fn zero_threshold_always_wary_when_alerted() {
        let mut w = Wary::new(10.0, 0.0, 0.0, 0.25);
        w.alert(0.001);
        assert!(w.is_wary());
    }
}
