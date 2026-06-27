use bevy_ecs::prelude::Component;

/// Field-specimen cataloging tracker. `specimens` builds via
/// `observe(amount)` and accumulates passively at `catalog_rate`
/// per second in `tick(dt)` or is reduced immediately via
/// `extirpate(amount)`.
///
/// Models wildlife-survey completeness fill levels, species-catalog
/// saturation bars, field-study data accumulation trackers,
/// taxonomic-classification progress meters, biodiversity-survey
/// completion gauges, specimen-collection fill levels, natural-
/// history-museum acquisition bars, herpetology-field-trip data
/// accumulators, entomology-collection saturation trackers, or
/// any mechanic where patient field observation slowly fills a
/// naturalist's notebook with meticulously measured specimens until
/// every species in the territory has been caught, cataloged, and
/// pinned in neat rows — only for a habitat disturbance to clear
/// the land and push the whole catalog back toward the heartbreaking
/// blank of an empty field season.
///
/// `observe(amount)` adds specimens; fires `just_cataloged` when
/// first reaching `max_specimens`. No-op when disabled.
///
/// `extirpate(amount)` reduces specimens immediately; fires
/// `just_extinct` when reaching 0. No-op when disabled or already
/// extinct.
///
/// `tick(dt)` clears both flags, then increases specimens by
/// `catalog_rate * dt` (capped at `max_specimens`). Fires
/// `just_cataloged` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_cataloged()` returns `specimens >= max_specimens && enabled`.
///
/// `is_extinct()` returns `specimens == 0.0` (not gated by `enabled`).
///
/// `specimen_fraction()` returns `(specimens / max_specimens).clamp(0, 1)`.
///
/// `effective_survey(scale)` returns `scale * specimen_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — catalogs at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoology {
    pub specimens: f32,
    pub max_specimens: f32,
    pub catalog_rate: f32,
    pub just_cataloged: bool,
    pub just_extinct: bool,
    pub enabled: bool,
}

impl Zoology {
    pub fn new(max_specimens: f32, catalog_rate: f32) -> Self {
        Self {
            specimens: 0.0,
            max_specimens: max_specimens.max(0.1),
            catalog_rate: catalog_rate.max(0.0),
            just_cataloged: false,
            just_extinct: false,
            enabled: true,
        }
    }

    /// Add specimens; fires `just_cataloged` when first reaching max.
    /// No-op when disabled.
    pub fn observe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.specimens < self.max_specimens;
        self.specimens = (self.specimens + amount).min(self.max_specimens);
        if was_below && self.specimens >= self.max_specimens {
            self.just_cataloged = true;
        }
    }

    /// Reduce specimens; fires `just_extinct` when reaching 0.
    /// No-op when disabled or already extinct.
    pub fn extirpate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.specimens <= 0.0 {
            return;
        }
        self.specimens = (self.specimens - amount).max(0.0);
        if self.specimens <= 0.0 {
            self.just_extinct = true;
        }
    }

    /// Clear flags, then increase specimens by `catalog_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_cataloged = false;
        self.just_extinct = false;
        if self.enabled && self.catalog_rate > 0.0 && self.specimens < self.max_specimens {
            let was_below = self.specimens < self.max_specimens;
            self.specimens = (self.specimens + self.catalog_rate * dt).min(self.max_specimens);
            if was_below && self.specimens >= self.max_specimens {
                self.just_cataloged = true;
            }
        }
    }

    /// `true` when specimens is at maximum and component is enabled.
    pub fn is_cataloged(&self) -> bool {
        self.specimens >= self.max_specimens && self.enabled
    }

    /// `true` when specimens is 0 (not gated by `enabled`).
    pub fn is_extinct(&self) -> bool {
        self.specimens == 0.0
    }

    /// Fraction of maximum specimens [0.0, 1.0].
    pub fn specimen_fraction(&self) -> f32 {
        (self.specimens / self.max_specimens).clamp(0.0, 1.0)
    }

    /// Returns `scale * specimen_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_survey(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.specimen_fraction()
    }
}

impl Default for Zoology {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoology {
        Zoology::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_extinct() {
        let z = z();
        assert_eq!(z.specimens, 0.0);
        assert!(z.is_extinct());
        assert!(!z.is_cataloged());
    }

