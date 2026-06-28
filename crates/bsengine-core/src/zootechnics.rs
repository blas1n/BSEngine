use bevy_ecs::prelude::Component;

/// Animal-husbandry accumulation tracker named after zootechnics,
/// the science and practice of managing domestic animals for human
/// benefit — selecting breeding stock, designing feeding regimens,
/// preventing and treating disease, improving housing, and optimising
/// the productive performance of cattle, sheep, pigs, poultry, fish,
/// and the many other species that supply humanity with meat, milk,
/// fibre, traction, and companionship. The word derives from Greek
/// zōon (animal) plus technē (art or craft), and it encompasses a
/// spectrum of knowledge that ranges from the pre-scientific wisdom
/// of peasant farmers who recognised that a cow's milk let-down
/// depended on calf presence all the way to the molecular breeding
/// programmes that now select dairy animals by genomic markers
/// predicting yield, fertility, and longevity years before a heifer
/// has calved. The discipline is inseparable from civilisation: the
/// Neolithic package that enabled settled farming comprised not just
/// wheat and barley cultivation but the simultaneous domestication
/// of cattle, sheep, goats, and pigs, each species managed by
/// accumulated craft knowledge that was refined across generations
/// without ever being written down. Modern zootechnics applies
/// nutrition science, reproductive physiology, veterinary medicine,
/// genetics, and even computational modelling to answer the central
/// question: how can a given population of animals be managed to
/// maximise human utility while preserving the welfare and productive
/// longevity of the animals themselves? `husbandry` builds via
/// `tend(amount)` and accumulates passively at `tend_rate` per
/// second in `tick(dt)` or falls via `neglect(amount)`.
///
/// Models animal-husbandry fill levels, livestock-management
/// saturation bars, breeding-programme progress trackers, veterinary-
/// care accumulation gauges, pasture-improvement fill levels,
/// stock-welfare saturation indicators, feed-optimisation
/// accumulation bars, zootechnical-mastery meters, herd-productivity
/// fill levels, or any mechanic where a player or faction slowly
/// perfects the science of caring for domesticated animals — refining
/// the feed ration, culling the unhealthy and selecting the
/// productive, building shelter that keeps disease at bay and stress
/// below the threshold that suppresses growth — until the herd is a
/// precision instrument for converting grass into protein.
///
/// `tend(amount)` adds husbandry; fires `just_thriving` when first
/// reaching `max_husbandry`. No-op when disabled.
///
/// `neglect(amount)` reduces husbandry immediately; fires
/// `just_declining` when reaching 0. No-op when disabled or already
/// declining.
///
/// `tick(dt)` clears both flags, then increases husbandry by
/// `tend_rate * dt` (capped at `max_husbandry`). Fires `just_thriving`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_thriving()` returns `husbandry >= max_husbandry && enabled`.
///
/// `is_declining()` returns `husbandry == 0.0` (not gated by
/// `enabled`).
///
/// `husbandry_fraction()` returns
/// `(husbandry / max_husbandry).clamp(0, 1)`.
///
/// `effective_yield(scale)` returns `scale * husbandry_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — tends at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zootechnics {
    pub husbandry: f32,
    pub max_husbandry: f32,
    pub tend_rate: f32,
    pub just_thriving: bool,
    pub just_declining: bool,
    pub enabled: bool,
}

impl Zootechnics {
    pub fn new(max_husbandry: f32, tend_rate: f32) -> Self {
        Self {
            husbandry: 0.0,
            max_husbandry: max_husbandry.max(0.1),
            tend_rate: tend_rate.max(0.0),
            just_thriving: false,
            just_declining: false,
            enabled: true,
        }
    }

    /// Add husbandry; fires `just_thriving` when first reaching max.
    /// No-op when disabled.
    pub fn tend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.husbandry < self.max_husbandry;
        self.husbandry = (self.husbandry + amount).min(self.max_husbandry);
        if was_below && self.husbandry >= self.max_husbandry {
            self.just_thriving = true;
        }
    }

    /// Reduce husbandry; fires `just_declining` when reaching 0.
    /// No-op when disabled or already declining.
    pub fn neglect(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.husbandry <= 0.0 {
            return;
        }
        self.husbandry = (self.husbandry - amount).max(0.0);
        if self.husbandry <= 0.0 {
            self.just_declining = true;
        }
    }

    /// Clear flags, then increase husbandry by `tend_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_thriving = false;
        self.just_declining = false;
        if self.enabled && self.tend_rate > 0.0 && self.husbandry < self.max_husbandry {
            let was_below = self.husbandry < self.max_husbandry;
            self.husbandry = (self.husbandry + self.tend_rate * dt).min(self.max_husbandry);
            if was_below && self.husbandry >= self.max_husbandry {
                self.just_thriving = true;
            }
        }
    }

    /// `true` when husbandry is at maximum and component is enabled.
    pub fn is_thriving(&self) -> bool {
        self.husbandry >= self.max_husbandry && self.enabled
    }

    /// `true` when husbandry is 0 (not gated by `enabled`).
    pub fn is_declining(&self) -> bool {
        self.husbandry == 0.0
    }

    /// Fraction of maximum husbandry [0.0, 1.0].
    pub fn husbandry_fraction(&self) -> f32 {
        (self.husbandry / self.max_husbandry).clamp(0.0, 1.0)
    }

    /// Returns `scale * husbandry_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_yield(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.husbandry_fraction()
    }
}

