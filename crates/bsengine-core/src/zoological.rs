use bevy_ecs::prelude::Component;

/// Zoological-collection accumulation tracker named after zoological,
/// the adjective meaning "of, relating to, or concerned with zoology
/// or zoological gardens." The zoological garden as an institution
/// has its immediate ancestor in the royal menageries of antiquity —
/// the lion pits of Mesopotamian kings, the aviaries of the Han
/// emperors, the Aztec zoo at Texcoco that astonished the Spanish
/// conquistadors — but the scientific zoological garden is a product
/// of Enlightenment taxonomy. The Schönbrunn in Vienna (1752) and
/// the Jardin des Plantes in Paris (1793) opened their collections
/// to the paying public while simultaneously operating as research
/// institutions, their keepers submitting specimens and observations
/// to the comparative anatomists who were building the first
/// systematic accounts of animal diversity. The Zoological Society
/// of London, founded in 1826, made the research mission explicit in
/// its name: the gardens at Regent's Park existed primarily to study
/// living animals, with public admission a secondary revenue stream.
/// Carl Hagenbeck's 1907 innovation of open moats and naturalistic
/// habitat design transformed the zoo from a cabinet of curiosities
/// into a conservation and education institution whose catalogue
/// could encompass thousands of species across hundreds of habitats.
/// `catalogue` builds via `acquire(amount)` and accumulates passively
/// at `accession_rate` per second in `tick(dt)` or depletes via
/// `deaccession(amount)`.
///
/// Models zoological-collection fill levels, specimen-catalogue
/// saturation bars, habitat-completion progress trackers, conservation-
/// programme accumulation gauges, wildlife-sanctuary admission meters,
/// species-inventory fill bars, biodiversity-index saturation
/// indicators, sanctuary-capacity accumulators, field-research-
/// specimen trackers, or any mechanic where patient institutional
/// effort slowly grows a collection of living animals until every
/// habitat is occupied and every species on the target list is
/// represented — and where disease, poaching, or funding collapse
/// drains the catalogue back to a handful of hardy generalists in
/// bare concrete pens.
///
/// `acquire(amount)` adds catalogue; fires `just_complete` when first
/// reaching `max_catalogue`. No-op when disabled.
///
/// `deaccession(amount)` reduces catalogue immediately; fires
/// `just_depleted` when reaching 0. No-op when disabled or already
/// depleted.
///
/// `tick(dt)` clears both flags, then increases catalogue by
/// `accession_rate * dt` (capped at `max_catalogue`). Fires
/// `just_complete` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_complete()` returns `catalogue >= max_catalogue && enabled`.
///
/// `is_depleted()` returns `catalogue == 0.0` (not gated by `enabled`).
///
/// `catalogue_fraction()` returns
/// `(catalogue / max_catalogue).clamp(0, 1)`.
///
/// `effective_specimen(scale)` returns `scale * catalogue_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — accessions at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoological {
    pub catalogue: f32,
    pub max_catalogue: f32,
    pub accession_rate: f32,
    pub just_complete: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Zoological {
    pub fn new(max_catalogue: f32, accession_rate: f32) -> Self {
        Self {
            catalogue: 0.0,
            max_catalogue: max_catalogue.max(0.1),
            accession_rate: accession_rate.max(0.0),
            just_complete: false,
            just_depleted: false,
            enabled: true,
        }
    }

    /// Add catalogue; fires `just_complete` when first reaching max.
    /// No-op when disabled.
    pub fn acquire(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.catalogue < self.max_catalogue;
        self.catalogue = (self.catalogue + amount).min(self.max_catalogue);
        if was_below && self.catalogue >= self.max_catalogue {
            self.just_complete = true;
        }
    }

    /// Reduce catalogue; fires `just_depleted` when reaching 0.
    /// No-op when disabled or already depleted.
    pub fn deaccession(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.catalogue <= 0.0 {
            return;
        }
        self.catalogue = (self.catalogue - amount).max(0.0);
        if self.catalogue <= 0.0 {
            self.just_depleted = true;
        }
    }

    /// Clear flags, then increase catalogue by `accession_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_complete = false;
        self.just_depleted = false;
        if self.enabled && self.accession_rate > 0.0 && self.catalogue < self.max_catalogue {
            let was_below = self.catalogue < self.max_catalogue;
            self.catalogue = (self.catalogue + self.accession_rate * dt).min(self.max_catalogue);
            if was_below && self.catalogue >= self.max_catalogue {
                self.just_complete = true;
            }
        }
    }

    /// `true` when catalogue is at maximum and component is enabled.
    pub fn is_complete(&self) -> bool {
        self.catalogue >= self.max_catalogue && self.enabled
    }

    /// `true` when catalogue is 0 (not gated by `enabled`).
    pub fn is_depleted(&self) -> bool {
        self.catalogue == 0.0
    }

    /// Fraction of maximum catalogue [0.0, 1.0].
    pub fn catalogue_fraction(&self) -> f32 {
        (self.catalogue / self.max_catalogue).clamp(0.0, 1.0)
    }

    /// Returns `scale * catalogue_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_specimen(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.catalogue_fraction()
    }
}