    #[test]
    fn new_clamps_max_specimens() {
        let z = Zoology::new(-5.0, 1.5);
        assert!((z.max_specimens - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_catalog_rate() {
        let z = Zoology::new(100.0, -1.5);
        assert_eq!(z.catalog_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoology::default();
        assert!((z.max_specimens - 100.0).abs() < 1e-5);
        assert!((z.catalog_rate - 1.5).abs() < 1e-5);
    }

    // --- observe ---

    #[test]
    fn observe_adds_specimens() {
        let mut z = z();
        z.observe(40.0);
        assert!((z.specimens - 40.0).abs() < 1e-3);
    }

    #[test]
    fn observe_clamps_at_max() {
        let mut z = z();
        z.observe(200.0);
        assert!((z.specimens - 100.0).abs() < 1e-3);
    }

    #[test]
    fn observe_fires_just_cataloged_at_max() {
        let mut z = z();
        z.observe(100.0);
        assert!(z.just_cataloged);
        assert!(z.is_cataloged());
    }

    #[test]
    fn observe_no_just_cataloged_when_already_at_max() {
        let mut z = z();
        z.specimens = 100.0;
        z.observe(10.0);
        assert!(!z.just_cataloged);
    }

    #[test]
    fn observe_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.observe(50.0);
        assert_eq!(z.specimens, 0.0);
    }

    #[test]
    fn observe_no_op_when_amount_zero() {
        let mut z = z();
        z.observe(0.0);
        assert_eq!(z.specimens, 0.0);
    }

    // --- extirpate ---

    #[test]
    fn extirpate_reduces_specimens() {
        let mut z = z();
        z.specimens = 60.0;
        z.extirpate(20.0);
        assert!((z.specimens - 40.0).abs() < 1e-3);
    }

    #[test]
    fn extirpate_clamps_at_zero() {
        let mut z = z();
        z.specimens = 30.0;
        z.extirpate(200.0);
        assert_eq!(z.specimens, 0.0);
    }

    #[test]
    fn extirpate_fires_just_extinct_at_zero() {
        let mut z = z();
        z.specimens = 30.0;
        z.extirpate(30.0);
        assert!(z.just_extinct);
    }

    #[test]
    fn extirpate_no_op_when_already_extinct() {
        let mut z = z();
        z.extirpate(10.0);
        assert!(!z.just_extinct);
    }

    #[test]
    fn extirpate_no_op_when_disabled() {
        let mut z = z();
        z.specimens = 50.0;
        z.enabled = false;
        z.extirpate(50.0);
        assert!((z.specimens - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_catalogs_specimens() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.specimens - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_cataloged_on_catalog_to_max() {
        let mut z = Zoology::new(100.0, 200.0);
        z.specimens = 95.0;
        z.tick(1.0);
        assert!(z.just_cataloged);
        assert!(z.is_cataloged());
    }

    #[test]
    fn tick_no_catalog_when_already_cataloged() {
        let mut z = z();
        z.specimens = 100.0;
        z.tick(1.0);
        assert!(!z.just_cataloged);
    }

    #[test]
    fn tick_no_catalog_when_rate_zero() {
        let mut z = Zoology::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.specimens, 0.0);
    }

    #[test]
    fn tick_no_catalog_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.specimens, 0.0);
    }

    #[test]
    fn tick_clears_just_cataloged() {
        let mut z = Zoology::new(100.0, 200.0);
        z.specimens = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_cataloged);
    }

    #[test]
    fn tick_clears_just_extinct() {
        let mut z = z();
        z.specimens = 10.0;
        z.extirpate(10.0);
        z.tick(0.016);
        assert!(!z.just_extinct);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.specimens - 9.0).abs() < 1e-3);
    }

    // --- is_cataloged / is_extinct ---

    #[test]
    fn is_cataloged_false_when_disabled() {
        let mut z = z();
        z.specimens = 100.0;
        z.enabled = false;
        assert!(!z.is_cataloged());
    }

    #[test]
    fn is_extinct_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_extinct());
    }

    // --- specimen_fraction / effective_survey ---

    #[test]
    fn specimen_fraction_zero_when_extinct() {
        assert_eq!(z().specimen_fraction(), 0.0);
    }

    #[test]
    fn specimen_fraction_half_at_midpoint() {
        let mut z = z();
        z.specimens = 50.0;
        assert!((z.specimen_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_survey_zero_when_extinct() {
        assert_eq!(z().effective_survey(100.0), 0.0);
    }

    #[test]
    fn effective_survey_scales_with_specimens() {
        let mut z = z();
        z.specimens = 75.0;
        assert!((z.effective_survey(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_survey_zero_when_disabled() {
        let mut z = z();
        z.specimens = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_survey(100.0), 0.0);
    }
}
