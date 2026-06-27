use bevy_ecs::prelude::Component;

/// Social instability tracker. `unrest_level` climbs when `agitate()` is
/// called and decays naturally at `decay_rate` per second during `tick()`.
/// When `unrest_level >= threshold` the entity is considered "restless"
/// (`is_restless()` returns `true`) and its output effectiveness is penalised
/// by `penalty * unrest_fraction()`.
///
/// `agitate(amount)` increases `unrest_level` by `amount` (capped at
/// `max_unrest`). Fires `just_became_restless` on the first tick that crosses
/// `threshold` from below. No-op when disabled or `amount <= 0`.
///
/// `calm(amount)` decreases `unrest_level` by `amount` (floored at 0.0).
/// Fires `just_calmed` when dropping back below `threshold`. No-op when
/// disabled or `amount <= 0`.
///
/// `tick(dt)` clears one-frame flags first; decays `unrest_level` by
/// `decay_rate * dt` (floored at 0.0); fires `just_calmed` when the decay
/// crosses below `threshold`. No-op when disabled.
///
/// `is_restless()` returns `unrest_level >= threshold && enabled`.
///
/// `unrest_fraction()` returns `(unrest_level / max_unrest).clamp(0.0, 1.0)`.
///
/// `effective_output(base)` returns
/// `base * (1.0 - penalty * unrest_fraction())` when enabled, floored at 0.0;
/// returns `base` when disabled.
///
/// Distinct from `Morale` (group-level positively framed spirit that boosts
/// performance), `Confusion` (directional disorientation debuff), `Dread`
/// (fear-based stat reduction), and `Suppress` (external silencing of
/// abilities): Unrest is an **entity-local instability accumulator** —
/// agitation builds from internal or social events, decays naturally over
/// time, and penalises output until the entity settles.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Unrest {
    /// Current instability level [0.0, max_unrest].
    pub unrest_level: f32,
    /// Maximum instability. Clamped >= 1.0.
    pub max_unrest: f32,
    /// Level at which the entity becomes restless. Clamped [0.0, max_unrest].
    pub threshold: f32,
    /// Natural decay per second. Clamped >= 0.0.
    pub decay_rate: f32,
    /// Maximum fractional output penalty at full unrest. Clamped [0.0, 1.0].
    pub penalty: f32,
    pub just_became_restless: bool,
    pub just_calmed: bool,
    pub enabled: bool,
}

impl Unrest {
    pub fn new(max_unrest: f32, threshold: f32, decay_rate: f32, penalty: f32) -> Self {
        let max_unrest = max_unrest.max(1.0);
        Self {
            unrest_level: 0.0,
            max_unrest,
            threshold: threshold.clamp(0.0, max_unrest),
            decay_rate: decay_rate.max(0.0),
            penalty: penalty.clamp(0.0, 1.0),
            just_became_restless: false,
            just_calmed: false,
            enabled: true,
        }
    }

    /// Increase instability by `amount`. Fires `just_became_restless` on
    /// first crossing of `threshold`. No-op when disabled or `amount <= 0`.
    pub fn agitate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.unrest_level < self.threshold;
        self.unrest_level = (self.unrest_level + amount).min(self.max_unrest);
        if was_below && self.unrest_level >= self.threshold {
            self.just_became_restless = true;
        }
    }

    /// Decrease instability by `amount`. Fires `just_calmed` when dropping
    /// back below `threshold`. No-op when disabled or `amount <= 0`.
    pub fn calm(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_restless = self.unrest_level >= self.threshold;
        self.unrest_level = (self.unrest_level - amount).max(0.0);
        if was_restless && self.unrest_level < self.threshold {
            self.just_calmed = true;
        }
    }

    /// Advance one frame: clear flags, apply natural decay, fire `just_calmed`
    /// if decay pushes below threshold. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_became_restless = false;
        self.just_calmed = false;

        if !self.enabled {
            return;
        }

        if self.unrest_level > 0.0 && self.decay_rate > 0.0 {
            let was_restless = self.unrest_level >= self.threshold;
            self.unrest_level = (self.unrest_level - self.decay_rate * dt).max(0.0);
            if was_restless && self.unrest_level < self.threshold {
                self.just_calmed = true;
            }
        }
    }

    /// `true` when `unrest_level >= threshold` and component is enabled.
    pub fn is_restless(&self) -> bool {
        self.unrest_level >= self.threshold && self.enabled
    }

    /// Instability as a fraction of the maximum [0.0, 1.0].
    pub fn unrest_fraction(&self) -> f32 {
        (self.unrest_level / self.max_unrest).clamp(0.0, 1.0)
    }

    /// Output effectiveness penalised by instability. Returns
    /// `base * (1.0 - penalty * unrest_fraction())` when enabled, floored at
    /// 0.0. Returns `base` when disabled.
    pub fn effective_output(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.penalty * self.unrest_fraction())).max(0.0)
    }
}

