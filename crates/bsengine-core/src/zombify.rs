use bevy_ecs::prelude::Component;

/// Zombification-taint tracker. `taint` builds via `infect(amount)` and
/// spreads passively at `spread_rate` per second in `tick(dt)` or is
/// purged immediately via `purify(amount)`.
///
/// Models zombie-virus spread meters, undead-infection progress bars,
/// corruption-taint accumulators, dark-magic contamination gauges,
/// pathogen-load trackers, blight-contagion fill levels,
/// necromantic-influence indicators, or any mechanic where accumulated
/// taint eventually converts the target into an enemy.
///
/// `infect(amount)` adds taint; fires `just_zombified` when first
/// reaching `max_taint`. No-op when disabled.
///
/// `purify(amount)` reduces taint immediately; fires `just_purified`
/// when reaching 0. No-op when disabled or already purified.
///
/// `tick(dt)` clears both flags, then increases taint by
/// `spread_rate * dt` (capped at `max_taint`). Fires `just_zombified`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_zombified()` returns `taint >= max_taint && enabled`.
///
/// `is_purified()` returns `taint == 0.0` (not gated by `enabled`).
///
/// `taint_fraction()` returns `(taint / max_taint).clamp(0, 1)`.
///
/// `effective_corruption(scale)` returns `scale * taint_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — spreads taint at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zombify {
    pub taint: f32,
    pub max_taint: f32,
    pub spread_rate: f32,
    pub just_zombified: bool,
    pub just_purified: bool,
    pub enabled: bool,
}

impl Zombify {
    pub fn new(max_taint: f32, spread_rate: f32) -> Self {
        Self {
            taint: 0.0,
            max_taint: max_taint.max(0.1),
            spread_rate: spread_rate.max(0.0),
            just_zombified: false,
            just_purified: false,
            enabled: true,
        }
    }

    /// Add taint; fires `just_zombified` when first reaching max.
    /// No-op when disabled.
    pub fn infect(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.taint < self.max_taint;
        self.taint = (self.taint + amount).min(self.max_taint);
        if was_below && self.taint >= self.max_taint {
            self.just_zombified = true;
        }
    }

    /// Reduce taint; fires `just_purified` when reaching 0.
    /// No-op when disabled or already purified.
    pub fn purify(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.taint <= 0.0 {
            return;
        }
        self.taint = (self.taint - amount).max(0.0);
        if self.taint <= 0.0 {
            self.just_purified = true;
        }
    }

    /// Clear flags, then increase taint by `spread_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_zombified = false;
        self.just_purified = false;
        if self.enabled && self.spread_rate > 0.0 && self.taint < self.max_taint {
            let was_below = self.taint < self.max_taint;
            self.taint = (self.taint + self.spread_rate * dt).min(self.max_taint);
            if was_below && self.taint >= self.max_taint {
                self.just_zombified = true;
            }
        }
    }

    /// `true` when taint is at maximum and component is enabled.
    pub fn is_zombified(&self) -> bool {
        self.taint >= self.max_taint && self.enabled
    }

    /// `true` when taint is 0 (not gated by `enabled`).
    pub fn is_purified(&self) -> bool {
        self.taint == 0.0
    }

    /// Fraction of maximum taint [0.0, 1.0].
    pub fn taint_fraction(&self) -> f32 {
        (self.taint / self.max_taint).clamp(0.0, 1.0)
    }

    /// Returns `scale * taint_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_corruption(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.taint_fraction()
    }
}

impl Default for Zombify {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zombify {
        Zombify::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_purified() {
        let z = z();
        assert_eq!(z.taint, 0.0);
        assert!(z.is_purified());
        assert!(!z.is_zombified());
    }

