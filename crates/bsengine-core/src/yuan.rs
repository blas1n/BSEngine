use bevy_ecs::prelude::Component;

/// Resource reserve with capacity cap. Tracks a pool of currency, tokens,
/// or counted resources in [0, `max_balance`]. `earn(amount)` adds to the
/// reserve; `spend(amount)` draws from it. Fires `just_flush` on first
/// reaching max and `just_spent` on first reaching 0.
///
/// Models gold reserves, energy credits, token pools, commodity stocks, or
/// any counted resource where zero triggers scarcity events and max triggers
/// prosperity events.
///
/// `earn(amount)` increases balance. Fires `just_flush` on first reaching
/// `max_balance`. No-op when disabled or already full.
///
/// `spend(amount)` decreases balance. Fires `just_spent` on first reaching
/// 0. No-op when disabled, already empty, or `amount <= 0`.
///
/// `tick(_dt)` clears `just_flush` and `just_spent` only.
///
/// `is_flush()` returns `balance >= max_balance && enabled`.
///
/// `is_spent()` returns `balance == 0.0` (not gated by `enabled`).
///
/// `balance_fraction()` returns `(balance / max_balance).clamp(0, 1)`.
///
/// `effective_value(base)` returns `base * balance_fraction()` when enabled;
/// `0.0` when disabled. Scales with how full the reserve is.
///
/// Default: `new(100.0)` — empty reserve.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yuan {
    pub balance: f32,
    pub max_balance: f32,
    pub just_flush: bool,
    pub just_spent: bool,
    pub enabled: bool,
}

impl Yuan {
    pub fn new(max_balance: f32) -> Self {
        Self {
            balance: 0.0,
            max_balance: max_balance.max(0.1),
            just_flush: false,
            just_spent: false,
            enabled: true,
        }
    }

    /// Add to reserve. Fires `just_flush` on first reaching `max_balance`.
    /// No-op when disabled or already full.
    pub fn earn(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.balance >= self.max_balance {
            return;
        }
        self.balance = (self.balance + amount).min(self.max_balance);
        if self.balance >= self.max_balance {
            self.just_flush = true;
        }
    }

    /// Draw from reserve. Fires `just_spent` on first reaching 0. No-op
    /// when disabled, already empty, or `amount <= 0`.
    pub fn spend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.balance <= 0.0 {
            return;
        }
        self.balance = (self.balance - amount).max(0.0);
        if self.balance <= 0.0 {
            self.just_spent = true;
        }
    }

    /// Advance one frame: clear `just_flush` and `just_spent` only.
    pub fn tick(&mut self, _dt: f32) {
        self.just_flush = false;
        self.just_spent = false;
    }

    /// `true` when reserve is full and component is enabled.
    pub fn is_flush(&self) -> bool {
        self.balance >= self.max_balance && self.enabled
    }

    /// `true` when reserve is empty (not gated by `enabled`).
    pub fn is_spent(&self) -> bool {
        self.balance == 0.0
    }

    /// Fraction of capacity filled [0.0, 1.0].
    pub fn balance_fraction(&self) -> f32 {
        (self.balance / self.max_balance).clamp(0.0, 1.0)
    }

    /// Returns `base * balance_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_value(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.balance_fraction()
    }
}

impl Default for Yuan {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yuan {
        Yuan::new(100.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let y = y();
        assert_eq!(y.balance, 0.0);
        assert!(y.is_spent());
        assert!(!y.is_flush());
    }

    #[test]
    fn new_clamps_max_balance() {
        let y = Yuan::new(-5.0);
        assert!((y.max_balance - 0.1).abs() < 1e-5);
    }

    #[test]
    fn default_max_balance_is_hundred() {
        assert!((Yuan::default().max_balance - 100.0).abs() < 1e-5);
    }

    // --- earn ---

    #[test]
    fn earn_increases_balance() {
        let mut y = y();
        y.earn(40.0);
        assert!((y.balance - 40.0).abs() < 1e-4);
    }

    #[test]
    fn earn_clamps_at_max() {
        let mut y = y();
        y.earn(200.0);
        assert!((y.balance - 100.0).abs() < 1e-5);
    }

