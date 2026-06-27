use bevy_ecs::prelude::Component;

/// Leukotriene-inhibition saturation tracker. `coverage` builds via
/// `dose(amount)` and accumulates passively at `absorption_rate` per second
/// in `tick(dt)` or is cleared via `metabolize(amount)`.
///
/// Models asthma-medication coverage bars, leukotriene-pathway suppression
/// gauges, airway-inflammation inhibition fill levels, broncho-protection
/// accumulation trackers, prostaglandin-blockade saturation meters,
/// arachidonic-acid cascade suppression indicators, anti-inflammatory drug
/// bioavailability build-up bars, eosinophil-recruitment inhibition gauges,
/// or any mechanic where steadily accumulating pharmacological coverage
/// suppresses the inflammatory cascade until the dosing interval lapses
/// and the leukotriene pathway begins to unspool again.
///
/// `dose(amount)` adds coverage; fires `just_covered` when first
/// reaching `max_coverage`. No-op when disabled.
///
/// `metabolize(amount)` reduces coverage immediately; fires `just_lapsed`
/// when reaching 0. No-op when disabled or already lapsed.
///
/// `tick(dt)` clears both flags, then increases coverage by
/// `absorption_rate * dt` (capped at `max_coverage`). Fires `just_covered`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_covered()` returns `coverage >= max_coverage && enabled`.
///
/// `is_lapsed()` returns `coverage == 0.0` (not gated by `enabled`).
///
/// `coverage_fraction()` returns `(coverage / max_coverage).clamp(0, 1)`.
///
/// `effective_suppression(scale)` returns `scale * coverage_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — absorbs at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zileuton {
    pub coverage: f32,
    pub max_coverage: f32,
    pub absorption_rate: f32,
    pub just_covered: bool,
    pub just_lapsed: bool,
    pub enabled: bool,
}

impl Zileuton {
    pub fn new(max_coverage: f32, absorption_rate: f32) -> Self {
        Self {
            coverage: 0.0,
            max_coverage: max_coverage.max(0.1),
            absorption_rate: absorption_rate.max(0.0),
            just_covered: false,
            just_lapsed: false,
            enabled: true,
        }
    }

    /// Add coverage; fires `just_covered` when first reaching max.
    /// No-op when disabled.
    pub fn dose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.coverage < self.max_coverage;
        self.coverage = (self.coverage + amount).min(self.max_coverage);
        if was_below && self.coverage >= self.max_coverage {
            self.just_covered = true;
        }
    }

    /// Reduce coverage; fires `just_lapsed` when reaching 0.
    /// No-op when disabled or already lapsed.
    pub fn metabolize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.coverage <= 0.0 {
            return;
        }
        self.coverage = (self.coverage - amount).max(0.0);
        if self.coverage <= 0.0 {
            self.just_lapsed = true;
        }
    }

    /// Clear flags, then increase coverage by `absorption_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_covered = false;
        self.just_lapsed = false;
        if self.enabled && self.absorption_rate > 0.0 && self.coverage < self.max_coverage {
            let was_below = self.coverage < self.max_coverage;
            self.coverage = (self.coverage + self.absorption_rate * dt).min(self.max_coverage);
            if was_below && self.coverage >= self.max_coverage {
                self.just_covered = true;
            }
        }
    }

    /// `true` when coverage is at maximum and component is enabled.
    pub fn is_covered(&self) -> bool {
        self.coverage >= self.max_coverage && self.enabled
    }

    /// `true` when coverage is 0 (not gated by `enabled`).
    pub fn is_lapsed(&self) -> bool {
        self.coverage == 0.0
    }

    /// Fraction of maximum coverage [0.0, 1.0].
    pub fn coverage_fraction(&self) -> f32 {
        (self.coverage / self.max_coverage).clamp(0.0, 1.0)
    }

    /// Returns `scale * coverage_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_suppression(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.coverage_fraction()
    }
}

impl Default for Zileuton {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zileuton {
        Zileuton::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_lapsed() {
        let z = z();
        assert_eq!(z.coverage, 0.0);
        assert!(z.is_lapsed());
        assert!(!z.is_covered());
    }