impl Default for Zoological {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoological {
        Zoological::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_depleted() {
        let z = z();
        assert_eq!(z.catalogue, 0.0);
        assert!(z.is_depleted());
        assert!(!z.is_complete());
    }

    #[test]
    fn new_clamps_max_catalogue() {
        let z = Zoological::new(-5.0, 1.5);
        assert!((z.max_catalogue - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_accession_rate() {
        let z = Zoological::new(100.0, -1.5);
        assert_eq!(z.accession_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoological::default();
        assert!((z.max_catalogue - 100.0).abs() < 1e-5);
        assert!((z.accession_rate - 1.5).abs() < 1e-5);
    }

    // --- acquire ---

    #[test]
    fn acquire_adds_catalogue() {
        let mut z = z();
        z.acquire(40.0);
        assert!((z.catalogue - 40.0).abs() < 1e-3);
    }

    #[test]
    fn acquire_clamps_at_max() {
        let mut z = z();
        z.acquire(200.0);
        assert!((z.catalogue - 100.0).abs() < 1e-3);
    }

    #[test]
    fn acquire_fires_just_complete_at_max() {
        let mut z = z();
        z.acquire(100.0);
        assert!(z.just_complete);
        assert!(z.is_complete());
    }

    #[test]
    fn acquire_no_just_complete_when_already_at_max() {
        let mut z = z();
        z.catalogue = 100.0;
        z.acquire(10.0);
        assert!(!z.just_complete);
    }

    #[test]
    fn acquire_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.acquire(50.0);
        assert_eq!(z.catalogue, 0.0);
    }

    #[test]
    fn acquire_no_op_when_amount_zero() {
        let mut z = z();
        z.acquire(0.0);
        assert_eq!(z.catalogue, 0.0);
    }

    // --- deaccession ---

    #[test]
    fn deaccession_reduces_catalogue() {
        let mut z = z();
        z.catalogue = 60.0;
        z.deaccession(20.0);
        assert!((z.catalogue - 40.0).abs() < 1e-3);
    }

    #[test]
    fn deaccession_clamps_at_zero() {
        let mut z = z();
        z.catalogue = 30.0;
        z.deaccession(200.0);
        assert_eq!(z.catalogue, 0.0);
    }

    #[test]
    fn deaccession_fires_just_depleted_at_zero() {
        let mut z = z();
        z.catalogue = 30.0;
        z.deaccession(30.0);
        assert!(z.just_depleted);
    }

    #[test]
    fn deaccession_no_op_when_already_depleted() {
        let mut z = z();
        z.deaccession(10.0);
        assert!(!z.just_depleted);
    }

    #[test]
    fn deaccession_no_op_when_disabled() {
        let mut z = z();
        z.catalogue = 50.0;
        z.enabled = false;
        z.deaccession(50.0);
        assert!((z.catalogue - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_accessions_catalogue() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.catalogue - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_complete_on_accession_to_max() {
        let mut z = Zoological::new(100.0, 200.0);
        z.catalogue = 95.0;
        z.tick(1.0);
        assert!(z.just_complete);
        assert!(z.is_complete());
    }

    #[test]
    fn tick_no_accession_when_already_complete() {
        let mut z = z();
        z.catalogue = 100.0;
        z.tick(1.0);
        assert!(!z.just_complete);
    }

    #[test]
    fn tick_no_accession_when_rate_zero() {
        let mut z = Zoological::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.catalogue, 0.0);
    }

    #[test]
    fn tick_no_accession_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.catalogue, 0.0);
    }

    #[test]
    fn tick_clears_just_complete() {
        let mut z = Zoological::new(100.0, 200.0);
        z.catalogue = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_complete);
    }

    #[test]
    fn tick_clears_just_depleted() {
        let mut z = z();
        z.catalogue = 10.0;
        z.deaccession(10.0);
        z.tick(0.016);
        assert!(!z.just_depleted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.catalogue - 9.0).abs() < 1e-3);
    }

    // --- is_complete / is_depleted ---

    #[test]
    fn is_complete_false_when_disabled() {
        let mut z = z();
        z.catalogue = 100.0;
        z.enabled = false;
        assert!(!z.is_complete());
    }

    #[test]
    fn is_depleted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_depleted());
    }

    // --- catalogue_fraction / effective_specimen ---

    #[test]
    fn catalogue_fraction_zero_when_depleted() {
        assert_eq!(z().catalogue_fraction(), 0.0);
    }

    #[test]
    fn catalogue_fraction_half_at_midpoint() {
        let mut z = z();
        z.catalogue = 50.0;
        assert!((z.catalogue_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_specimen_zero_when_depleted() {
        assert_eq!(z().effective_specimen(100.0), 0.0);
    }

    #[test]
    fn effective_specimen_scales_with_catalogue() {
        let mut z = z();
        z.catalogue = 75.0;
        assert!((z.effective_specimen(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_specimen_zero_when_disabled() {
        let mut z = z();
        z.catalogue = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_specimen(100.0), 0.0);
    }
}
