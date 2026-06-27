use bevy_ecs::prelude::Component;

/// Comfort-support tracker. `support` builds via `rest(amount)` and
/// recovers passively at `recovery_rate` per second in `tick(dt)` or
/// drops immediately via `fatigue(amount)`.
///
/// Models cushion/support meters, ergonomic-comfort gauges, stamina-
/// recovery bars, rest-quality trackers, NPC recuperation states, or any
/// mechanic where an entity gradually regains comfort when allowed to rest
/// and loses it under sustained effort.
///
/// `rest(amount)` adds support; fires `just_supported` when first reaching
/// `max_support`. No-op when disabled.
///
/// `fatigue(amount)` reduces support immediately; fires `just_exhausted`
/// when reaching 0. No-op when disabled or already exhausted.
///
/// `tick(dt)` clears both flags, then recovers support by
/// `recovery_rate * dt` (capped at `max_support`). Fires `just_supported`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_supported()` returns `support >= max_support && enabled`.
///
/// `is_exhausted()` returns `support == 0.0` (not gated by `enabled`).
///
/// `support_fraction()` returns `(support / max_support).clamp(0, 1)`.
///
/// `effective_comfort(scale)` returns `scale * support_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 3.0)` — recovers at 3 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zafu {
    pub support: f32,
    pub max_support: f32,
    pub recovery_rate: f32,
    pub just_supported: bool,
    pub just_exhausted: bool,
    pub enabled: bool,
}

impl Zafu {
    pub fn new(max_support: f32, recovery_rate: f32) -> Self {
        Self {
            support: 0.0,
            max_support: max_support.max(0.1),
            recovery_rate: recovery_rate.max(0.0),
            just_supported: false,
            just_exhausted: false,
            enabled: true,
        }
    }

    /// Add support; fires `just_supported` when first reaching max.
    /// No-op when disabled.
    pub fn rest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.support < self.max_support;
        self.support = (self.support + amount).min(self.max_support);
        if was_below && self.support >= self.max_support {
            self.just_supported = true;
        }
    }

    /// Reduce support; fires `just_exhausted` when reaching 0.
    /// No-op when disabled or already exhausted.
    pub fn fatigue(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.support <= 0.0 {
            return;
        }
        self.support = (self.support - amount).max(0.0);
        if self.support <= 0.0 {
            self.just_exhausted = true;
        }
    }

    /// Clear flags, then recover support by `recovery_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_supported = false;
        self.just_exhausted = false;
        if self.enabled && self.recovery_rate > 0.0 && self.support < self.max_support {
            let was_below = self.support < self.max_support;
            self.support = (self.support + self.recovery_rate * dt).min(self.max_support);
            if was_below && self.support >= self.max_support {
                self.just_supported = true;
            }
        }
    }

    /// `true` when support is at maximum and component is enabled.
    pub fn is_supported(&self) -> bool {
        self.support >= self.max_support && self.enabled
    }

    /// `true` when support is 0 (not gated by `enabled`).
    pub fn is_exhausted(&self) -> bool {
        self.support == 0.0
    }

    /// Fraction of maximum support [0.0, 1.0].
    pub fn support_fraction(&self) -> f32 {
        (self.support / self.max_support).clamp(0.0, 1.0)
    }

    /// Returns `scale * support_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_comfort(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.support_fraction()
    }
}

