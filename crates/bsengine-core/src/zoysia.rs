use bevy_ecs::prelude::Component;

/// Turf-coverage tracker. `coverage` builds via `spread(amount)` and
/// grows passively at `growth_rate` per second in `tick(dt)` or is
/// cut back immediately via `mow(amount)`.
///
/// Models warm-season-grass lawn coverage meters, weed-invasion-front
/// advance bars, sports-turf recovery accumulators, golf-fairway
/// density gauges, drought-resistant sod establishment trackers,
/// groundcover creep progress bars, ornamental-lawn fill-level
/// indicators, turf-management recovery meters, suburban-yard
/// colonisation trackers, or any mechanic where a tenacious
/// low-growing grass relentlessly spreads its runners until every
/// patch of bare soil disappears beneath a smooth, mowable mat.
///
/// `spread(amount)` adds coverage; fires `just_covered` when first
/// reaching `max_coverage`. No-op when disabled.
///
/// `mow(amount)` reduces coverage immediately; fires `just_bare` when
/// reaching 0. No-op when disabled or already bare.
///
/// `tick(dt)` clears both flags, then increases coverage by
/// `growth_rate * dt` (capped at `max_coverage`). Fires `just_covered`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_covered()` returns `coverage >= max_coverage && enabled`.
///
/// `is_bare()` returns `coverage == 0.0` (not gated by `enabled`).
///
/// `coverage_fraction()` returns `(coverage / max_coverage).clamp(0, 1)`.
///
/// `effective_density(scale)` returns `scale * coverage_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — spreads at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoysia {
    pub coverage: f32,
    pub max_coverage: f32,
    pub growth_rate: f32,
    pub just_covered: bool,
    pub just_bare: bool,
    pub enabled: bool,
}

impl Zoysia {
    pub fn new(max_coverage: f32, growth_rate: f32) -> Self {
        Self {
            coverage: 0.0,
            max_coverage: max_coverage.max(0.1),
            growth_rate: growth_rate.max(0.0),
            just_covered: false,
            just_bare: false,
            enabled: true,
        }
    }

    /// Add coverage; fires `just_covered` when first reaching max.
    /// No-op when disabled.
    pub fn spread(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.coverage < self.max_coverage;
        self.coverage = (self.coverage + amount).min(self.max_coverage);
        if was_below && self.coverage >= self.max_coverage {
            self.just_covered = true;
        }
    }

    /// Reduce coverage; fires `just_bare` when reaching 0.
    /// No-op when disabled or already bare.
    pub fn mow(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.coverage <= 0.0 {
            return;
        }
        self.coverage = (self.coverage - amount).max(0.0);
        if self.coverage <= 0.0 {
            self.just_bare = true;
        }
    }

    /// Clear flags, then increase coverage by `growth_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_covered = false;
        self.just_bare = false;
        if self.enabled && self.growth_rate > 0.0 && self.coverage < self.max_coverage {
            let was_below = self.coverage < self.max_coverage;
            self.coverage = (self.coverage + self.growth_rate * dt).min(self.max_coverage);
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
    pub fn is_bare(&self) -> bool {
        self.coverage == 0.0
    }

    /// Fraction of maximum coverage [0.0, 1.0].
    pub fn coverage_fraction(&self) -> f32 {
        (self.coverage / self.max_coverage).clamp(0.0, 1.0)
    }

    /// Returns `scale * coverage_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_density(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.coverage_fraction()
    }
}

impl Default for Zoysia {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoysia {
        Zoysia::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_bare() {
        let z = z();
        assert_eq!(z.coverage, 0.0);
        assert!(z.is_bare());
        assert!(!z.is_covered());
    }

