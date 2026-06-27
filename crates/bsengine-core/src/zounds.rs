use bevy_ecs::prelude::Component;

/// Impact-shock tracker. `shock` builds via `jolt(amount)` and dampens
/// passively at `dampening_rate` per second in `tick(dt)` or immediately
/// via `settle(amount)`.
///
/// Models physical shockwaves, concussive force meters, hit-stagger
/// accumulators, recoil gauges, explosive-blast intensity, or any mechanic
/// where sudden impacts accumulate into a sustained daze that gradually
/// clears.
///
/// `jolt(amount)` adds shock; fires `just_stunned` when first reaching
/// `max_shock`. No-op when disabled.
///
/// `settle(amount)` reduces shock immediately; fires `just_settled` when
/// reaching 0. No-op when disabled or already settled.
///
/// `tick(dt)` clears both flags, then dampens shock by
/// `dampening_rate * dt` (floored at 0). Fires `just_settled` when
/// reaching 0 via dampening. No-op when disabled or rate is 0.
///
/// `is_stunned()` returns `shock >= max_shock && enabled`.
///
/// `is_settled()` returns `shock == 0.0` (not gated by `enabled`).
///
/// `shock_fraction()` returns `(shock / max_shock).clamp(0, 1)`.
///
/// `effective_impact(scale)` returns `scale * shock_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 12.0)` — dampens at 12 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zounds {
    pub shock: f32,
    pub max_shock: f32,
    pub dampening_rate: f32,
    pub just_stunned: bool,
    pub just_settled: bool,
    pub enabled: bool,
}

impl Zounds {
    pub fn new(max_shock: f32, dampening_rate: f32) -> Self {
        Self {
            shock: 0.0,
            max_shock: max_shock.max(0.1),
            dampening_rate: dampening_rate.max(0.0),
            just_stunned: false,
            just_settled: false,
            enabled: true,
        }
    }

    /// Add shock; fires `just_stunned` when first reaching max.
    /// No-op when disabled.
    pub fn jolt(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.shock < self.max_shock;
        self.shock = (self.shock + amount).min(self.max_shock);
        if was_below && self.shock >= self.max_shock {
            self.just_stunned = true;
        }
    }

    /// Reduce shock; fires `just_settled` when reaching 0.
    /// No-op when disabled or already settled.
    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.shock <= 0.0 {
            return;
        }
        self.shock = (self.shock - amount).max(0.0);
        if self.shock <= 0.0 {
            self.just_settled = true;
        }
    }

    /// Clear flags, then dampen shock by `dampening_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_stunned = false;
        self.just_settled = false;
        if self.enabled && self.dampening_rate > 0.0 && self.shock > 0.0 {
            self.shock = (self.shock - self.dampening_rate * dt).max(0.0);
            if self.shock <= 0.0 {
                self.just_settled = true;
            }
        }
    }

    /// `true` when shock is at maximum and component is enabled.
    pub fn is_stunned(&self) -> bool {
        self.shock >= self.max_shock && self.enabled
    }

    /// `true` when shock is 0 (not gated by `enabled`).
    pub fn is_settled(&self) -> bool {
        self.shock == 0.0
    }

    /// Fraction of maximum shock [0.0, 1.0].
    pub fn shock_fraction(&self) -> f32 {
        (self.shock / self.max_shock).clamp(0.0, 1.0)
    }

    /// Returns `scale * shock_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_impact(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.shock_fraction()
    }
}

