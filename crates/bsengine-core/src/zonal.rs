use bevy_ecs::prelude::Component;

/// Spatial-zone coverage accumulation tracker named after zonal, the
/// adjective meaning "of, relating to, or having the form of a zone."
/// Zones are among the most fundamental organisational concepts in
/// physical and human geography: the Earth's climate belts are zonal —
/// arranged in latitudinal bands from tropical to polar — because solar
/// energy input varies systematically with latitude and drives the same
/// pressure and wind patterns at equivalent latitudes on every
/// continent. Urban planners apply zonal logic to land use,
/// partitioning cities into residential, commercial, industrial, and
/// mixed-use zones; ecologists use biomes and life zones as analytical
/// units; military planners define kill zones, exclusion zones, and
/// buffer zones; agricultural scientists map soil-capability zones and
/// irrigation zones. The concept scales from the molecular — reaction
/// zones in a flame, depletion zones in a semiconductor junction,
/// Fresnel zones in an optical aperture — to the planetary: Venus's
/// atmospheric circulation cells, Jupiter's banded zonal winds, and
/// the heliosphere's termination shock are all zonal phenomena in the
/// sense that they reflect the underlying symmetry of a rotating
/// sphere. `coverage` builds via `demarcate(amount)` and accumulates
/// passively at `zone_rate` per second in `tick(dt)` or dissolves via
/// `dissolve(amount)`.
///
/// Models spatial-zone fill levels, territorial-partition saturation
/// bars, land-use-zoning completion trackers, exclusion-zone
/// establishment gauges, climate-belt coverage meters, urban-zone
/// demarcation fill bars, agricultural-zone mapping progress
/// indicators, military-zone establishment accumulators, biome-
/// coverage saturation bars, or any mechanic where patient spatial
/// organisation slowly divides a territory into clearly delineated
/// zones until every parcel of ground belongs to a defined partition —
/// and where conflict, flood, or administrative collapse dissolves
/// those carefully drawn boundaries back to undifferentiated
/// wilderness.
///
/// `demarcate(amount)` adds coverage; fires `just_zoned` when first
/// reaching `max_coverage`. No-op when disabled.
///
/// `dissolve(amount)` reduces coverage immediately; fires `just_unzoned`
/// when reaching 0. No-op when disabled or already unzoned.
///
/// `tick(dt)` clears both flags, then increases coverage by
/// `zone_rate * dt` (capped at `max_coverage`). Fires `just_zoned`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_zoned()` returns `coverage >= max_coverage && enabled`.
///
/// `is_unzoned()` returns `coverage == 0.0` (not gated by `enabled`).
///
/// `coverage_fraction()` returns
/// `(coverage / max_coverage).clamp(0, 1)`.
///
/// `effective_partition(scale)` returns `scale * coverage_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — zones at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zonal {
    pub coverage: f32,
    pub max_coverage: f32,
    pub zone_rate: f32,
    pub just_zoned: bool,
    pub just_unzoned: bool,
    pub enabled: bool,
}

impl Zonal {
    pub fn new(max_coverage: f32, zone_rate: f32) -> Self {
        Self {
            coverage: 0.0,
            max_coverage: max_coverage.max(0.1),
            zone_rate: zone_rate.max(0.0),
            just_zoned: false,
            just_unzoned: false,
            enabled: true,
        }
    }

    /// Add coverage; fires `just_zoned` when first reaching max.
    /// No-op when disabled.
    pub fn demarcate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.coverage < self.max_coverage;
        self.coverage = (self.coverage + amount).min(self.max_coverage);
        if was_below && self.coverage >= self.max_coverage {
            self.just_zoned = true;
        }
    }

    /// Reduce coverage; fires `just_unzoned` when reaching 0.
    /// No-op when disabled or already unzoned.
    pub fn dissolve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.coverage <= 0.0 {
            return;
        }
        self.coverage = (self.coverage - amount).max(0.0);
        if self.coverage <= 0.0 {
            self.just_unzoned = true;
        }
    }

    /// Clear flags, then increase coverage by `zone_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_zoned = false;
        self.just_unzoned = false;
        if self.enabled && self.zone_rate > 0.0 && self.coverage < self.max_coverage {
            let was_below = self.coverage < self.max_coverage;
            self.coverage = (self.coverage + self.zone_rate * dt).min(self.max_coverage);
            if was_below && self.coverage >= self.max_coverage {
                self.just_zoned = true;
            }
        }
    }

    /// `true` when coverage is at maximum and component is enabled.
    pub fn is_zoned(&self) -> bool {
        self.coverage >= self.max_coverage && self.enabled
    }

    /// `true` when coverage is 0 (not gated by `enabled`).
    pub fn is_unzoned(&self) -> bool {
        self.coverage == 0.0
    }

    /// Fraction of maximum coverage [0.0, 1.0].
    pub fn coverage_fraction(&self) -> f32 {
        (self.coverage / self.max_coverage).clamp(0.0, 1.0)
    }

    /// Returns `scale * coverage_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_partition(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.coverage_fraction()
    }
}

