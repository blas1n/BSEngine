use bevy_ecs::prelude::Component;

/// Motile-gamete accumulation tracker named after zoosperm, the
/// botanical and zoological term for a flagellated male gamete capable
/// of swimming under its own power. The word collapses into a single
/// label the two complementary meanings recorded in the major
/// dictionaries: first, the animal spermatozoon — the streamlined,
/// tail-driven cell that carries half of an animal's genome toward an
/// egg; and second, the zoospore of algae and lower fungi — a
/// flagellated asexual spore that swims through water to colonise a
/// new substrate. In the animal sense, the zoosperm is the product
/// of spermatogenesis: diploid spermatogonia divide by mitosis to
/// produce spermatocytes, which then undergo two rounds of meiosis to
/// yield haploid spermatids that differentiate into the distinctive
/// head-midpiece-tail architecture of the mature spermatozoon. In the
/// botanical sense, zoosperms are the motile stage in the life cycles
/// of charophytes, bryophytes, pteridophytes, and cycads — groups that
/// have not yet evolved the pollen tube and still depend on a film of
/// free water to carry sperm from antheridium to archegonium. Both
/// meanings share the essential property: a self-propelled packet of
/// genetic information that must navigate an external medium to reach
/// its target. `motility` builds via `propel(amount)` and accumulates
/// passively at `swim_rate` per second in `tick(dt)` or decays via
/// `arrest(amount)`.
///
/// Models motile-gamete fill levels, spermatozoon-count saturation
/// bars, fertilisation-potential accumulation trackers, spore-
/// dispersal gauges, flagellar-motility fill levels, reproductive-
/// vigour saturation indicators, sperm-competition accumulation bars,
/// gamete-viability meters, bryophyte-sperm-cloud fill levels, or any
/// mechanic where an organism gradually produces or concentrates
/// enough motile propagules to achieve fertilisation — and where
/// stress, toxins, or time erode that reservoir back toward sterility.
///
/// `propel(amount)` adds motility; fires `just_fertile` when first
/// reaching `max_motility`. No-op when disabled.
///
/// `arrest(amount)` reduces motility immediately; fires
/// `just_sterile` when reaching 0. No-op when disabled or already
/// sterile.
///
/// `tick(dt)` clears both flags, then increases motility by
/// `swim_rate * dt` (capped at `max_motility`). Fires `just_fertile`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_fertile()` returns `motility >= max_motility && enabled`.
///
/// `is_sterile()` returns `motility == 0.0` (not gated by `enabled`).
///
/// `motility_fraction()` returns
/// `(motility / max_motility).clamp(0, 1)`.
///
/// `effective_dispersal(scale)` returns `scale * motility_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — swims at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoosperm {
    pub motility: f32,
    pub max_motility: f32,
    pub swim_rate: f32,
    pub just_fertile: bool,
    pub just_sterile: bool,
    pub enabled: bool,
}

impl Zoosperm {
    pub fn new(max_motility: f32, swim_rate: f32) -> Self {
        Self {
            motility: 0.0,
            max_motility: max_motility.max(0.1),
            swim_rate: swim_rate.max(0.0),
            just_fertile: false,
            just_sterile: false,
            enabled: true,
        }
    }

    /// Add motility; fires `just_fertile` when first reaching max.
    /// No-op when disabled.
    pub fn propel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.motility < self.max_motility;
        self.motility = (self.motility + amount).min(self.max_motility);
        if was_below && self.motility >= self.max_motility {
            self.just_fertile = true;
        }
    }

    /// Reduce motility; fires `just_sterile` when reaching 0.
    /// No-op when disabled or already sterile.
    pub fn arrest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.motility <= 0.0 {
            return;
        }
        self.motility = (self.motility - amount).max(0.0);
        if self.motility <= 0.0 {
            self.just_sterile = true;
        }
    }

    /// Clear flags, then increase motility by `swim_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_fertile = false;
        self.just_sterile = false;
        if self.enabled && self.swim_rate > 0.0 && self.motility < self.max_motility {
            let was_below = self.motility < self.max_motility;
            self.motility = (self.motility + self.swim_rate * dt).min(self.max_motility);
            if was_below && self.motility >= self.max_motility {
                self.just_fertile = true;
            }
        }
    }

    /// `true` when motility is at maximum and component is enabled.
    pub fn is_fertile(&self) -> bool {
        self.motility >= self.max_motility && self.enabled
    }

    /// `true` when motility is 0 (not gated by `enabled`).
    pub fn is_sterile(&self) -> bool {
        self.motility == 0.0
    }

    /// Fraction of maximum motility [0.0, 1.0].
    pub fn motility_fraction(&self) -> f32 {
        (self.motility / self.max_motility).clamp(0.0, 1.0)
    }

    /// Returns `scale * motility_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_dispersal(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.motility_fraction()
    }
}

