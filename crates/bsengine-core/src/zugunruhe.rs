use bevy_ecs::prelude::Component;

/// Seasonal-migratory-drive tracker named after zugunruhe (from German
/// Zug "migration" + Unruhe "unrest, restlessness"), the restless
/// migratory excitement that seizes birds and other long-distance
/// migrants as the breeding or wintering season draws to a close.
/// The phenomenon was first described in cage birds in the eighteenth
/// century: migratory species that should be flying south begin instead
/// to flutter against the cage bars at night, orienting themselves
/// approximately in the compass direction of their migration route even
/// without celestial or magnetic landmarks. The strength and duration of
/// zugunruhe correlates precisely with the distance the bird would fly
/// in the wild — a garden warbler held in a lab in Germany shows three
/// weeks of nocturnal restlessness, just enough to carry it to sub-
/// Saharan Africa. The timing is controlled by a circannual clock
/// entrained by photoperiod: as day length shortens in autumn the
/// hypothalamus signals the release of corticosterone, which triggers
/// hyperphagia, fuel deposition, organ regression, and finally the
/// restlessness itself. `restlessness` builds via `agitate(amount)` and
/// accumulates passively at `migrate_rate` per second in `tick(dt)` or
/// dissipates via `settle(amount)`.
///
/// Models seasonal-migration-drive fill levels, animal-restlessness
/// saturation bars, circannual-clock-discharge gauges, photoperiod-
/// driven compulsion trackers, long-distance-flight-urge accumulation
/// meters, cage-bird bar-fluttering saturation indicators, nocturnal-
/// migratory-activity fill levels, corticosterone-surge accumulation
/// bars, hyperphagia-trigger proximity meters, or any mechanic where
/// a slowly tightening seasonal clock charges an invisible drive until
/// the creature's entire physiology pivots overnight toward a
/// destination it has never seen but can locate to within a few degrees
/// using magnetic inclination, stellar rotation, and the smell of the
/// wind.
///
/// `agitate(amount)` adds restlessness; fires `just_impelled` when
/// first reaching `max_restlessness`. No-op when disabled.
///
/// `settle(amount)` reduces restlessness immediately; fires
/// `just_settled` when reaching 0. No-op when disabled or already
/// settled.
///
/// `tick(dt)` clears both flags, then increases restlessness by
/// `migrate_rate * dt` (capped at `max_restlessness`). Fires
/// `just_impelled` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_impelled()` returns `restlessness >= max_restlessness && enabled`.
///
/// `is_settled()` returns `restlessness == 0.0` (not gated by `enabled`).
///
/// `restlessness_fraction()` returns
/// `(restlessness / max_restlessness).clamp(0, 1)`.
///
/// `effective_drive(scale)` returns `scale * restlessness_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — migrates at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zugunruhe {
    pub restlessness: f32,
    pub max_restlessness: f32,
    pub migrate_rate: f32,
    pub just_impelled: bool,
    pub just_settled: bool,
    pub enabled: bool,
}

impl Zugunruhe {
    pub fn new(max_restlessness: f32, migrate_rate: f32) -> Self {
        Self {
            restlessness: 0.0,
            max_restlessness: max_restlessness.max(0.1),
            migrate_rate: migrate_rate.max(0.0),
            just_impelled: false,
            just_settled: false,
            enabled: true,
        }
    }

    /// Add restlessness; fires `just_impelled` when first reaching max.
    /// No-op when disabled.
    pub fn agitate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.restlessness < self.max_restlessness;
        self.restlessness = (self.restlessness + amount).min(self.max_restlessness);
        if was_below && self.restlessness >= self.max_restlessness {
            self.just_impelled = true;
        }
    }

    /// Reduce restlessness; fires `just_settled` when reaching 0.
    /// No-op when disabled or already settled.
    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.restlessness <= 0.0 {
            return;
        }
        self.restlessness = (self.restlessness - amount).max(0.0);
        if self.restlessness <= 0.0 {
            self.just_settled = true;
        }
    }

    /// Clear flags, then increase restlessness by `migrate_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_impelled = false;
        self.just_settled = false;
        if self.enabled && self.migrate_rate > 0.0 && self.restlessness < self.max_restlessness {
            let was_below = self.restlessness < self.max_restlessness;
            self.restlessness =
                (self.restlessness + self.migrate_rate * dt).min(self.max_restlessness);
            if was_below && self.restlessness >= self.max_restlessness {
                self.just_impelled = true;
            }
        }
    }

    /// `true` when restlessness is at maximum and component is enabled.
    pub fn is_impelled(&self) -> bool {
        self.restlessness >= self.max_restlessness && self.enabled
    }

    /// `true` when restlessness is 0 (not gated by `enabled`).
    pub fn is_settled(&self) -> bool {
        self.restlessness == 0.0
    }

    /// Fraction of maximum restlessness [0.0, 1.0].
    pub fn restlessness_fraction(&self) -> f32 {
        (self.restlessness / self.max_restlessness).clamp(0.0, 1.0)
    }

    /// Returns `scale * restlessness_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_drive(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.restlessness_fraction()
    }
}