    #[test]
    fn earn_fires_just_flush_at_max() {
        let mut y = y();
        y.earn(100.0);
        assert!(y.just_flush);
        assert!(y.is_flush());
    }

    #[test]
    fn earn_no_refire_when_already_full() {
        let mut y = y();
        y.earn(100.0);
        y.tick(0.016);
        y.earn(10.0); // already at max
        assert!(!y.just_flush);
    }

    #[test]
    fn earn_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.earn(50.0);
        assert_eq!(y.balance, 0.0);
    }

    #[test]
    fn earn_no_op_for_zero_amount() {
        let mut y = y();
        y.earn(0.0);
        assert_eq!(y.balance, 0.0);
    }

    // --- spend ---

    #[test]
    fn spend_decreases_balance() {
        let mut y = y();
        y.earn(80.0);
        y.tick(0.016);
        y.spend(30.0);
        assert!((y.balance - 50.0).abs() < 1e-4);
    }

    #[test]
    fn spend_clamps_at_zero() {
        let mut y = y();
        y.earn(40.0);
        y.tick(0.016);
        y.spend(100.0);
        assert_eq!(y.balance, 0.0);
    }

    #[test]
    fn spend_fires_just_spent_at_zero() {
        let mut y = y();
        y.earn(40.0);
        y.tick(0.016);
        y.spend(40.0);
        assert!(y.just_spent);
        assert!(y.is_spent());
    }

    #[test]
    fn spend_no_refire_when_already_empty() {
        let mut y = y();
        y.spend(10.0); // already empty
        assert!(!y.just_spent);
    }

    #[test]
    fn spend_no_op_when_disabled() {
        let mut y = y();
        y.earn(60.0);
        y.enabled = false;
        y.spend(20.0);
        assert!((y.balance - 60.0).abs() < 1e-4);
    }

    #[test]
    fn spend_no_op_for_zero_amount() {
        let mut y = y();
        y.earn(50.0);
        y.tick(0.016);
        y.spend(0.0);
        assert!((y.balance - 50.0).abs() < 1e-4);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_flush() {
        let mut y = y();
        y.earn(100.0);
        y.tick(0.016);
        assert!(!y.just_flush);
    }

    #[test]
    fn tick_clears_just_spent() {
        let mut y = y();
        y.earn(30.0);
        y.spend(30.0);
        y.tick(0.016);
        assert!(!y.just_spent);
    }

    #[test]
    fn tick_does_not_change_balance() {
        let mut y = y();
        y.earn(60.0);
        y.tick(1000.0);
        assert!((y.balance - 60.0).abs() < 1e-5);
    }

    // --- is_flush / is_spent ---

    #[test]
    fn is_flush_false_below_max() {
        let mut y = y();
        y.earn(50.0);
        assert!(!y.is_flush());
    }

    #[test]
    fn is_flush_false_when_disabled() {
        let mut y = y();
        y.earn(100.0);
        y.enabled = false;
        assert!(!y.is_flush());
    }

    #[test]
    fn is_spent_true_at_zero() {
        assert!(y().is_spent());
    }

    #[test]
    fn is_spent_true_even_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_spent()); // not gated
    }

    #[test]
    fn is_spent_false_with_balance() {
        let mut y = y();
        y.earn(10.0);
        assert!(!y.is_spent());
    }

    // --- fractions / effective ---

    #[test]
    fn balance_fraction_zero_when_empty() {
        assert_eq!(y().balance_fraction(), 0.0);
    }

    #[test]
    fn balance_fraction_half_at_midpoint() {
        let mut y = y();
        y.earn(50.0);
        assert!((y.balance_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn balance_fraction_one_when_full() {
        let mut y = y();
        y.earn(100.0);
        assert!((y.balance_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_value_zero_when_empty() {
        assert_eq!(y().effective_value(100.0), 0.0);
    }

    #[test]
    fn effective_value_scales_with_fraction() {
        let mut y = y();
        y.earn(60.0);
        assert!((y.effective_value(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_value_zero_when_disabled() {
        let mut y = y();
        y.earn(50.0);
        y.enabled = false;
        assert_eq!(y.effective_value(100.0), 0.0);
    }
}
