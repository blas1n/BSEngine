use bevy_ecs::prelude::Component;

/// Faunal-range accumulation tracker named after zoogeography, the
/// branch of geography that studies the distribution of animal species
/// across the surface of the Earth — mapping which creatures live
/// where, why they live there, and how they arrived. The discipline
/// fuses ecology, evolutionary biology, plate tectonics, and
/// palaeoclimatology: a zoogeographer who finds a marsupial in South
/// America and another in Australia asks not only why they are
/// similar but how the geological and climatic history of Gondwana
/// could have carried a common ancestor to both continents before
/// continental drift separated them. Alfred Russel Wallace codified
/// the field in his 1876 work "The Geographical Distribution of
/// Animals," dividing the world into six zoogeographical regions —
/// Nearctic, Palaearctic, Neotropical, Ethiopian, Oriental, and
/// Australasian — each defined by its characteristic fauna and the
/// dispersal barriers (mountain ranges, oceans, deserts) that prevent
/// free exchange with neighbouring regions. Modern zoogeography adds
/// phylogeography, which traces the genealogical relationships among
/// geographically separated populations using molecular clocks; the
/// pattern of genetic variation across a landscape reveals whether a
/// disjunct population represents one colonisation event or several,
/// and whether gene flow is still occurring. `range` builds via
/// `chart(amount)` and accumulates passively at `survey_rate` per
/// second in `tick(dt)` or retracts via `retract(amount)`.
///
/// Models faunal-range fill levels, biogeographic-survey saturation
/// bars, species-distribution accumulation trackers, Wallace-region
/// coverage gauges, dispersal-corridor fill levels, colonisation-
/// front advancement meters, island-biogeography arrival accumulators,
/// range-expansion progress bars, phylogeographic-sampling saturation
/// indicators, or any mechanic where patient exploration slowly
/// extends a creature's known territorial footprint across the map
/// until every biome has been surveyed and the fauna of every
/// continent logged in the great catalogue of living geography.
///
/// `chart(amount)` adds range; fires `just_mapped` when first
/// reaching `max_range`. No-op when disabled.
///
/// `retract(amount)` reduces range immediately; fires `just_uncharted`
/// when reaching 0. No-op when disabled or already uncharted.
///
/// `tick(dt)` clears both flags, then increases range by
/// `survey_rate * dt` (capped at `max_range`). Fires `just_mapped`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_mapped()` returns `range >= max_range && enabled`.
///
/// `is_uncharted()` returns `range == 0.0` (not gated by `enabled`).
///
/// `range_fraction()` returns `(range / max_range).clamp(0, 1)`.
///
/// `effective_dispersal(scale)` returns `scale * range_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — surveys at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoogeography {
    pub range: f32,
    pub max_range: f32,
    pub survey_rate: f32,
    pub just_mapped: bool,
    pub just_uncharted: bool,
    pub enabled: bool,
}

impl Zoogeography {
    pub fn new(max_range: f32, survey_rate: f32) -> Self {
        Self {
            range: 0.0,
            max_range: max_range.max(0.1),
            survey_rate: survey_rate.max(0.0),
            just_mapped: false,
            just_uncharted: false,
            enabled: true,
        }
    }

    /// Add range; fires `just_mapped` when first reaching max.
    /// No-op when disabled.
    pub fn chart(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.range < self.max_range;
        self.range = (self.range + amount).min(self.max_range);
        if was_below && self.range >= self.max_range {
            self.just_mapped = true;
        }
    }

    /// Reduce range; fires `just_uncharted` when reaching 0.
    /// No-op when disabled or already uncharted.
    pub fn retract(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.range <= 0.0 {
            return;
        }
        self.range = (self.range - amount).max(0.0);
        if self.range <= 0.0 {
            self.just_uncharted = true;
        }
    }

    /// Clear flags, then increase range by `survey_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_mapped = false;
        self.just_uncharted = false;
        if self.enabled && self.survey_rate > 0.0 && self.range < self.max_range {
            let was_below = self.range < self.max_range;
            self.range = (self.range + self.survey_rate * dt).min(self.max_range);
            if was_below && self.range >= self.max_range {
                self.just_mapped = true;
            }
        }
    }

    /// `true` when range is at maximum and component is enabled.
    pub fn is_mapped(&self) -> bool {
        self.range >= self.max_range && self.enabled
    }

    /// `true` when range is 0 (not gated by `enabled`).
    pub fn is_uncharted(&self) -> bool {
        self.range == 0.0
    }

    /// Fraction of maximum range [0.0, 1.0].
    pub fn range_fraction(&self) -> f32 {
        (self.range / self.max_range).clamp(0.0, 1.0)
    }

    /// Returns `scale * range_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_dispersal(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.range_fraction()
    }
}

