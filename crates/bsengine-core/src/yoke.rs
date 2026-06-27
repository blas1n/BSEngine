use bevy_ecs::prelude::Component;

/// Shared-burden tracker that accumulates weight from external sources and
/// penalises movement speed proportionally. `yoke_weight` grows via
/// `burden(amount)` calls and decays passively at `recovery_rate` per second
/// during `tick()`. Systems layer on weight from environment, debuffs, or
/// physics interactions and read `effective_speed()` to apply the penalty.
///
/// `burden(amount)` increases `yoke_weight` by `amount` (capped at
/// `max_weight`). Fires `just_burdened` on the first positive weight from
/// 0.0. No-op when disabled or `amount <= 0`.
///
/// `relieve(amount)` decreases `yoke_weight` by `amount` (floored at 0.0).
/// Fires `just_freed` when weight returns to 0.0. No-op when disabled or
/// `amount <= 0`.
///
/// `tick(dt)` clears one-frame flags first; decays `yoke_weight` by
/// `recovery_rate * dt` (floored at 0.0); fires `just_freed` when decay
/// reaches 0.0. No-op when disabled.
///
/// `is_burdened()` returns `yoke_weight > 0.0 && enabled`.
///
/// `burden_fraction()` returns `(yoke_weight / max_weight).clamp(0.0, 1.0)`.
///
/// `effective_speed(base)` returns
/// `base * (1.0 - speed_penalty * burden_fraction())` when enabled, floored
/// at 0.0; returns `base` when disabled.
///
/// Distinct from `Slow` (flat external speed reduction applied as a timed
/// debuff from another entity), `Laden` (inventory-weight-based capacity
/// limit), `Crush` (escalating damage from pressure overhead), and `Hobble`
/// (targeted limb-damage movement debuff): Yoke models a **variable shared
/// burden** — weight accumulates additively from any number of sources,
/// recovers passively, and scales movement penalty continuously.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yoke {
    /// Current accumulated burden [0.0, max_weight].
    pub yoke_weight: f32,
    /// Maximum weight before further burden is ignored. Clamped >= 1.0.
    pub max_weight: f32,
    /// Passive weight recovery per second. Clamped >= 0.0.
    pub recovery_rate: f32,
    /// Maximum fractional speed penalty at full weight. Clamped [0.0, 1.0].
    pub speed_penalty: f32,
    pub just_burdened: bool,
    pub just_freed: bool,
    pub enabled: bool,
}

impl Yoke {
    pub fn new(max_weight: f32, recovery_rate: f32, speed_penalty: f32) -> Self {
        Self {
            yoke_weight: 0.0,
            max_weight: max_weight.max(1.0),
            recovery_rate: recovery_rate.max(0.0),
            speed_penalty: speed_penalty.clamp(0.0, 1.0),
            just_burdened: false,
            just_freed: false,
            enabled: true,
        }
    }

    /// Add burden. Fires `just_burdened` on the first positive weight from
    /// 0.0. No-op when disabled or `amount <= 0`.
    pub fn burden(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_zero = self.yoke_weight == 0.0;
        self.yoke_weight = (self.yoke_weight + amount).min(self.max_weight);
        if was_zero && self.yoke_weight > 0.0 {
            self.just_burdened = true;
        }
    }

    /// Reduce burden. Fires `just_freed` when weight returns to 0.0. No-op
    /// when disabled or `amount <= 0`.
    pub fn relieve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_positive = self.yoke_weight > 0.0;
        self.yoke_weight = (self.yoke_weight - amount).max(0.0);
        if was_positive && self.yoke_weight == 0.0 {
            self.just_freed = true;
        }
    }

    /// Advance one frame: clear flags, apply passive recovery, fire
    /// `just_freed` when recovery reaches 0.0. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_burdened = false;
        self.just_freed = false;

        if !self.enabled {
            return;
        }

        if self.yoke_weight > 0.0 && self.recovery_rate > 0.0 {
            let was_positive = self.yoke_weight > 0.0;
            self.yoke_weight = (self.yoke_weight - self.recovery_rate * dt).max(0.0);
            if was_positive && self.yoke_weight == 0.0 {
                self.just_freed = true;
            }
        }
    }

    /// `true` when the entity carries any burden and the component is enabled.
    pub fn is_burdened(&self) -> bool {
        self.yoke_weight > 0.0 && self.enabled
    }

    /// Burden as a fraction of the maximum [0.0, 1.0].
    pub fn burden_fraction(&self) -> f32 {
        (self.yoke_weight / self.max_weight).clamp(0.0, 1.0)
    }

    /// Movement speed reduced by current burden. Returns
    /// `base * (1.0 - speed_penalty * burden_fraction())` when enabled,
    /// floored at 0.0. Returns `base` when disabled.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.speed_penalty * self.burden_fraction())).max(0.0)
    }
}

