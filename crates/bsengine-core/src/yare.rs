use bevy_ecs::prelude::Component;

/// Readiness-and-nimbleness tracker. "Yare" (nautical) means ready for
/// action. `readiness` in [0, max_readiness] starts full. `exert(amount)`
/// depletes it when acting; `prime(amount)` refills it deliberately;
/// `tick(dt)` optionally recovers it passively at `recovery_rate` per second.
///
/// Models action-point pools, burst-readiness gauges, nimbleness ratings,
/// or any mechanic where preparation matters and spending readiness costs.
///
/// `prime(amount)` adds to readiness when below max. Fires `just_primed`
/// on first reaching `max_readiness`. No-op when disabled or already at max.
///
/// `exert(amount)` reduces readiness when positive balance exists. Fires
/// `just_exhausted` when readiness first reaches 0. No-op when disabled.
///
/// `tick(dt)` clears `just_primed` and `just_exhausted`. Then (when enabled
/// and `recovery_rate > 0`) calls `prime(recovery_rate * dt)`.
///
/// `is_primed()` returns `readiness >= max_readiness && enabled`.
///
/// `is_exhausted()` returns `readiness == 0.0` (not gated by `enabled`).
///
/// `readiness_fraction()` returns `(readiness / max_readiness).clamp(0, 1)`.
///
/// `effective_agility(base)` returns `base * readiness_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 0.0)` — starts full, no passive recovery.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yare {
    pub readiness: f32,
    pub max_readiness: f32,
    pub recovery_rate: f32,
    pub just_primed: bool,
    pub just_exhausted: bool,
    pub enabled: bool,
}

impl Yare {
    pub fn new(max_readiness: f32, recovery_rate: f32) -> Self {
        let max = max_readiness.max(0.1);
        Self {
            readiness: max,
            max_readiness: max,
            recovery_rate: recovery_rate.max(0.0),
            just_primed: false,
            just_exhausted: false,
            enabled: true,
        }
    }

    /// Add to readiness; fires `just_primed` on first reaching max.
    /// No-op when disabled or already at max.
    pub fn prime(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.readiness >= self.max_readiness {
            return;
        }
        self.readiness = (self.readiness + amount).min(self.max_readiness);
        if self.readiness >= self.max_readiness {
            self.just_primed = true;
        }
    }

    /// Subtract from readiness; fires `just_exhausted` on first reaching 0.
    /// No-op when disabled or readiness already 0.
    pub fn exert(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.readiness <= 0.0 {
            return;
        }
        self.readiness = (self.readiness - amount).max(0.0);
        if self.readiness <= 0.0 {
            self.just_exhausted = true;
        }
    }

    /// Advance one frame: clear flags, then passively recover when enabled
    /// and `recovery_rate > 0`.
    pub fn tick(&mut self, dt: f32) {
        self.just_primed = false;
        self.just_exhausted = false;
        if self.enabled && self.recovery_rate > 0.0 {
            self.prime(self.recovery_rate * dt);
        }
    }

    /// `true` when readiness is at maximum and component is enabled.
    pub fn is_primed(&self) -> bool {
        self.readiness >= self.max_readiness && self.enabled
    }

    /// `true` when readiness is 0 (not gated by `enabled`).
    pub fn is_exhausted(&self) -> bool {
        self.readiness == 0.0
    }

    /// Fraction of maximum readiness [0.0, 1.0].
    pub fn readiness_fraction(&self) -> f32 {
        (self.readiness / self.max_readiness).clamp(0.0, 1.0)
    }

    /// Returns `base * readiness_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_agility(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.readiness_fraction()
    }
}

