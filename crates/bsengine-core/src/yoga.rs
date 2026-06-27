use bevy_ecs::prelude::Component;

/// Flexibility and centering tracker. `flexibility` starts at its maximum
/// (fully limber). `strain(amount)` erodes it; `practice(amount)` restores
/// it; `tick(dt)` applies passive recovery at `recovery_rate` per second.
/// Fires `just_centered` on first reaching max; `just_broken` on first
/// reaching 0.
///
/// Starts full — unlike accumulators that build from 0, yoga represents
/// an existing capability that degrades under stress and recovers through
/// deliberate practice or rest.
///
/// Models character limberness, balance bars, meditation / focus meters,
/// strain accumulators for acrobatic moves, or any stat that erodes under
/// use and recovers passively.
///
/// `practice(amount)` restores flexibility (capped at `max_flexibility`).
/// Fires `just_centered` on first reaching max. No-op when disabled.
///
/// `strain(amount)` reduces flexibility. Fires `just_broken` when reaching 0.
/// No-op when disabled.
///
/// `tick(dt)` clears `just_centered` and `just_broken`. Then (when enabled
/// and `recovery_rate > 0`) restores flexibility by `recovery_rate * dt`,
/// capped at max. Fires `just_centered` if flexibility reaches max.
///
/// `is_centered()` returns `flexibility >= max_flexibility && enabled`.
///
/// `is_broken()` returns `flexibility == 0.0` (not gated by `enabled`).
///
/// `flexibility_fraction()` returns `(flexibility / max_flexibility).clamp(0, 1)`.
///
/// `effective_agility(base)` returns `base * flexibility_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — starts fully centered, 5/sec passive recovery.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yoga {
    pub flexibility: f32,
    pub max_flexibility: f32,
    pub recovery_rate: f32,
    pub just_centered: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Yoga {
    pub fn new(max_flexibility: f32, recovery_rate: f32) -> Self {
        let max = max_flexibility.max(0.1);
        Self {
            flexibility: max,
            max_flexibility: max,
            recovery_rate: recovery_rate.max(0.0),
            just_centered: false,
            just_broken: false,
            enabled: true,
        }
    }

    /// Restore flexibility; fires `just_centered` on first reaching max.
    /// No-op when disabled or already at max.
    pub fn practice(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.flexibility >= self.max_flexibility {
            return;
        }
        self.flexibility = (self.flexibility + amount).min(self.max_flexibility);
        if self.flexibility >= self.max_flexibility {
            self.just_centered = true;
        }
    }

    /// Erode flexibility; fires `just_broken` when reaching 0.
    /// No-op when disabled or flexibility already 0.
    pub fn strain(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.flexibility <= 0.0 {
            return;
        }
        self.flexibility = (self.flexibility - amount).max(0.0);
        if self.flexibility <= 0.0 {
            self.just_broken = true;
        }
    }

    /// Advance one frame: clear flags, then recover flexibility passively
    /// when enabled and `recovery_rate > 0`. Fires `just_centered` if
    /// flexibility reaches max.
    pub fn tick(&mut self, dt: f32) {
        self.just_centered = false;
        self.just_broken = false;
        if self.enabled && self.recovery_rate > 0.0 {
            self.practice(self.recovery_rate * dt);
        }
    }

    /// `true` when flexibility is at maximum and component is enabled.
    pub fn is_centered(&self) -> bool {
        self.flexibility >= self.max_flexibility && self.enabled
    }

    /// `true` when flexibility is 0 (not gated by `enabled`).
    pub fn is_broken(&self) -> bool {
        self.flexibility == 0.0
    }

    /// Fraction of maximum flexibility [0.0, 1.0].
    pub fn flexibility_fraction(&self) -> f32 {
        (self.flexibility / self.max_flexibility).clamp(0.0, 1.0)
    }

    /// Returns `base * flexibility_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_agility(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.flexibility_fraction()
    }
}

impl Default for Yoga {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yoga {
        Yoga::new(100.0, 0.0) // no passive recovery; strain tests explicit
    }

    // --- construction ---

    #[test]
    fn new_starts_at_max() {
        let y = y();
        assert!((y.flexibility - 100.0).abs() < 1e-5);
        assert!(y.is_centered());
        assert!(!y.is_broken());
    }