impl Default for Zootechnics {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zootechnics {
        Zootechnics::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_declining() {
        let z = z();
        assert_eq!(z.husbandry, 0.0);
        assert!(z.is_declining());
        assert!(!z.is_thriving());
    }

    #[test]
    fn new_clamps_max_husbandry() {
        let z = Zootechnics::new(-5.0, 1.5);
        assert!((z.max_husbandry - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_tend_rate() {
        let z = Zootechnics::new(100.0, -1.5);
        assert_eq!(z.tend_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zootechnics::default();
        assert!((z.max_husbandry - 100.0).abs() < 1e-5);
        assert!((z.tend_rate - 1.5).abs() < 1e-5);
    }

    // --- tend ---

    #[test]
    fn tend_adds_husbandry() {
        let mut z = z();
        z.tend(40.0);
        assert!((z.husbandry - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tend_clamps_at_max() {
        let mut z = z();
        z.tend(200.0);
        assert!((z.husbandry - 100.0).abs() < 1e-3);
    }

    #[test]
    fn tend_fires_just_thriving_at_max() {
        let mut z = z();
        z.tend(100.0);
        assert!(z.just_thriving);
        assert!(z.is_thriving());
    }

    #[test]
    fn tend_no_just_thriving_when_already_at_max() {
        let mut z = z();
        z.husbandry = 100.0;
        z.tend(10.0);
        assert!(!z.just_thriving);
    }

    #[test]
    fn tend_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tend(50.0);
        assert_eq!(z.husbandry, 0.0);
    }

    #[test]
    fn tend_no_op_when_amount_zero() {
        let mut z = z();
        z.tend(0.0);
        assert_eq!(z.husbandry, 0.0);
    }

    // --- neglect ---

    #[test]
    fn neglect_reduces_husbandry() {
        let mut z = z();
        z.husbandry = 60.0;
        z.neglect(20.0);
        assert!((z.husbandry - 40.0).abs() < 1e-3);
    }

    #[test]
    fn neglect_clamps_at_zero() {
        let mut z = z();
        z.husbandry = 30.0;
        z.neglect(200.0);
        assert_eq!(z.husbandry, 0.0);
    }

    #[test]
    fn neglect_fires_just_declining_at_zero() {
        let mut z = z();
        z.husbandry = 30.0;
        z.neglect(30.0);
        assert!(z.just_declining);
    }

    #[test]
    fn neglect_no_op_when_already_declining() {
        let mut z = z();
        z.neglect(10.0);
        assert!(!z.just_declining);
    }

    #[test]
    fn neglect_no_op_when_disabled() {
        let mut z = z();
        z.husbandry = 50.0;
        z.enabled = false;
        z.neglect(50.0);
        assert!((z.husbandry - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_tends_husbandry() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.husbandry - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_thriving_on_tend_to_max() {
        let mut z = Zootechnics::new(100.0, 200.0);
        z.husbandry = 95.0;
        z.tick(1.0);
        assert!(z.just_thriving);
        assert!(z.is_thriving());
    }

    #[test]
    fn tick_no_tend_when_already_thriving() {
        let mut z = z();
        z.husbandry = 100.0;
        z.tick(1.0);
        assert!(!z.just_thriving);
    }

    #[test]
    fn tick_no_tend_when_rate_zero() {
        let mut z = Zootechnics::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.husbandry, 0.0);
    }

    #[test]
    fn tick_no_tend_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.husbandry, 0.0);
    }

    #[test]
    fn tick_clears_just_thriving() {
        let mut z = Zootechnics::new(100.0, 200.0);
        z.husbandry = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_thriving);
    }

    #[test]
    fn tick_clears_just_declining() {
        let mut z = z();
        z.husbandry = 10.0;
        z.neglect(10.0);
        z.tick(0.016);
        assert!(!z.just_declining);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.husbandry - 9.0).abs() < 1e-3);
    }

    // --- is_thriving / is_declining ---

    #[test]
    fn is_thriving_false_when_disabled() {
        let mut z = z();
        z.husbandry = 100.0;
        z.enabled = false;
        assert!(!z.is_thriving());
    }

    #[test]
    fn is_declining_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_declining());
    }

    // --- husbandry_fraction / effective_yield ---

    #[test]
    fn husbandry_fraction_zero_when_declining() {
        assert_eq!(z().husbandry_fraction(), 0.0);
    }

    #[test]
    fn husbandry_fraction_half_at_midpoint() {
        let mut z = z();
        z.husbandry = 50.0;
        assert!((z.husbandry_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_yield_zero_when_declining() {
        assert_eq!(z().effective_yield(100.0), 0.0);
    }

    #[test]
    fn effective_yield_scales_with_husbandry() {
        let mut z = z();
        z.husbandry = 75.0;
        assert!((z.effective_yield(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_yield_zero_when_disabled() {
        let mut z = z();
        z.husbandry = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_yield(100.0), 0.0);
    }
}