impl Default for Yare {
    fn default() -> Self {
        Self::new(100.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yare {
        Yare::new(100.0, 0.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_full() {
        let y = y();
        assert!((y.readiness - 100.0).abs() < 1e-5);
        assert!(y.is_primed());
        assert!(!y.is_exhausted());
    }

    #[test]
    fn new_clamps_max_readiness() {
        let y = Yare::new(-5.0, 0.0);
        assert!((y.max_readiness - 0.1).abs() < 1e-5);
        assert!((y.readiness - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_recovery_rate() {
        let y = Yare::new(100.0, -5.0);
        assert_eq!(y.recovery_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yare::default();
        assert!((y.max_readiness - 100.0).abs() < 1e-5);
        assert_eq!(y.recovery_rate, 0.0);
        assert!((y.readiness - 100.0).abs() < 1e-5);
    }

    // --- prime ---

    #[test]
    fn prime_increases_readiness() {
        let mut y = y();
        y.exert(60.0); // drain to 40
        y.prime(20.0);
        assert!((y.readiness - 60.0).abs() < 1e-3);
    }

    #[test]
    fn prime_clamps_at_max() {
        let mut y = y();
        y.exert(10.0); // 90
        y.prime(200.0);
        assert!((y.readiness - 100.0).abs() < 1e-5);
    }

    #[test]
    fn prime_fires_just_primed_on_reaching_max() {
        let mut y = y();
        y.exert(50.0);
        y.prime(50.0);
        assert!(y.just_primed);
        assert!(y.is_primed());
    }

    #[test]
    fn prime_no_op_when_already_at_max() {
        let mut y = y(); // starts full
        y.prime(10.0);
        assert!(!y.just_primed);
        assert!((y.readiness - 100.0).abs() < 1e-5);
    }

    #[test]
    fn prime_no_op_when_disabled() {
        let mut y = y();
        y.exert(50.0);
        y.enabled = false;
        y.prime(50.0);
        assert!((y.readiness - 50.0).abs() < 1e-3);
    }

    #[test]
    fn prime_no_op_for_zero_amount() {
        let mut y = y();
        y.exert(50.0);
        y.prime(0.0);
        assert!((y.readiness - 50.0).abs() < 1e-3);
    }

    // --- exert ---

    #[test]
    fn exert_reduces_readiness() {
        let mut y = y();
        y.exert(30.0);
        assert!((y.readiness - 70.0).abs() < 1e-3);
    }

    #[test]
    fn exert_clamps_at_zero() {
        let mut y = y();
        y.exert(200.0);
        assert_eq!(y.readiness, 0.0);
    }

    #[test]
    fn exert_fires_just_exhausted_at_zero() {
        let mut y = y();
        y.exert(100.0);
        assert!(y.just_exhausted);
        assert!(y.is_exhausted());
    }

    #[test]
    fn exert_no_op_when_already_exhausted() {
        let mut y = y();
        y.exert(100.0);
        y.exert(10.0); // already 0
        assert!(y.just_exhausted); // only first time
    }

    #[test]
    fn exert_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.exert(50.0);
        assert!((y.readiness - 100.0).abs() < 1e-3);
    }

    #[test]
    fn exert_no_op_for_zero_amount() {
        let mut y = y();
        y.exert(0.0);
        assert!((y.readiness - 100.0).abs() < 1e-3);
    }

    #[test]
    fn exert_accumulates() {
        let mut y = y();
        y.exert(20.0);
        y.exert(20.0);
        assert!((y.readiness - 60.0).abs() < 1e-3);
    }

    // --- tick (recovery) ---

    #[test]
    fn tick_clears_just_primed() {
        let mut y = y();
        y.exert(50.0);
        y.prime(50.0);
        y.tick(0.016);
        assert!(!y.just_primed);
    }

    #[test]
    fn tick_clears_just_exhausted() {
        let mut y = y();
        y.exert(100.0);
        y.tick(0.016);
        assert!(!y.just_exhausted);
    }

    #[test]
    fn tick_recovers_when_rate_set() {
        let mut y = Yare::new(100.0, 10.0);
        y.exert(50.0); // 50 remaining
        y.tick(1.0); // recover 10 → 60
        assert!((y.readiness - 60.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_recovery_at_max() {
        let mut y = Yare::new(100.0, 10.0);
        y.tick(1.0); // already full
        assert!((y.readiness - 100.0).abs() < 1e-5);
        assert!(!y.just_primed);
    }

    #[test]
    fn tick_no_recovery_when_rate_zero() {
        let mut y = y();
        y.exert(50.0);
        y.tick(100.0); // no recovery
        assert!((y.readiness - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_recovery_when_disabled() {
        let mut y = Yare::new(100.0, 10.0);
        y.exert(50.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.readiness - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_recovery_fires_just_primed() {
        let mut y = Yare::new(100.0, 100.0);
        y.exert(50.0); // 50 remaining
        y.tick(1.0); // recover 100 → full
        assert!(y.just_primed);
    }

    // --- is_primed / is_exhausted ---

    #[test]
    fn is_primed_false_below_max() {
        let mut y = y();
        y.exert(1.0);
        assert!(!y.is_primed());
    }

    #[test]
    fn is_primed_false_when_disabled() {
        let mut y = y(); // starts full
        y.enabled = false;
        assert!(!y.is_primed());
    }

    #[test]
    fn is_exhausted_false_when_above_zero() {
        let mut y = y();
        y.exert(50.0);
        assert!(!y.is_exhausted());
    }

    #[test]
    fn is_exhausted_not_gated_by_enabled() {
        let mut y = y();
        y.exert(100.0);
        y.enabled = false;
        assert!(y.is_exhausted());
    }

    // --- fractions / effective ---

    #[test]
    fn readiness_fraction_one_at_max() {
        assert!((y().readiness_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn readiness_fraction_half_at_midpoint() {
        let mut y = y();
        y.exert(50.0);
        assert!((y.readiness_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn readiness_fraction_zero_when_exhausted() {
        let mut y = y();
        y.exert(100.0);
        assert_eq!(y.readiness_fraction(), 0.0);
    }

    #[test]
    fn effective_agility_full_when_primed() {
        assert!((y().effective_agility(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_agility_scales_with_fraction() {
        let mut y = y();
        y.exert(75.0);
        assert!((y.effective_agility(100.0) - 25.0).abs() < 1e-3);
    }

    #[test]
    fn effective_agility_zero_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert_eq!(y.effective_agility(100.0), 0.0);
    }
}