impl Default for Zoosperm {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoosperm {
        Zoosperm::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_sterile() {
        let z = z();
        assert_eq!(z.motility, 0.0);
        assert!(z.is_sterile());
        assert!(!z.is_fertile());
    }

    #[test]
    fn new_clamps_max_motility() {
        let z = Zoosperm::new(-5.0, 1.5);
        assert!((z.max_motility - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_swim_rate() {
        let z = Zoosperm::new(100.0, -1.5);
        assert_eq!(z.swim_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoosperm::default();
        assert!((z.max_motility - 100.0).abs() < 1e-5);
        assert!((z.swim_rate - 1.5).abs() < 1e-5);
    }

    // --- propel ---

    #[test]
    fn propel_adds_motility() {
        let mut z = z();
        z.propel(40.0);
        assert!((z.motility - 40.0).abs() < 1e-3);
    }

    #[test]
    fn propel_clamps_at_max() {
        let mut z = z();
        z.propel(200.0);
        assert!((z.motility - 100.0).abs() < 1e-3);
    }

    #[test]
    fn propel_fires_just_fertile_at_max() {
        let mut z = z();
        z.propel(100.0);
        assert!(z.just_fertile);
        assert!(z.is_fertile());
    }

    #[test]
    fn propel_no_just_fertile_when_already_at_max() {
        let mut z = z();
        z.motility = 100.0;
        z.propel(10.0);
        assert!(!z.just_fertile);
    }

    #[test]
    fn propel_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.propel(50.0);
        assert_eq!(z.motility, 0.0);
    }

    #[test]
    fn propel_no_op_when_amount_zero() {
        let mut z = z();
        z.propel(0.0);
        assert_eq!(z.motility, 0.0);
    }

    // --- arrest ---

    #[test]
    fn arrest_reduces_motility() {
        let mut z = z();
        z.motility = 60.0;
        z.arrest(20.0);
        assert!((z.motility - 40.0).abs() < 1e-3);
    }

    #[test]
    fn arrest_clamps_at_zero() {
        let mut z = z();
        z.motility = 30.0;
        z.arrest(200.0);
        assert_eq!(z.motility, 0.0);
    }

    #[test]
    fn arrest_fires_just_sterile_at_zero() {
        let mut z = z();
        z.motility = 30.0;
        z.arrest(30.0);
        assert!(z.just_sterile);
    }

    #[test]
    fn arrest_no_op_when_already_sterile() {
        let mut z = z();
        z.arrest(10.0);
        assert!(!z.just_sterile);
    }

    #[test]
    fn arrest_no_op_when_disabled() {
        let mut z = z();
        z.motility = 50.0;
        z.enabled = false;
        z.arrest(50.0);
        assert!((z.motility - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_propels_motility() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.motility - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_fertile_on_propel_to_max() {
        let mut z = Zoosperm::new(100.0, 200.0);
        z.motility = 95.0;
        z.tick(1.0);
        assert!(z.just_fertile);
        assert!(z.is_fertile());
    }

    #[test]
    fn tick_no_propel_when_already_fertile() {
        let mut z = z();
        z.motility = 100.0;
        z.tick(1.0);
        assert!(!z.just_fertile);
    }

    #[test]
    fn tick_no_propel_when_rate_zero() {
        let mut z = Zoosperm::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.motility, 0.0);
    }

    #[test]
    fn tick_no_propel_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.motility, 0.0);
    }

    #[test]
    fn tick_clears_just_fertile() {
        let mut z = Zoosperm::new(100.0, 200.0);
        z.motility = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_fertile);
    }

    #[test]
    fn tick_clears_just_sterile() {
        let mut z = z();
        z.motility = 10.0;
        z.arrest(10.0);
        z.tick(0.016);
        assert!(!z.just_sterile);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.motility - 9.0).abs() < 1e-3);
    }

    // --- is_fertile / is_sterile ---

    #[test]
    fn is_fertile_false_when_disabled() {
        let mut z = z();
        z.motility = 100.0;
        z.enabled = false;
        assert!(!z.is_fertile());
    }

    #[test]
    fn is_sterile_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_sterile());
    }

    // --- motility_fraction / effective_dispersal ---

    #[test]
    fn motility_fraction_zero_when_sterile() {
        assert_eq!(z().motility_fraction(), 0.0);
    }

    #[test]
    fn motility_fraction_half_at_midpoint() {
        let mut z = z();
        z.motility = 50.0;
        assert!((z.motility_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_dispersal_zero_when_sterile() {
        assert_eq!(z().effective_dispersal(100.0), 0.0);
    }

    #[test]
    fn effective_dispersal_scales_with_motility() {
        let mut z = z();
        z.motility = 75.0;
        assert!((z.effective_dispersal(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_dispersal_zero_when_disabled() {
        let mut z = z();
        z.motility = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_dispersal(100.0), 0.0);
    }
}