    #[test]
    fn new_clamps_max_coverage() {
        let z = Zileuton::new(-5.0, 1.5);
        assert!((z.max_coverage - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_absorption_rate() {
        let z = Zileuton::new(100.0, -1.5);
        assert_eq!(z.absorption_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zileuton::default();
        assert!((z.max_coverage - 100.0).abs() < 1e-5);
        assert!((z.absorption_rate - 1.5).abs() < 1e-5);
    }

    // --- dose ---

    #[test]
    fn dose_adds_coverage() {
        let mut z = z();
        z.dose(40.0);
        assert!((z.coverage - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dose_clamps_at_max() {
        let mut z = z();
        z.dose(200.0);
        assert!((z.coverage - 100.0).abs() < 1e-3);
    }

    #[test]
    fn dose_fires_just_covered_at_max() {
        let mut z = z();
        z.dose(100.0);
        assert!(z.just_covered);
        assert!(z.is_covered());
    }

    #[test]
    fn dose_no_just_covered_when_already_at_max() {
        let mut z = z();
        z.coverage = 100.0;
        z.dose(10.0);
        assert!(!z.just_covered);
    }

    #[test]
    fn dose_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.dose(50.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn dose_no_op_when_amount_zero() {
        let mut z = z();
        z.dose(0.0);
        assert_eq!(z.coverage, 0.0);
    }

    // --- metabolize ---

    #[test]
    fn metabolize_reduces_coverage() {
        let mut z = z();
        z.coverage = 60.0;
        z.metabolize(20.0);
        assert!((z.coverage - 40.0).abs() < 1e-3);
    }

    #[test]
    fn metabolize_clamps_at_zero() {
        let mut z = z();
        z.coverage = 30.0;
        z.metabolize(200.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn metabolize_fires_just_lapsed_at_zero() {
        let mut z = z();
        z.coverage = 30.0;
        z.metabolize(30.0);
        assert!(z.just_lapsed);
    }

    #[test]
    fn metabolize_no_op_when_already_lapsed() {
        let mut z = z();
        z.metabolize(10.0);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn metabolize_no_op_when_disabled() {
        let mut z = z();
        z.coverage = 50.0;
        z.enabled = false;
        z.metabolize(50.0);
        assert!((z.coverage - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_absorbs_coverage() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.coverage - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_covered_on_absorb_to_max() {
        let mut z = Zileuton::new(100.0, 200.0);
        z.coverage = 95.0;
        z.tick(1.0);
        assert!(z.just_covered);
        assert!(z.is_covered());
    }

    #[test]
    fn tick_no_absorb_when_already_covered() {
        let mut z = z();
        z.coverage = 100.0;
        z.tick(1.0);
        assert!(!z.just_covered);
    }

    #[test]
    fn tick_no_absorb_when_rate_zero() {
        let mut z = Zileuton::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn tick_no_absorb_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn tick_clears_just_covered() {
        let mut z = Zileuton::new(100.0, 200.0);
        z.coverage = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_covered);
    }

    #[test]
    fn tick_clears_just_lapsed() {
        let mut z = z();
        z.coverage = 10.0;
        z.metabolize(10.0);
        z.tick(0.016);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.coverage - 9.0).abs() < 1e-3);
    }

    // --- is_covered / is_lapsed ---

    #[test]
    fn is_covered_false_when_disabled() {
        let mut z = z();
        z.coverage = 100.0;
        z.enabled = false;
        assert!(!z.is_covered());
    }

    #[test]
    fn is_lapsed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_lapsed());
    }

    // --- coverage_fraction / effective_suppression ---

    #[test]
    fn coverage_fraction_zero_when_lapsed() {
        assert_eq!(z().coverage_fraction(), 0.0);
    }

    #[test]
    fn coverage_fraction_half_at_midpoint() {
        let mut z = z();
        z.coverage = 50.0;
        assert!((z.coverage_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_suppression_zero_when_lapsed() {
        assert_eq!(z().effective_suppression(100.0), 0.0);
    }

    #[test]
    fn effective_suppression_scales_with_coverage() {
        let mut z = z();
        z.coverage = 75.0;
        assert!((z.effective_suppression(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_suppression_zero_when_disabled() {
        let mut z = z();
        z.coverage = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_suppression(100.0), 0.0);
    }
}
