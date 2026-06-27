use bevy_ecs::prelude::Component;

/// Currency/wealth tracker. `coins` accumulates via `earn(amount)` and
/// decreases via `spend(amount)` or passive `tax_rate` per second in
/// `tick(dt)`.
///
/// Models in-game currency pools, barter value, economic resources, trade
/// credit, reputation-as-currency, or any mechanic where wealth builds up
/// through activity and erodes through overhead or time.
///
/// `earn(amount)` adds to `coins`; fires `just_wealthy` when first reaching
/// `max_coins`. No-op when disabled.
///
/// `spend(amount)` subtracts from `coins`; fires `just_broke` when reaching
/// 0. No-op when disabled or already broke.
///
/// `tick(dt)` clears `just_wealthy` and `just_broke`, then applies passive
/// taxation: `coins -= tax_rate * dt` (floored at 0). Fires `just_broke`
/// when reaching 0 via tax. No-op tax when disabled or rate is 0.
///
/// `is_wealthy()` returns `coins >= max_coins && enabled`.
///
/// `is_broke()` returns `coins == 0.0` (not gated by `enabled`).
///
/// `wealth_fraction()` returns `(coins / max_coins).clamp(0, 1)`.
///
/// `effective_purchasing_power(base)` returns `base * wealth_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 0.0)` — no passive tax.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zloty {
    pub coins: f32,
    pub max_coins: f32,
    pub tax_rate: f32,
    pub just_wealthy: bool,
    pub just_broke: bool,
    pub enabled: bool,
}

impl Zloty {
    pub fn new(max_coins: f32, tax_rate: f32) -> Self {
        Self {
            coins: 0.0,
            max_coins: max_coins.max(0.1),
            tax_rate: tax_rate.max(0.0),
            just_wealthy: false,
            just_broke: false,
            enabled: true,
        }
    }

    /// Add to coins; fires `just_wealthy` when first reaching max.
    /// No-op when disabled.
    pub fn earn(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.coins < self.max_coins;
        self.coins = (self.coins + amount).min(self.max_coins);
        if was_below && self.coins >= self.max_coins {
            self.just_wealthy = true;
        }
    }

    /// Subtract from coins; fires `just_broke` when reaching 0.
    /// No-op when disabled or already broke.
    pub fn spend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.coins <= 0.0 {
            return;
        }
        self.coins = (self.coins - amount).max(0.0);
        if self.coins <= 0.0 {
            self.just_broke = true;
        }
    }

    /// Clear flags, then apply tax at `tax_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wealthy = false;
        self.just_broke = false;
        if self.enabled && self.tax_rate > 0.0 && self.coins > 0.0 {
            self.coins = (self.coins - self.tax_rate * dt).max(0.0);
            if self.coins <= 0.0 {
                self.just_broke = true;
            }
        }
    }

    /// `true` when coins is at maximum and component is enabled.
    pub fn is_wealthy(&self) -> bool {
        self.coins >= self.max_coins && self.enabled
    }

    /// `true` when coins is 0 (not gated by `enabled`).
    pub fn is_broke(&self) -> bool {
        self.coins == 0.0
    }

    /// Fraction of maximum coins [0.0, 1.0].
    pub fn wealth_fraction(&self) -> f32 {
        (self.coins / self.max_coins).clamp(0.0, 1.0)
    }

    /// Returns `base * wealth_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_purchasing_power(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.wealth_fraction()
    }
}