impl Default for Yoke {
    fn default() -> Self {
        Self::new(10.0, 2.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_unburdened() {
        let y = Yoke::new(10.0, 2.0, 0.5);
        assert_eq!(y.yoke_weight, 0.0);
        assert!(!y.is_burdened());
        assert!(!y.just_burdened);
    }

    #[test]
    fn burden_increases_weight() {
        let mut y = Yoke::new(10.0, 2.0, 0.5);
        y.burden(3.0);
        assert!((y.yoke_weight - 3.0).abs() < 1e-5);
    }

    #[test]
    fn burden_caps_at_max_weight() {
        let mut y = Yoke::new(10.0, 2.0, 0.5);
        y.burden(20.0);
        assert!((y.yoke_weight - 10.0).abs() < 1e-5);
    }

    #[test]
    fn burden_fires_just_burdened_from_zero() {
        let mut y = Yoke::new(10.0, 2.0, 0.5);
        y.burden(3.0);
        assert!(y.just_burdened);
        assert!(y.is_burdened());
    }

    #[test]
    fn burden_no_just_burdened_when_already_positive() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(2.0); // just_burdened fires
        y.tick(0.016); // clear flags
        y.burden(1.0); // still positive, no re-fire
        assert!(!y.just_burdened);
    }

    #[test]
    fn burden_no_op_when_disabled() {
        let mut y = Yoke::new(10.0, 2.0, 0.5);
        y.enabled = false;
        y.burden(5.0);
        assert_eq!(y.yoke_weight, 0.0);
    }

    #[test]
    fn burden_no_op_when_amount_zero() {
        let mut y = Yoke::new(10.0, 2.0, 0.5);
        y.burden(0.0);
        assert_eq!(y.yoke_weight, 0.0);
    }

    #[test]
    fn burden_no_op_when_amount_negative() {
        let mut y = Yoke::new(10.0, 2.0, 0.5);
        y.burden(-1.0);
        assert_eq!(y.yoke_weight, 0.0);
    }

    #[test]
    fn relieve_decreases_weight() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(8.0);
        y.relieve(3.0);
        assert!((y.yoke_weight - 5.0).abs() < 1e-5);
    }