impl Default for Zugunruhe {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zugunruhe {
        Zugunruhe::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_settled() {
        let z = z();
        assert_eq!(z.restlessness, 0.0);
        assert!(z.is_settled());
        assert!(!z.is_impelled());
    }

    #[test]
    fn new_clamps_max_restlessness() {
        let z = Zugunruhe::new(-5.0, 1.5);
        assert!((z.max_restlessness - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_migrate_rate() {
        let z = Zugunruhe::new(100.0, -1.5);
        assert_eq!(z.migrate_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zugunruhe::default();
        assert!((z.max_restlessness - 100.0).abs() < 1e-5);
        assert!((z.migrate_rate - 1.5).abs() < 1e-5);
    }

    // --- agitate ---

    #[test]
    fn agitate_adds_restlessness() {
        let mut z = z();
        z.agitate(40.0);
        assert!((z.restlessness - 40.0).abs() < 1e-3);
    }

    #[test]
    fn agitate_clamps_at_max() {
        let mut z = z();
        z.agitate(200.0);
        assert!((z.restlessness - 100.0).abs() < 1e-3);
    }

    #[test]
    fn agitate_fires_just_impelled_at_max() {
        let mut z = z();
        z.agitate(100.0);
        assert!(z.just_impelled);
        assert!(z.is_impelled());
    }

    #[test]
    fn agitate_no_just_impelled_when_already_at_max() {
        let mut z = z();
        z.restlessness = 100.0;
        z.agitate(10.0);
        assert!(!z.just_impelled);
    }

    #[test]
    fn agitate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.agitate(50.0);
        assert_eq!(z.restlessness, 0.0);
    }

    #[test]
    fn agitate_no_op_when_amount_zero() {
        let mut z = z();
        z.agitate(0.0);
        assert_eq!(z.restlessness, 0.0);
    }

    // --- settle ---

    #[test]
    fn settle_reduces_restlessness() {
        let mut z = z();
        z.restlessness = 60.0;
        z.settle(20.0);
        assert!((z.restlessness - 40.0).abs() < 1e-3);
    }

    #[test]
    fn settle_clamps_at_zero() {
        let mut z = z();
        z.restlessness = 30.0;
        z.settle(200.0);
        assert_eq!(z.restlessness, 0.0);
    }

    #[test]
    fn settle_fires_just_settled_at_zero() {
        let mut z = z();
        z.restlessness = 30.0;
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
        z.restlessness = 50.0;
        z.enabled = false;
        z.settle(50.0);
        assert!((z.restlessness - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_migrates_restlessness() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.restlessness - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_impelled_on_migrate_to_max() {
        let mut z = Zugunruhe::new(100.0, 200.0);
        z.restlessness = 95.0;
        z.tick(1.0);
        assert!(z.just_impelled);
        assert!(z.is_impelled());
    }

    #[test]
    fn tick_no_migrate_when_already_impelled() {
        let mut z = z();
        z.restlessness = 100.0;
        z.tick(1.0);
        assert!(!z.just_impelled);
    }

    #[test]
    fn tick_no_migrate_when_rate_zero() {
        let mut z = Zugunruhe::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.restlessness, 0.0);
    }

    #[test]
    fn tick_no_migrate_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.restlessness, 0.0);
    }

    #[test]
    fn tick_clears_just_impelled() {
        let mut z = Zugunruhe::new(100.0, 200.0);
        z.restlessness = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_impelled);
    }

    #[test]
    fn tick_clears_just_settled() {
        let mut z = z();
        z.restlessness = 10.0;
        z.settle(10.0);
        z.tick(0.016);
        assert!(!z.just_settled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.restlessness - 9.0).abs() < 1e-3);
    }

    // --- is_impelled / is_settled ---

    #[test]
    fn is_impelled_false_when_disabled() {
        let mut z = z();
        z.restlessness = 100.0;
        z.enabled = false;
        assert!(!z.is_impelled());
    }

    #[test]
    fn is_settled_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_settled());
    }

    // --- restlessness_fraction / effective_drive ---

    #[test]
    fn restlessness_fraction_zero_when_settled() {
        assert_eq!(z().restlessness_fraction(), 0.0);
    }

    #[test]
    fn restlessness_fraction_half_at_midpoint() {
        let mut z = z();
        z.restlessness = 50.0;
        assert!((z.restlessness_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_drive_zero_when_settled() {
        assert_eq!(z().effective_drive(100.0), 0.0);
    }

    #[test]
    fn effective_drive_scales_with_restlessness() {
        let mut z = z();
        z.restlessness = 75.0;
        assert!((z.effective_drive(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_drive_zero_when_disabled() {
        let mut z = z();
        z.restlessness = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_drive(100.0), 0.0);
    }
}