impl Default for Zafu {
    fn default() -> Self {
        Self::new(100.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zafu {
        Zafu::new(100.0, 3.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_exhausted() {
        let z = z();
        assert_eq!(z.support, 0.0);
        assert!(z.is_exhausted());
        assert!(!z.is_supported());
    }

    #[test]
    fn new_clamps_max_support() {
        let z = Zafu::new(-5.0, 3.0);
        assert!((z.max_support - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_recovery_rate() {
        let z = Zafu::new(100.0, -3.0);
        assert_eq!(z.recovery_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zafu::default();
        assert!((z.max_support - 100.0).abs() < 1e-5);
        assert!((z.recovery_rate - 3.0).abs() < 1e-5);
    }

    // --- rest ---

    #[test]
    fn rest_adds_support() {
        let mut z = z();
        z.rest(40.0);
        assert!((z.support - 40.0).abs() < 1e-3);
    }

    #[test]
    fn rest_clamps_at_max() {
        let mut z = z();
        z.rest(200.0);
        assert!((z.support - 100.0).abs() < 1e-3);
    }

    #[test]
    fn rest_fires_just_supported_at_max() {
        let mut z = z();
        z.rest(100.0);
        assert!(z.just_supported);
        assert!(z.is_supported());
    }

    #[test]
    fn rest_no_just_supported_when_already_at_max() {
        let mut z = z();
        z.support = 100.0;
        z.rest(10.0);
        assert!(!z.just_supported);
    }

    #[test]
    fn rest_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.rest(50.0);
        assert_eq!(z.support, 0.0);
    }

    #[test]
    fn rest_no_op_when_amount_zero() {
        let mut z = z();
        z.rest(0.0);
        assert_eq!(z.support, 0.0);
    }

    // --- fatigue ---

    #[test]
    fn fatigue_reduces_support() {
        let mut z = z();
        z.support = 60.0;
        z.fatigue(20.0);
        assert!((z.support - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fatigue_clamps_at_zero() {
        let mut z = z();
        z.support = 30.0;
        z.fatigue(200.0);
        assert_eq!(z.support, 0.0);
    }

    #[test]
    fn fatigue_fires_just_exhausted_at_zero() {
        let mut z = z();
        z.support = 30.0;
        z.fatigue(30.0);
        assert!(z.just_exhausted);
    }

    #[test]
    fn fatigue_no_op_when_already_exhausted() {
        let mut z = z();
        z.fatigue(10.0);
        assert!(!z.just_exhausted);
    }

    #[test]
    fn fatigue_no_op_when_disabled() {
        let mut z = z();
        z.support = 50.0;
        z.enabled = false;
        z.fatigue(50.0);
        assert!((z.support - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_recovers_support() {
        let mut z = z(); // recovery=3
        z.tick(1.0); // 0 + 3 = 3
        assert!((z.support - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_supported_on_recover_to_max() {
        let mut z = Zafu::new(100.0, 200.0);
        z.support = 95.0;
        z.tick(1.0);
        assert!(z.just_supported);
        assert!(z.is_supported());
    }

    #[test]
    fn tick_no_recovery_when_already_at_max() {
        let mut z = z();
        z.support = 100.0;
        z.tick(1.0);
        assert!(!z.just_supported);
    }

    #[test]
    fn tick_no_recovery_when_rate_zero() {
        let mut z = Zafu::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.support, 0.0);
    }

    #[test]
    fn tick_no_recovery_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.support, 0.0);
    }

    #[test]
    fn tick_clears_just_supported() {
        let mut z = Zafu::new(100.0, 200.0);
        z.support = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_supported);
    }

    #[test]
    fn tick_clears_just_exhausted() {
        let mut z = z();
        z.support = 10.0;
        z.fatigue(10.0);
        z.tick(0.016);
        assert!(!z.just_exhausted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // recovery=3
        z.tick(7.0); // 3*7 = 21
        assert!((z.support - 21.0).abs() < 1e-3);
    }

    // --- is_supported / is_exhausted ---

    #[test]
    fn is_supported_false_when_disabled() {
        let mut z = z();
        z.support = 100.0;
        z.enabled = false;
        assert!(!z.is_supported());
    }

    #[test]
    fn is_exhausted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_exhausted());
    }

    // --- support_fraction / effective_comfort ---

    #[test]
    fn support_fraction_zero_when_exhausted() {
        assert_eq!(z().support_fraction(), 0.0);
    }

    #[test]
    fn support_fraction_half_at_midpoint() {
        let mut z = z();
        z.support = 50.0;
        assert!((z.support_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_comfort_zero_when_exhausted() {
        assert_eq!(z().effective_comfort(100.0), 0.0);
    }

    #[test]
    fn effective_comfort_scales_with_support() {
        let mut z = z();
        z.support = 65.0;
        assert!((z.effective_comfort(100.0) - 65.0).abs() < 1e-3);
    }

    #[test]
    fn effective_comfort_zero_when_disabled() {
        let mut z = z();
        z.support = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_comfort(100.0), 0.0);
    }
}