    #[test]
    fn relieve_floors_at_zero() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(3.0);
        y.relieve(10.0);
        assert_eq!(y.yoke_weight, 0.0);
    }

    #[test]
    fn relieve_fires_just_freed_when_reaching_zero() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(5.0);
        y.tick(0.016); // clear flags
        y.relieve(5.0);
        assert!(y.just_freed);
        assert!(!y.is_burdened());
    }

    #[test]
    fn relieve_no_just_freed_when_still_positive() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(8.0);
        y.tick(0.016);
        y.relieve(3.0); // 8 → 5, still positive
        assert!(!y.just_freed);
    }

    #[test]
    fn relieve_no_op_when_disabled() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(8.0);
        y.enabled = false;
        y.relieve(5.0);
        assert!((y.yoke_weight - 8.0).abs() < 1e-5);
    }

    #[test]
    fn relieve_no_op_when_amount_zero() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(5.0);
        y.relieve(0.0);
        assert!((y.yoke_weight - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_weight() {
        let mut y = Yoke::new(10.0, 3.0, 0.5);
        y.burden(6.0);
        y.tick(1.0); // -3.0 = 3.0
        assert!((y.yoke_weight - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_floors_weight_at_zero() {
        let mut y = Yoke::new(10.0, 10.0, 0.5);
        y.burden(2.0);
        y.tick(1.0); // would go to -8
        assert_eq!(y.yoke_weight, 0.0);
    }

    #[test]
    fn tick_fires_just_freed_when_decay_reaches_zero() {
        let mut y = Yoke::new(10.0, 5.0, 0.5);
        y.burden(2.0); // just_burdened fires
        y.tick(0.016); // clear flags, small decay
        y.tick(1.0); // -5.0, reaches 0
        assert!(y.just_freed);
    }

    #[test]
    fn tick_no_just_freed_when_not_at_zero() {
        let mut y = Yoke::new(10.0, 1.0, 0.5);
        y.burden(5.0);
        y.tick(0.016);
        y.tick(1.0); // 5 - 1 = 4, not zero
        assert!(!y.just_freed);
    }

    #[test]
    fn tick_clears_just_burdened() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(3.0); // just_burdened = true
        y.tick(0.016);
        assert!(!y.just_burdened);
    }

    #[test]
    fn tick_clears_just_freed() {
        let mut y = Yoke::new(10.0, 10.0, 0.5);
        y.burden(1.0);
        y.tick(0.016);
        y.tick(1.0); // just_freed = true
        y.tick(0.016); // cleared
        assert!(!y.just_freed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut y = Yoke::new(10.0, 3.0, 0.5);
        y.burden(6.0);
        y.enabled = false;
        y.tick(2.0);
        assert!((y.yoke_weight - 6.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_change_when_weight_already_zero() {
        let mut y = Yoke::new(10.0, 5.0, 0.5);
        y.tick(1.0); // no burden, nothing to do
        assert_eq!(y.yoke_weight, 0.0);
        assert!(!y.just_freed);
    }

    #[test]
    fn is_burdened_true_when_positive() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(1.0);
        assert!(y.is_burdened());
    }

    #[test]
    fn is_burdened_false_at_zero() {
        let y = Yoke::new(10.0, 0.0, 0.5);
        assert!(!y.is_burdened());
    }

    #[test]
    fn is_burdened_false_when_disabled() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(5.0);
        y.enabled = false;
        assert!(!y.is_burdened());
    }

    #[test]
    fn burden_fraction_zero_when_empty() {
        let y = Yoke::new(10.0, 0.0, 0.5);
        assert_eq!(y.burden_fraction(), 0.0);
    }

    #[test]
    fn burden_fraction_half_at_midpoint() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(5.0);
        assert!((y.burden_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn burden_fraction_one_at_max() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(10.0);
        assert!((y.burden_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_reduced_at_full_burden() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(10.0); // full burden
                        // 100 * (1 - 0.5 * 1.0) = 50
        assert!((y.effective_speed(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_partial_at_half_burden() {
        let mut y = Yoke::new(10.0, 0.0, 0.4);
        y.burden(5.0); // 50% burden
                       // 100 * (1 - 0.4 * 0.5) = 80
        assert!((y.effective_speed(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_full_when_unburdened() {
        let y = Yoke::new(10.0, 0.0, 0.5);
        assert!((y.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_base_when_disabled() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(10.0);
        y.enabled = false;
        assert!((y.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_floored_at_zero() {
        let mut y = Yoke::new(10.0, 0.0, 1.0);
        y.burden(10.0); // full burden, full penalty
        assert_eq!(y.effective_speed(100.0), 0.0);
    }

    #[test]
    fn max_weight_clamped_to_one() {
        let y = Yoke::new(0.0, 2.0, 0.5);
        assert!((y.max_weight - 1.0).abs() < 1e-5);
    }

    #[test]
    fn recovery_rate_clamped_to_zero() {
        let y = Yoke::new(10.0, -1.0, 0.5);
        assert_eq!(y.recovery_rate, 0.0);
    }

    #[test]
    fn speed_penalty_clamped_to_one() {
        let y = Yoke::new(10.0, 2.0, 2.0);
        assert!((y.speed_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn speed_penalty_clamped_to_zero() {
        let y = Yoke::new(10.0, 2.0, -0.5);
        assert_eq!(y.speed_penalty, 0.0);
    }

    #[test]
    fn burden_relieve_cycle() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(7.0);
        y.tick(0.016);
        y.relieve(7.0); // just_freed fires
        assert!(y.just_freed);
        assert!(!y.is_burdened());
        y.burden(3.0); // just_burdened re-fires
        assert!(y.just_burdened);
    }

    #[test]
    fn additive_burden_from_multiple_sources() {
        let mut y = Yoke::new(10.0, 0.0, 0.5);
        y.burden(2.0);
        y.burden(3.0);
        y.burden(1.5);
        assert!((y.yoke_weight - 6.5).abs() < 1e-4);
    }
}