impl Default for Zounds {
    fn default() -> Self {
        Self::new(100.0, 12.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zounds {
        Zounds::new(100.0, 12.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_settled() {
        let z = z();
        assert_eq!(z.shock, 0.0);
        assert!(z.is_settled());
        assert!(!z.is_stunned());
    }

    #[test]
    fn new_clamps_max_shock() {
        let z = Zounds::new(-5.0, 12.0);
        assert!((z.max_shock - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_dampening_rate() {
        let z = Zounds::new(100.0, -3.0);
        assert_eq!(z.dampening_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zounds::default();
        assert!((z.max_shock - 100.0).abs() < 1e-5);
        assert!((z.dampening_rate - 12.0).abs() < 1e-5);
    }

    // --- jolt ---

    #[test]
    fn jolt_adds_shock() {
        let mut z = z();
        z.jolt(40.0);
        assert!((z.shock - 40.0).abs() < 1e-3);
    }

    #[test]
    fn jolt_clamps_at_max() {
        let mut z = z();
        z.jolt(200.0);
        assert!((z.shock - 100.0).abs() < 1e-3);
    }

    #[test]
    fn jolt_fires_just_stunned_at_max() {
        let mut z = z();
        z.jolt(100.0);
        assert!(z.just_stunned);
        assert!(z.is_stunned());
    }

    #[test]
    fn jolt_no_just_stunned_when_already_at_max() {
        let mut z = z();
        z.shock = 100.0;
        z.jolt(10.0);
        assert!(!z.just_stunned);
    }

    #[test]
    fn jolt_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.jolt(50.0);
        assert_eq!(z.shock, 0.0);
    }

    #[test]
    fn jolt_no_op_when_amount_zero() {
        let mut z = z();
        z.jolt(0.0);
        assert_eq!(z.shock, 0.0);
    }

    // --- settle ---

    #[test]
    fn settle_reduces_shock() {
        let mut z = z();
        z.shock = 60.0;
        z.settle(20.0);
        assert!((z.shock - 40.0).abs() < 1e-3);
    }

    #[test]
    fn settle_clamps_at_zero() {
        let mut z = z();
        z.shock = 30.0;
        z.settle(200.0);
        assert_eq!(z.shock, 0.0);
    }

    #[test]
    fn settle_fires_just_settled_at_zero() {
        let mut z = z();
        z.shock = 30.0;
        z.settle(30.0);
        assert!(z.just_settled);
    }

    #[test]
    fn settle_no_op_when_already_settled() {
        let mut z = z();
        z.settle(10.0);
        assert!(!z.just_settled);
    }

    #[test]
    fn settle_no_op_when_disabled() {
        let mut z = z();
        z.shock = 50.0;
        z.enabled = false;
        z.settle(50.0);
        assert!((z.shock - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_dampens_shock() {
        let mut z = z(); // dampening=12
        z.shock = 60.0;
        z.tick(1.0); // 60 - 12 = 48
        assert!((z.shock - 48.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_settled_on_dampen_to_zero() {
        let mut z = Zounds::new(100.0, 200.0);
        z.shock = 5.0;
        z.tick(1.0);
        assert!(z.just_settled);
        assert!(z.is_settled());
    }

    #[test]
    fn tick_no_dampening_when_already_settled() {
        let mut z = z();
        z.tick(10.0);
        assert!(!z.just_settled);
    }

    #[test]
    fn tick_no_dampening_when_rate_zero() {
        let mut z = Zounds::new(100.0, 0.0);
        z.shock = 50.0;
        z.tick(100.0);
        assert!((z.shock - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_dampening_when_disabled() {
        let mut z = z();
        z.shock = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.shock - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_stunned() {
        let mut z = z();
        z.jolt(100.0);
        z.tick(0.016);
        assert!(!z.just_stunned);
    }

    #[test]
    fn tick_clears_just_settled() {
        let mut z = Zounds::new(100.0, 200.0);
        z.shock = 5.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_settled);
    }

    #[test]
    fn tick_scales_dampening_with_dt() {
        let mut z = z(); // dampening=12
        z.shock = 100.0;
        z.tick(2.0); // 100 - 12*2 = 76
        assert!((z.shock - 76.0).abs() < 1e-3);
    }

    // --- is_stunned / is_settled ---

    #[test]
    fn is_stunned_false_when_disabled() {
        let mut z = z();
        z.shock = 100.0;
        z.enabled = false;
        assert!(!z.is_stunned());
    }

    #[test]
    fn is_settled_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_settled());
    }

    // --- shock_fraction / effective_impact ---

    #[test]
    fn shock_fraction_zero_when_settled() {
        assert_eq!(z().shock_fraction(), 0.0);
    }

    #[test]
    fn shock_fraction_half_at_midpoint() {
        let mut z = z();
        z.shock = 50.0;
        assert!((z.shock_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_impact_zero_when_settled() {
        assert_eq!(z().effective_impact(100.0), 0.0);
    }

    #[test]
    fn effective_impact_scales_with_shock() {
        let mut z = z();
        z.shock = 75.0;
        assert!((z.effective_impact(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_impact_zero_when_disabled() {
        let mut z = z();
        z.shock = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_impact(100.0), 0.0);
    }
}