    #[test]
    fn new_clamps_max_taint() {
        let z = Zombify::new(-5.0, 4.0);
        assert!((z.max_taint - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spread_rate() {
        let z = Zombify::new(100.0, -3.0);
        assert_eq!(z.spread_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zombify::default();
        assert!((z.max_taint - 100.0).abs() < 1e-5);
        assert!((z.spread_rate - 4.0).abs() < 1e-5);
    }

    // --- infect ---

    #[test]
    fn infect_adds_taint() {
        let mut z = z();
        z.infect(40.0);
        assert!((z.taint - 40.0).abs() < 1e-3);
    }

    #[test]
    fn infect_clamps_at_max() {
        let mut z = z();
        z.infect(200.0);
        assert!((z.taint - 100.0).abs() < 1e-3);
    }

    #[test]
    fn infect_fires_just_zombified_at_max() {
        let mut z = z();
        z.infect(100.0);
        assert!(z.just_zombified);
        assert!(z.is_zombified());
    }

    #[test]
    fn infect_no_just_zombified_when_already_at_max() {
        let mut z = z();
        z.taint = 100.0;
        z.infect(10.0);
        assert!(!z.just_zombified);
    }

    #[test]
    fn infect_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.infect(50.0);
        assert_eq!(z.taint, 0.0);
    }

    #[test]
    fn infect_no_op_when_amount_zero() {
        let mut z = z();
        z.infect(0.0);
        assert_eq!(z.taint, 0.0);
    }

    // --- purify ---

    #[test]
    fn purify_reduces_taint() {
        let mut z = z();
        z.taint = 60.0;
        z.purify(20.0);
        assert!((z.taint - 40.0).abs() < 1e-3);
    }

    #[test]
    fn purify_clamps_at_zero() {
        let mut z = z();
        z.taint = 30.0;
        z.purify(200.0);
        assert_eq!(z.taint, 0.0);
    }

    #[test]
    fn purify_fires_just_purified_at_zero() {
        let mut z = z();
        z.taint = 30.0;
        z.purify(30.0);
        assert!(z.just_purified);
    }

    #[test]
    fn purify_no_op_when_already_purified() {
        let mut z = z();
        z.purify(10.0);
        assert!(!z.just_purified);
    }

    #[test]
    fn purify_no_op_when_disabled() {
        let mut z = z();
        z.taint = 50.0;
        z.enabled = false;
        z.purify(50.0);
        assert!((z.taint - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_spreads_taint() {
        let mut z = z(); // rate=4
        z.tick(1.0); // 0 + 4 = 4
        assert!((z.taint - 4.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_zombified_on_spread_to_max() {
        let mut z = Zombify::new(100.0, 200.0);
        z.taint = 95.0;
        z.tick(1.0);
        assert!(z.just_zombified);
        assert!(z.is_zombified());
    }

    #[test]
    fn tick_no_spread_when_already_zombified() {
        let mut z = z();
        z.taint = 100.0;
        z.tick(1.0);
        assert!(!z.just_zombified);
    }

    #[test]
    fn tick_no_spread_when_rate_zero() {
        let mut z = Zombify::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.taint, 0.0);
    }

    #[test]
    fn tick_no_spread_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.taint, 0.0);
    }

    #[test]
    fn tick_clears_just_zombified() {
        let mut z = Zombify::new(100.0, 200.0);
        z.taint = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_zombified);
    }

    #[test]
    fn tick_clears_just_purified() {
        let mut z = z();
        z.taint = 10.0;
        z.purify(10.0);
        z.tick(0.016);
        assert!(!z.just_purified);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=4
        z.tick(3.0); // 4*3 = 12
        assert!((z.taint - 12.0).abs() < 1e-3);
    }

    // --- is_zombified / is_purified ---

    #[test]
    fn is_zombified_false_when_disabled() {
        let mut z = z();
        z.taint = 100.0;
        z.enabled = false;
        assert!(!z.is_zombified());
    }

    #[test]
    fn is_purified_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_purified());
    }

    // --- taint_fraction / effective_corruption ---

    #[test]
    fn taint_fraction_zero_when_purified() {
        assert_eq!(z().taint_fraction(), 0.0);
    }

    #[test]
    fn taint_fraction_half_at_midpoint() {
        let mut z = z();
        z.taint = 50.0;
        assert!((z.taint_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_corruption_zero_when_purified() {
        assert_eq!(z().effective_corruption(100.0), 0.0);
    }

    #[test]
    fn effective_corruption_scales_with_taint() {
        let mut z = z();
        z.taint = 75.0;
        assert!((z.effective_corruption(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_corruption_zero_when_disabled() {
        let mut z = z();
        z.taint = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_corruption(100.0), 0.0);
    }
}