impl Default for Zonal {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zonal {
        Zonal::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_unzoned() {
        let z = z();
        assert_eq!(z.coverage, 0.0);
        assert!(z.is_unzoned());
        assert!(!z.is_zoned());
    }

    #[test]
    fn new_clamps_max_coverage() {
        let z = Zonal::new(-5.0, 1.5);
        assert!((z.max_coverage - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_zone_rate() {
        let z = Zonal::new(100.0, -1.5);
        assert_eq!(z.zone_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zonal::default();
        assert!((z.max_coverage - 100.0).abs() < 1e-5);
        assert!((z.zone_rate - 1.5).abs() < 1e-5);
    }

    // --- demarcate ---

    #[test]
    fn demarcate_adds_coverage() {
        let mut z = z();
        z.demarcate(40.0);
        assert!((z.coverage - 40.0).abs() < 1e-3);
    }

    #[test]
    fn demarcate_clamps_at_max() {
        let mut z = z();
        z.demarcate(200.0);
        assert!((z.coverage - 100.0).abs() < 1e-3);
    }

    #[test]
    fn demarcate_fires_just_zoned_at_max() {
        let mut z = z();
        z.demarcate(100.0);
        assert!(z.just_zoned);
        assert!(z.is_zoned());
    }

    #[test]
    fn demarcate_no_just_zoned_when_already_at_max() {
        let mut z = z();
        z.coverage = 100.0;
        z.demarcate(10.0);
        assert!(!z.just_zoned);
    }

    #[test]
    fn demarcate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.demarcate(50.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn demarcate_no_op_when_amount_zero() {
        let mut z = z();
        z.demarcate(0.0);
        assert_eq!(z.coverage, 0.0);
    }

    // --- dissolve ---

    #[test]
    fn dissolve_reduces_coverage() {
        let mut z = z();
        z.coverage = 60.0;
        z.dissolve(20.0);
        assert!((z.coverage - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dissolve_clamps_at_zero() {
        let mut z = z();
        z.coverage = 30.0;
        z.dissolve(200.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn dissolve_fires_just_unzoned_at_zero() {
        let mut z = z();
        z.coverage = 30.0;
        z.dissolve(30.0);
        assert!(z.just_unzoned);
    }

    #[test]
    fn dissolve_no_op_when_already_unzoned() {
        let mut z = z();
        z.dissolve(10.0);
        assert!(!z.just_unzoned);
    }

    #[test]
    fn dissolve_no_op_when_disabled() {
        let mut z = z();
        z.coverage = 50.0;
        z.enabled = false;
        z.dissolve(50.0);
        assert!((z.coverage - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_zones_coverage() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.coverage - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_zoned_on_zone_to_max() {
        let mut z = Zonal::new(100.0, 200.0);
        z.coverage = 95.0;
        z.tick(1.0);
        assert!(z.just_zoned);
        assert!(z.is_zoned());
    }

    #[test]
    fn tick_no_zone_when_already_zoned() {
        let mut z = z();
        z.coverage = 100.0;
        z.tick(1.0);
        assert!(!z.just_zoned);
    }

    #[test]
    fn tick_no_zone_when_rate_zero() {
        let mut z = Zonal::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn tick_no_zone_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.coverage, 0.0);
    }

    #[test]
    fn tick_clears_just_zoned() {
        let mut z = Zonal::new(100.0, 200.0);
        z.coverage = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_zoned);
    }

    #[test]
    fn tick_clears_just_unzoned() {
        let mut z = z();
        z.coverage = 10.0;
        z.dissolve(10.0);
        z.tick(0.016);
        assert!(!z.just_unzoned);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.coverage - 9.0).abs() < 1e-3);
    }

    // --- is_zoned / is_unzoned ---

    #[test]
    fn is_zoned_false_when_disabled() {
        let mut z = z();
        z.coverage = 100.0;
        z.enabled = false;
        assert!(!z.is_zoned());
    }

    #[test]
    fn is_unzoned_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_unzoned());
    }

    // --- coverage_fraction / effective_partition ---

    #[test]
    fn coverage_fraction_zero_when_unzoned() {
        assert_eq!(z().coverage_fraction(), 0.0);
    }

    #[test]
    fn coverage_fraction_half_at_midpoint() {
        let mut z = z();
        z.coverage = 50.0;
        assert!((z.coverage_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_partition_zero_when_unzoned() {
        assert_eq!(z().effective_partition(100.0), 0.0);
    }

    #[test]
    fn effective_partition_scales_with_coverage() {
        let mut z = z();
        z.coverage = 75.0;
        assert!((z.effective_partition(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_partition_zero_when_disabled() {
        let mut z = z();
        z.coverage = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_partition(100.0), 0.0);
    }
}