impl Default for Zloty {
    fn default() -> Self {
        Self::new(100.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Zloty {
        Zloty::new(100.0, 0.0) // no tax by default for simple tests
    }

    // --- construction ---

    #[test]
    fn new_starts_broke() {
        let y = y();
        assert_eq!(y.coins, 0.0);
        assert!(y.is_broke());
        assert!(!y.is_wealthy());
    }

    #[test]
    fn new_clamps_max_coins() {
        let y = Zloty::new(-5.0, 0.0);
        assert!((y.max_coins - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_tax_rate() {
        let y = Zloty::new(100.0, -3.0);
        assert_eq!(y.tax_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Zloty::default();
        assert!((y.max_coins - 100.0).abs() < 1e-5);
        assert_eq!(y.tax_rate, 0.0);
    }

    // --- earn ---

    #[test]
    fn earn_adds_coins() {
        let mut y = y();
        y.earn(40.0);
        assert!((y.coins - 40.0).abs() < 1e-3);
    }

    #[test]
    fn earn_clamps_at_max() {
        let mut y = y();
        y.earn(200.0);
        assert!((y.coins - 100.0).abs() < 1e-3);
    }

    #[test]
    fn earn_fires_just_wealthy_at_max() {
        let mut y = y();
        y.earn(100.0);
        assert!(y.just_wealthy);
        assert!(y.is_wealthy());
    }

    #[test]
    fn earn_no_just_wealthy_when_already_at_max() {
        let mut y = y();
        y.coins = 100.0;
        y.earn(10.0);
        assert!(!y.just_wealthy);
    }

    #[test]
    fn earn_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.earn(50.0);
        assert_eq!(y.coins, 0.0);
    }

    #[test]
    fn earn_no_op_when_amount_zero() {
        let mut y = y();
        y.earn(0.0);
        assert_eq!(y.coins, 0.0);
    }

    // --- spend ---

    #[test]
    fn spend_reduces_coins() {
        let mut y = y();
        y.coins = 60.0;
        y.spend(20.0);
        assert!((y.coins - 40.0).abs() < 1e-3);
    }

    #[test]
    fn spend_clamps_at_zero() {
        let mut y = y();
        y.coins = 30.0;
        y.spend(200.0);
        assert_eq!(y.coins, 0.0);
    }

    #[test]
    fn spend_fires_just_broke_at_zero() {
        let mut y = y();
        y.coins = 30.0;
        y.spend(30.0);
        assert!(y.just_broke);
    }

    #[test]
    fn spend_no_op_when_already_broke() {
        let mut y = y();
        y.spend(10.0);
        assert!(!y.just_broke);
    }

    #[test]
    fn spend_no_op_when_disabled() {
        let mut y = y();
        y.coins = 50.0;
        y.enabled = false;
        y.spend(50.0);
        assert!((y.coins - 50.0).abs() < 1e-3);
    }

    // --- tick / tax ---

    #[test]
    fn tick_applies_tax() {
        let mut y = Zloty::new(100.0, 10.0);
        y.coins = 60.0;
        y.tick(1.0); // 60 - 10 = 50
        assert!((y.coins - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_broke_on_tax_to_zero() {
        let mut y = Zloty::new(100.0, 200.0);
        y.coins = 5.0;
        y.tick(1.0);
        assert!(y.just_broke);
        assert!(y.is_broke());
    }

    #[test]
    fn tick_no_tax_when_already_broke() {
        let mut y = Zloty::new(100.0, 10.0);
        y.tick(10.0); // already broke
        assert!(!y.just_broke);
    }

    #[test]
    fn tick_no_tax_when_rate_zero() {
        let mut y = y(); // tax=0
        y.coins = 50.0;
        y.tick(100.0);
        assert!((y.coins - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_tax_when_disabled() {
        let mut y = Zloty::new(100.0, 10.0);
        y.coins = 50.0;
        y.enabled = false;
        y.tick(1.0);
        assert!((y.coins - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_wealthy() {
        let mut y = y();
        y.earn(100.0); // just_wealthy fires
        y.tick(0.016);
        assert!(!y.just_wealthy);
    }

    #[test]
    fn tick_clears_just_broke() {
        let mut y = Zloty::new(100.0, 200.0);
        y.coins = 5.0;
        y.tick(1.0); // just_broke fires
        y.tick(0.016);
        assert!(!y.just_broke);
    }

    #[test]
    fn tick_scales_tax_with_dt() {
        let mut y = Zloty::new(100.0, 10.0);
        y.coins = 100.0;
        y.tick(2.0); // 100 - 10*2 = 80
        assert!((y.coins - 80.0).abs() < 1e-3);
    }

    // --- is_wealthy / is_broke ---

    #[test]
    fn is_wealthy_false_when_disabled() {
        let mut y = y();
        y.coins = 100.0;
        y.enabled = false;
        assert!(!y.is_wealthy());
    }

    #[test]
    fn is_broke_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_broke());
    }

    // --- wealth_fraction / effective_purchasing_power ---

    #[test]
    fn wealth_fraction_zero_when_broke() {
        assert_eq!(y().wealth_fraction(), 0.0);
    }

    #[test]
    fn wealth_fraction_half_at_midpoint() {
        let mut y = y();
        y.coins = 50.0;
        assert!((y.wealth_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_purchasing_power_zero_when_broke() {
        assert_eq!(y().effective_purchasing_power(100.0), 0.0);
    }

    #[test]
    fn effective_purchasing_power_scales_with_coins() {
        let mut y = y();
        y.coins = 75.0;
        assert!((y.effective_purchasing_power(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_purchasing_power_zero_when_disabled() {
        let mut y = y();
        y.coins = 50.0;
        y.enabled = false;
        assert_eq!(y.effective_purchasing_power(100.0), 0.0);
    }
}