    #[test]
    fn new_clamps_max_flexibility() {
        let y = Yoga::new(-5.0, 0.0);
        assert!((y.max_flexibility - 0.1).abs() < 1e-5);
        assert!((y.flexibility - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_recovery_rate() {
        let y = Yoga::new(100.0, -3.0);
        assert_eq!(y.recovery_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yoga::default();
        assert!((y.max_flexibility - 100.0).abs() < 1e-5);
        assert!((y.recovery_rate - 5.0).abs() < 1e-5);
        assert!((y.flexibility - 100.0).abs() < 1e-5);
    }

    // --- practice ---

    #[test]
    fn practice_restores_flexibility() {
        let mut y = y();
        y.strain(40.0);
        y.practice(20.0);
        assert!((y.flexibility - 80.0).abs() < 1e-3);
    }

    #[test]
    fn practice_clamps_at_max() {
        let mut y = y();
        y.strain(10.0);
        y.practice(200.0);
        assert!((y.flexibility - 100.0).abs() < 1e-5);
    }

    #[test]
    fn practice_fires_just_centered_at_max() {
        let mut y = y();
        y.strain(50.0);
        y.practice(50.0);
        assert!(y.just_centered);
        assert!(y.is_centered());
    }

    #[test]
    fn practice_no_op_when_already_centered() {
        let mut y = y();
        y.practice(10.0); // already at max
        assert!(!y.just_centered);
    }

    #[test]
    fn practice_no_op_when_disabled() {
        let mut y = y();
        y.strain(50.0);
        y.enabled = false;
        y.practice(50.0);
        assert!((y.flexibility - 50.0).abs() < 1e-3);
    }

    #[test]
    fn practice_no_op_for_zero_amount() {
        let mut y = y();
        y.strain(50.0);
        y.practice(0.0);
        assert!((y.flexibility - 50.0).abs() < 1e-3);
    }

    // --- strain ---

    #[test]
    fn strain_reduces_flexibility() {
        let mut y = y();
        y.strain(30.0);
        assert!((y.flexibility - 70.0).abs() < 1e-3);
    }

    #[test]
    fn strain_clamps_at_zero() {
        let mut y = y();
        y.strain(200.0);
        assert_eq!(y.flexibility, 0.0);
    }

    #[test]
    fn strain_fires_just_broken_at_zero() {
        let mut y = y();
        y.strain(100.0);
        assert!(y.just_broken);
        assert!(y.is_broken());
    }

    #[test]
    fn strain_no_op_when_already_broken() {
        let mut y = y();
        y.strain(100.0); // now 0
        y.strain(10.0); // already 0
        assert!(y.just_broken); // still set from first call
    }

    #[test]
    fn strain_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.strain(50.0);
        assert!((y.flexibility - 100.0).abs() < 1e-3);
    }

    #[test]
    fn strain_no_op_for_zero_amount() {
        let mut y = y();
        y.strain(0.0);
        assert!((y.flexibility - 100.0).abs() < 1e-3);
    }

    #[test]
    fn strain_accumulates() {
        let mut y = y();
        y.strain(30.0);
        y.strain(20.0);
        assert!((y.flexibility - 50.0).abs() < 1e-3);
    }

    // --- tick (passive recovery) ---

    #[test]
    fn tick_recovers_flexibility() {
        let mut y = Yoga::new(100.0, 10.0);
        y.strain(60.0);
        y.tick(1.0); // 40 + 10*1 = 50
        assert!((y.flexibility - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_recovery_at_max() {
        let mut y = Yoga::new(100.0, 100.0);
        y.strain(10.0);
        y.tick(1.0); // recovers past max, clamped
        assert!((y.flexibility - 100.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_centered_when_recovering_to_max() {
        let mut y = Yoga::new(100.0, 50.0);
        y.strain(10.0);
        y.tick(1.0); // 90 + 50 > 100
        assert!(y.just_centered);
    }

    #[test]
    fn tick_no_recovery_when_at_max() {
        let mut y = Yoga::new(100.0, 10.0);
        y.tick(1.0); // already at max
        assert!(!y.just_centered);
    }

    #[test]
    fn tick_no_recovery_when_rate_zero() {
        let mut y = y();
        y.strain(50.0);
        y.tick(100.0);
        assert!((y.flexibility - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_recovery_when_disabled() {
        let mut y = Yoga::new(100.0, 10.0);
        y.strain(50.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.flexibility - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_centered() {
        let mut y = Yoga::new(100.0, 50.0);
        y.strain(10.0);
        y.tick(1.0); // just_centered fires
        y.tick(0.016); // cleared
        assert!(!y.just_centered);
    }

    #[test]
    fn tick_clears_just_broken() {
        let mut y = y();
        y.strain(100.0); // just_broken fires
        y.tick(0.016); // cleared
        assert!(!y.just_broken);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = Yoga::new(100.0, 10.0);
        y.strain(60.0); // now 40
        y.tick(2.0); // 40 + 10*2 = 60
        assert!((y.flexibility - 60.0).abs() < 1e-2);
    }

    // --- is_centered / is_broken ---

    #[test]
    fn is_centered_false_below_max() {
        let mut y = y();
        y.strain(1.0);
        assert!(!y.is_centered());
    }

    #[test]
    fn is_centered_false_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(!y.is_centered());
    }

    #[test]
    fn is_broken_true_at_zero() {
        let mut y = y();
        y.strain(100.0);
        assert!(y.is_broken());
    }

    #[test]
    fn is_broken_not_gated_by_enabled() {
        let mut y = y();
        y.strain(100.0);
        y.enabled = false;
        assert!(y.is_broken());
    }

    // --- fractions / effective ---

    #[test]
    fn flexibility_fraction_one_at_start() {
        assert!((y().flexibility_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn flexibility_fraction_half_at_midpoint() {
        let mut y = y();
        y.strain(50.0);
        assert!((y.flexibility_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_agility_full_at_start() {
        assert!((y().effective_agility(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_agility_scales_with_fraction() {
        let mut y = y();
        y.strain(75.0);
        assert!((y.effective_agility(100.0) - 25.0).abs() < 1e-3);
    }

    #[test]
    fn effective_agility_zero_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert_eq!(y.effective_agility(100.0), 0.0);
    }
}