    #[test]
    fn new_clamps_max_coverage() {
        let z = Zoysia::new(-5.0, 1.5);
        assert!((z.max_coverage - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_growth_rate() {
        let z = Zoysia::new(100.0, -3.0);
        assert_eq!(z.growth_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoysia::default();
        assert!((z.max_coverage - 100.0).abs() < 1e-5);
        assert!((z.growth_rate - 1.5).abs() < 1e-5);
    }

    // --- spread ---

    #[test]
    fn spread_adds_coverage() {
        let mut z = z();
        z.spread(40.0);
        assert!((z.coverage - 40.0).abs() < 1e-3);
    }

    #[test]
    fn spread_clamps_at_max() {
        let mut z = z();
        z.spread(200.0);
        assert!((z.coverage - 100.0).abs() < 1e-3);
    }

    #[test]
    fn spread_fires_just_covered_at_max() {
        let mut z = z();
        z.spread(100.0);
        assert!(z.just_covered);
        assert!(z.is_covered());
    }

    #[test]
    fn spread_no_just_covered_when_already_at_max() {
        let mut z = z();
        z.coverage = 100.0;
        z.spread(10.0);
        assert!(!z.just_covered);
    }

    #[test]
    fn spread_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.spread(50.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn spread_no_op_when_amount_zero() {
        let mut z = z();
        z.spread(0.0);
        assert_eq!(z.coverage, 0.0);
    }

    // --- mow ---

    #[test]
    fn mow_reduces_coverage() {
        let mut z = z();
        z.coverage = 60.0;
        z.mow(20.0);
        assert!((z.coverage - 40.0).abs() < 1e-3);
    }

    #[test]
    fn mow_clamps_at_zero() {
        let mut z = z();
        z.coverage = 30.0;
        z.mow(200.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn mow_fires_just_bare_at_zero() {
        let mut z = z();
        z.coverage = 30.0;
        z.mow(30.0);
        assert!(z.just_bare);
    }

    #[test]
    fn mow_no_op_when_already_bare() {
        let mut z = z();
        z.mow(10.0);
        assert!(!z.just_bare);
    }

    #[test]
    fn mow_no_op_when_disabled() {
        let mut z = z();
        z.coverage = 50.0;
        z.enabled = false;
        z.mow(50.0);
        assert!((z.coverage - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_grows_coverage() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.coverage - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_covered_on_growth_to_max() {
        let mut z = Zoysia::new(100.0, 200.0);
        z.coverage = 95.0;
        z.tick(1.0);
        assert!(z.just_covered);
        assert!(z.is_covered());
    }

    #[test]
    fn tick_no_growth_when_already_covered() {
        let mut z = z();
        z.coverage = 100.0;
        z.tick(1.0);
        assert!(!z.just_covered);
    }

    #[test]
    fn tick_no_growth_when_rate_zero() {
        let mut z = Zoysia::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn tick_no_growth_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn tick_clears_just_covered() {
        let mut z = Zoysia::new(100.0, 200.0);
        z.coverage = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_covered);
    }

    #[test]
    fn tick_clears_just_bare() {
        let mut z = z();
        z.coverage = 10.0;
        z.mow(10.0);
        z.tick(0.016);
        assert!(!z.just_bare);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(10.0); // 1.5*10 = 15
        assert!((z.coverage - 15.0).abs() < 1e-3);
    }

    // --- is_covered / is_bare ---

    #[test]
    fn is_covered_false_when_disabled() {
        let mut z = z();
        z.coverage = 100.0;
        z.enabled = false;
        assert!(!z.is_covered());
    }

    #[test]
    fn is_bare_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_bare());
    }

    // --- coverage_fraction / effective_density ---

    #[test]
    fn coverage_fraction_zero_when_bare() {
        assert_eq!(z().coverage_fraction(), 0.0);
    }

    #[test]
    fn coverage_fraction_half_at_midpoint() {
        let mut z = z();
        z.coverage = 50.0;
        assert!((z.coverage_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_density_zero_when_bare() {
        assert_eq!(z().effective_density(100.0), 0.0);
    }

    #[test]
    fn effective_density_scales_with_coverage() {
        let mut z = z();
        z.coverage = 70.0;
        assert!((z.effective_density(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_density_zero_when_disabled() {
        let mut z = z();
        z.coverage = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_density(100.0), 0.0);
    }
}
