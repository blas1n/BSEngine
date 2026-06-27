use bevy_ecs::prelude::Component;

/// Coral-symbiosis tracker. `symbiosis` builds via `seed(amount)` and
/// photosynthesises passively at `photosynthesize_rate` per second in
/// `tick(dt)` or is expelled immediately via `expel(amount)`.
///
/// Models coral-polyp symbiosis health bars, reef-vitality
/// accumulators, bleaching-stress fill levels, endosymbiont-density
/// gauges, dinoflagellate-retention strength trackers, coral-color
/// saturation indicators, photosynthetic-output intensity bars,
/// thermal-stress resistance meters, or any mechanic where a tiny
/// algal partner slowly saturates every host cell with glucose and
/// color until a heat wave drives the last dinoflagellate out and
/// leaves nothing but bleached white calcium carbonate behind.
///
/// `seed(amount)` adds symbiosis; fires `just_thriving` when first
/// reaching `max_symbiosis`. No-op when disabled.
///
/// `expel(amount)` reduces symbiosis immediately; fires `just_bleached`
/// when reaching 0. No-op when disabled or already bleached.
///
/// `tick(dt)` clears both flags, then increases symbiosis by
/// `photosynthesize_rate * dt` (capped at `max_symbiosis`). Fires
/// `just_thriving` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_thriving()` returns `symbiosis >= max_symbiosis && enabled`.
///
/// `is_bleached()` returns `symbiosis == 0.0` (not gated by `enabled`).
///
/// `symbiosis_fraction()` returns `(symbiosis / max_symbiosis).clamp(0, 1)`.
///
/// `effective_photosynthesis(scale)` returns `scale * symbiosis_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 3.0)` — photosynthesises at 3 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooxanthella {
    pub symbiosis: f32,
    pub max_symbiosis: f32,
    pub photosynthesize_rate: f32,
    pub just_thriving: bool,
    pub just_bleached: bool,
    pub enabled: bool,
}

impl Zooxanthella {
    pub fn new(max_symbiosis: f32, photosynthesize_rate: f32) -> Self {
        Self {
            symbiosis: 0.0,
            max_symbiosis: max_symbiosis.max(0.1),
            photosynthesize_rate: photosynthesize_rate.max(0.0),
            just_thriving: false,
            just_bleached: false,
            enabled: true,
        }
    }

    /// Add symbiosis; fires `just_thriving` when first reaching max.
    /// No-op when disabled.
    pub fn seed(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.symbiosis < self.max_symbiosis;
        self.symbiosis = (self.symbiosis + amount).min(self.max_symbiosis);
        if was_below && self.symbiosis >= self.max_symbiosis {
            self.just_thriving = true;
        }
    }

    /// Reduce symbiosis; fires `just_bleached` when reaching 0.
    /// No-op when disabled or already bleached.
    pub fn expel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.symbiosis <= 0.0 {
            return;
        }
        self.symbiosis = (self.symbiosis - amount).max(0.0);
        if self.symbiosis <= 0.0 {
            self.just_bleached = true;
        }
    }

    /// Clear flags, then increase symbiosis by `photosynthesize_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_thriving = false;
        self.just_bleached = false;
        if self.enabled && self.photosynthesize_rate > 0.0 && self.symbiosis < self.max_symbiosis {
            let was_below = self.symbiosis < self.max_symbiosis;
            self.symbiosis =
                (self.symbiosis + self.photosynthesize_rate * dt).min(self.max_symbiosis);
            if was_below && self.symbiosis >= self.max_symbiosis {
                self.just_thriving = true;
            }
        }
    }

    /// `true` when symbiosis is at maximum and component is enabled.
    pub fn is_thriving(&self) -> bool {
        self.symbiosis >= self.max_symbiosis && self.enabled
    }

    /// `true` when symbiosis is 0 (not gated by `enabled`).
    pub fn is_bleached(&self) -> bool {
        self.symbiosis == 0.0
    }

    /// Fraction of maximum symbiosis [0.0, 1.0].
    pub fn symbiosis_fraction(&self) -> f32 {
        (self.symbiosis / self.max_symbiosis).clamp(0.0, 1.0)
    }

    /// Returns `scale * symbiosis_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_photosynthesis(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.symbiosis_fraction()
    }
}