impl Default for Unrest {
    fn default() -> Self {
        Self::new(10.0, 7.0, 1.0, 0.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_calm() {
        let u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        assert_eq!(u.unrest_level, 0.0);
        assert!(!u.is_restless());
        assert!(!u.just_became_restless);
    }

    #[test]
    fn agitate_increases_level() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.agitate(3.0);
        assert!((u.unrest_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn agitate_caps_at_max() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.agitate(20.0);
        assert!((u.unrest_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn agitate_fires_just_became_restless_on_threshold_crossing() {
        let mut u = Unrest::new(10.0, 5.0, 1.0, 0.4);
        u.agitate(5.0);
        assert!(u.just_became_restless);
        assert!(u.is_restless());
    }

    #[test]
    fn agitate_no_just_became_restless_before_threshold() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.agitate(3.0);
        assert!(!u.just_became_restless);
    }

    #[test]
    fn agitate_no_just_became_restless_when_already_restless() {
        let mut u = Unrest::new(10.0, 5.0, 0.0, 0.4); // zero decay — stays restless after tick
        u.agitate(5.0); // becomes restless
        u.tick(0.016); // clear flags, no decay (level stays at 5.0)
        u.agitate(2.0); // still restless, no re-fire
        assert!(!u.just_became_restless);
    }

    #[test]
    fn agitate_no_op_when_disabled() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.enabled = false;
        u.agitate(5.0);
        assert_eq!(u.unrest_level, 0.0);
    }

    #[test]
    fn agitate_no_op_when_amount_zero() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.agitate(0.0);
        assert_eq!(u.unrest_level, 0.0);
    }

    #[test]
    fn agitate_no_op_when_amount_negative() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.agitate(-1.0);
        assert_eq!(u.unrest_level, 0.0);
    }

    #[test]
    fn calm_decreases_level() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.agitate(8.0);
        u.calm(3.0);
        assert!((u.unrest_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn calm_floors_at_zero() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.agitate(2.0);
        u.calm(10.0);
        assert_eq!(u.unrest_level, 0.0);
    }

    #[test]
    fn calm_fires_just_calmed_when_crossing_below_threshold() {
        let mut u = Unrest::new(10.0, 5.0, 1.0, 0.4);
        u.agitate(7.0); // restless
        u.tick(0.016); // clear flags
        u.calm(3.0); // drop from 7 to 4, below threshold
        assert!(u.just_calmed);
        assert!(!u.is_restless());
    }

    #[test]
    fn calm_no_just_calmed_when_stays_restless() {
        let mut u = Unrest::new(10.0, 5.0, 1.0, 0.4);
        u.agitate(8.0);
        u.tick(0.016);
        u.calm(1.0); // 8 → 7, still above threshold
        assert!(!u.just_calmed);
    }

    #[test]
    fn calm_no_op_when_disabled() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.agitate(8.0);
        u.enabled = false;
        u.calm(5.0);
        assert!((u.unrest_level - 8.0).abs() < 1e-5);
    }

    #[test]
    fn calm_no_op_when_amount_zero() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.agitate(8.0);
        u.calm(0.0);
        assert!((u.unrest_level - 8.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_level() {
        let mut u = Unrest::new(10.0, 7.0, 2.0, 0.4);
        u.agitate(6.0);
        u.tick(1.0); // -2.0 = 4.0
        assert!((u.unrest_level - 4.0).abs() < 1e-5);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut u = Unrest::new(10.0, 7.0, 10.0, 0.4);
        u.agitate(2.0);
        u.tick(1.0); // would go below 0
        assert_eq!(u.unrest_level, 0.0);
    }

    #[test]
    fn tick_fires_just_calmed_on_decay_below_threshold() {
        let mut u = Unrest::new(10.0, 5.0, 2.0, 0.4);
        u.agitate(6.0); // restless
        u.tick(0.016); // clear flags, small decay
        u.tick(1.0); // -2.0 → 4.0 (approx), below threshold
                     // actually: 6 - 0.016*2 = 5.968, then - 2.0 = 3.968 (below 5.0)
        assert!(u.just_calmed);
    }

    #[test]
    fn tick_no_just_calmed_when_not_restless() {
        let mut u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        u.agitate(3.0);
        u.tick(1.0); // below threshold, no calmed event
        assert!(!u.just_calmed);
    }

    #[test]
    fn tick_clears_just_became_restless() {
        let mut u = Unrest::new(10.0, 5.0, 0.0, 0.4);
        u.agitate(5.0);
        assert!(u.just_became_restless);
        u.tick(0.016);
        assert!(!u.just_became_restless);
    }

    #[test]
    fn tick_clears_just_calmed() {
        let mut u = Unrest::new(10.0, 5.0, 10.0, 0.4);
        u.agitate(6.0);
        u.tick(0.016); // small decay
        u.tick(1.0); // just_calmed fires
        u.tick(0.016); // cleared
        assert!(!u.just_calmed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut u = Unrest::new(10.0, 7.0, 5.0, 0.4);
        u.agitate(8.0);
        u.enabled = false;
        u.tick(2.0);
        assert!((u.unrest_level - 8.0).abs() < 1e-5);
    }

    #[test]
    fn is_restless_true_at_threshold() {
        let mut u = Unrest::new(10.0, 5.0, 0.0, 0.4);
        u.agitate(5.0);
        assert!(u.is_restless());
    }

    #[test]
    fn is_restless_false_below_threshold() {
        let mut u = Unrest::new(10.0, 5.0, 0.0, 0.4);
        u.agitate(4.9);
        assert!(!u.is_restless());
    }

    #[test]
    fn is_restless_false_when_disabled() {
        let mut u = Unrest::new(10.0, 5.0, 0.0, 0.4);
        u.agitate(8.0);
        u.enabled = false;
        assert!(!u.is_restless());
    }

    #[test]
    fn unrest_fraction_zero_at_start() {
        let u = Unrest::new(10.0, 7.0, 1.0, 0.4);
        assert_eq!(u.unrest_fraction(), 0.0);
    }

    #[test]
    fn unrest_fraction_half_at_midpoint() {
        let mut u = Unrest::new(10.0, 7.0, 0.0, 0.4);
        u.agitate(5.0);
        assert!((u.unrest_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn unrest_fraction_one_at_max() {
        let mut u = Unrest::new(10.0, 7.0, 0.0, 0.4);
        u.agitate(10.0);
        assert!((u.unrest_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_reduced_at_full_unrest() {
        let mut u = Unrest::new(10.0, 7.0, 0.0, 0.5);
        u.agitate(10.0); // full unrest
                         // 100 * (1 - 0.5 * 1.0) = 50
        assert!((u.effective_output(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_partial_reduction() {
        let mut u = Unrest::new(10.0, 7.0, 0.0, 0.4);
        u.agitate(5.0); // 50% fraction
                        // 100 * (1 - 0.4 * 0.5) = 80
        assert!((u.effective_output(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_base_when_disabled() {
        let mut u = Unrest::new(10.0, 7.0, 0.0, 0.4);
        u.agitate(10.0);
        u.enabled = false;
        assert!((u.effective_output(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_base_at_zero_unrest() {
        let u = Unrest::new(10.0, 7.0, 0.0, 0.4);
        assert!((u.effective_output(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_floored_at_zero() {
        let mut u = Unrest::new(10.0, 7.0, 0.0, 1.0);
        u.agitate(10.0); // full penalty
        assert_eq!(u.effective_output(100.0), 0.0);
    }

    #[test]
    fn threshold_clamped_to_max() {
        let u = Unrest::new(10.0, 15.0, 1.0, 0.4);
        assert!((u.threshold - 10.0).abs() < 1e-5);
    }

    #[test]
    fn max_unrest_clamped_to_one() {
        let u = Unrest::new(0.0, 0.5, 1.0, 0.4);
        assert!((u.max_unrest - 1.0).abs() < 1e-5);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let u = Unrest::new(10.0, 7.0, -1.0, 0.4);
        assert_eq!(u.decay_rate, 0.0);
    }

    #[test]
    fn penalty_clamped_to_one() {
        let u = Unrest::new(10.0, 7.0, 1.0, 2.0);
        assert!((u.penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn penalty_clamped_to_zero() {
        let u = Unrest::new(10.0, 7.0, 1.0, -0.5);
        assert_eq!(u.penalty, 0.0);
    }

    #[test]
    fn agitate_calm_cycle() {
        let mut u = Unrest::new(10.0, 5.0, 0.0, 0.4);
        u.agitate(8.0);
        assert!(u.is_restless());
        u.calm(5.0); // 8 → 3, below threshold
        assert!(u.just_calmed);
        assert!(!u.is_restless());
    }
}