impl Default for Zoogeography {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoogeography {
        Zoogeography::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_uncharted() {
        let z = z();
        assert_eq!(z.range, 0.0);
        assert!(z.is_uncharted());
        assert!(!z.is_mapped());
    }

    #[test]
    fn new_clamps_max_range() {
        let z = Zoogeography::new(-5.0, 1.5);
        assert!((z.max_range - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_survey_rate() {
        let z = Zoogeography::new(100.0, -1.5);
        assert_eq!(z.survey_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoogeography::default();
        assert!((z.max_range - 100.0).abs() < 1e-5);
        assert!((z.survey_rate - 1.5).abs() < 1e-5);
    }

    // --- chart ---

    #[test]
    fn chart_adds_range() {
        let mut z = z();
        z.chart(40.0);
        assert!((z.range - 40.0).abs() < 1e-3);
    }

    #[test]
    fn chart_clamps_at_max() {
        let mut z = z();
        z.chart(200.0);
        assert!((z.range - 100.0).abs() < 1e-3);
    }

    #[test]
    fn chart_fires_just_mapped_at_max() {
        let mut z = z();
        z.chart(100.0);
        assert!(z.just_mapped);
        assert!(z.is_mapped());
    }

    #[test]
    fn chart_no_just_mapped_when_already_at_max() {
        let mut z = z();
        z.range = 100.0;
        z.chart(10.0);
        assert!(!z.just_mapped);
    }

    #[test]
    fn chart_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.chart(50.0);
        assert_eq!(z.range, 0.0);
    }

    #[test]
    fn chart_no_op_when_amount_zero() {
        let mut z = z();
        z.chart(0.0);
        assert_eq!(z.range, 0.0);
    }

    // --- retract ---

    #[test]
    fn retract_reduces_range() {
        let mut z = z();
        z.range = 60.0;
        z.retract(20.0);
        assert!((z.range - 40.0).abs() < 1e-3);
    }

    #[test]
    fn retract_clamps_at_zero() {
        let mut z = z();
        z.range = 30.0;
        z.retract(200.0);
        assert_eq!(z.range, 0.0);
    }

    #[test]
    fn retract_fires_just_uncharted_at_zero() {
        let mut z = z();
        z.range = 30.0;
        z.retract(30.0);
        assert!(z.just_uncharted);
    }

    #[test]
    fn retract_no_op_when_already_uncharted() {
        let mut z = z();
        z.retract(10.0);
        assert!(!z.just_uncharted);
    }

    #[test]
    fn retract_no_op_when_disabled() {
        let mut z = z();
        z.range = 50.0;
        z.enabled = false;
        z.retract(50.0);
        assert!((z.range - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_surveys_range() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.range - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_mapped_on_survey_to_max() {
        let mut z = Zoogeography::new(100.0, 200.0);
        z.range = 95.0;
        z.tick(1.0);
        assert!(z.just_mapped);
        assert!(z.is_mapped());
    }

    #[test]
    fn tick_no_survey_when_already_mapped() {
        let mut z = z();
        z.range = 100.0;
        z.tick(1.0);
        assert!(!z.just_mapped);
    }

    #[test]
    fn tick_no_survey_when_rate_zero() {
        let mut z = Zoogeography::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.range, 0.0);
    }

    #[test]
    fn tick_no_survey_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.range, 0.0);
    }

    #[test]
    fn tick_clears_just_mapped() {
        let mut z = Zoogeography::new(100.0, 200.0);
        z.range = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_mapped);
    }

    #[test]
    fn tick_clears_just_uncharted() {
        let mut z = z();
        z.range = 10.0;
        z.retract(10.0);
        z.tick(0.016);
        assert!(!z.just_uncharted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.range - 9.0).abs() < 1e-3);
    }

    // --- is_mapped / is_uncharted ---

    #[test]
    fn is_mapped_false_when_disabled() {
        let mut z = z();
        z.range = 100.0;
        z.enabled = false;
        assert!(!z.is_mapped());
    }

    #[test]
    fn is_uncharted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_uncharted());
    }

    // --- range_fraction / effective_dispersal ---

    #[test]
    fn range_fraction_zero_when_uncharted() {
        assert_eq!(z().range_fraction(), 0.0);
    }

    #[test]
    fn range_fraction_half_at_midpoint() {
        let mut z = z();
        z.range = 50.0;
        assert!((z.range_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_dispersal_zero_when_uncharted() {
        assert_eq!(z().effective_dispersal(100.0), 0.0);
    }

    #[test]
    fn effective_dispersal_scales_with_range() {
        let mut z = z();
        z.range = 75.0;
        assert!((z.effective_dispersal(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_dispersal_zero_when_disabled() {
        let mut z = z();
        z.range = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_dispersal(100.0), 0.0);
    }
}