impl Default for Zooxanthella {
    fn default() -> Self {
        Self::new(100.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zooxanthella {
        Zooxanthella::new(100.0, 3.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_bleached() {
        let z = z();
        assert_eq!(z.symbiosis, 0.0);
        assert!(z.is_bleached());
        assert!(!z.is_thriving());
    }

    #[test]
    fn new_clamps_max_symbiosis() {
        let z = Zooxanthella::new(-5.0, 3.0);
        assert!((z.max_symbiosis - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_photosynthesize_rate() {
        let z = Zooxanthella::new(100.0, -3.0);
        assert_eq!(z.photosynthesize_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zooxanthella::default();
        assert!((z.max_symbiosis - 100.0).abs() < 1e-5);
        assert!((z.photosynthesize_rate - 3.0).abs() < 1e-5);
    }

    // --- seed ---

    #[test]
    fn seed_adds_symbiosis() {
        let mut z = z();
        z.seed(40.0);
        assert!((z.symbiosis - 40.0).abs() < 1e-3);
    }

    #[test]
    fn seed_clamps_at_max() {
        let mut z = z();
        z.seed(200.0);
        assert!((z.symbiosis - 100.0).abs() < 1e-3);
    }

    #[test]
    fn seed_fires_just_thriving_at_max() {
        let mut z = z();
        z.seed(100.0);
        assert!(z.just_thriving);
        assert!(z.is_thriving());
    }

    #[test]
    fn seed_no_just_thriving_when_already_at_max() {
        let mut z = z();
        z.symbiosis = 100.0;
        z.seed(10.0);
        assert!(!z.just_thriving);
    }

    #[test]
    fn seed_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.seed(50.0);
        assert_eq!(z.symbiosis, 0.0);
    }

    #[test]
    fn seed_no_op_when_amount_zero() {
        let mut z = z();
        z.seed(0.0);
        assert_eq!(z.symbiosis, 0.0);
    }

    // --- expel ---

    #[test]
    fn expel_reduces_symbiosis() {
        let mut z = z();
        z.symbiosis = 60.0;
        z.expel(20.0);
        assert!((z.symbiosis - 40.0).abs() < 1e-3);
    }

    #[test]
    fn expel_clamps_at_zero() {
        let mut z = z();
        z.symbiosis = 30.0;
        z.expel(200.0);
        assert_eq!(z.symbiosis, 0.0);
    }

    #[test]
    fn expel_fires_just_bleached_at_zero() {
        let mut z = z();
        z.symbiosis = 30.0;
        z.expel(30.0);
        assert!(z.just_bleached);
    }

    #[test]
    fn expel_no_op_when_already_bleached() {
        let mut z = z();
        z.expel(10.0);
        assert!(!z.just_bleached);
    }

    #[test]
    fn expel_no_op_when_disabled() {
        let mut z = z();
        z.symbiosis = 50.0;
        z.enabled = false;
        z.expel(50.0);
        assert!((z.symbiosis - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_photosynthesises() {
        let mut z = z(); // rate=3
        z.tick(2.0); // 0 + 3*2 = 6
        assert!((z.symbiosis - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_thriving_on_fill_to_max() {
        let mut z = Zooxanthella::new(100.0, 200.0);
        z.symbiosis = 95.0;
        z.tick(1.0);
        assert!(z.just_thriving);
        assert!(z.is_thriving());
    }

    #[test]
    fn tick_no_growth_when_already_thriving() {
        let mut z = z();
        z.symbiosis = 100.0;
        z.tick(1.0);
        assert!(!z.just_thriving);
    }

    #[test]
    fn tick_no_growth_when_rate_zero() {
        let mut z = Zooxanthella::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.symbiosis, 0.0);
    }

    #[test]
    fn tick_no_growth_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.symbiosis, 0.0);
    }

    #[test]
    fn tick_clears_just_thriving() {
        let mut z = Zooxanthella::new(100.0, 200.0);
        z.symbiosis = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_thriving);
    }

    #[test]
    fn tick_clears_just_bleached() {
        let mut z = z();
        z.symbiosis = 10.0;
        z.expel(10.0);
        z.tick(0.016);
        assert!(!z.just_bleached);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=3
        z.tick(4.0); // 3*4 = 12
        assert!((z.symbiosis - 12.0).abs() < 1e-3);
    }

    // --- is_thriving / is_bleached ---

    #[test]
    fn is_thriving_false_when_disabled() {
        let mut z = z();
        z.symbiosis = 100.0;
        z.enabled = false;
        assert!(!z.is_thriving());
    }

    #[test]
    fn is_bleached_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_bleached());
    }

    // --- symbiosis_fraction / effective_photosynthesis ---

    #[test]
    fn symbiosis_fraction_zero_when_bleached() {
        assert_eq!(z().symbiosis_fraction(), 0.0);
    }

    #[test]
    fn symbiosis_fraction_half_at_midpoint() {
        let mut z = z();
        z.symbiosis = 50.0;
        assert!((z.symbiosis_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_photosynthesis_zero_when_bleached() {
        assert_eq!(z().effective_photosynthesis(100.0), 0.0);
    }

    #[test]
    fn effective_photosynthesis_scales_with_symbiosis() {
        let mut z = z();
        z.symbiosis = 75.0;
        assert!((z.effective_photosynthesis(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_photosynthesis_zero_when_disabled() {
        let mut z = z();
        z.symbiosis = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_photosynthesis(100.0), 0.0);
    }
}
