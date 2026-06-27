use bevy_ecs::prelude::Component;

/// Ceramic-hardness tracker named after zirconia (ZrO₂), the oxide
/// mineral and industrial ceramic famous for its exceptional hardness,
/// high melting point, and use in dental implants, cutting tools,
/// and cubic-zirconia gemstones. `hardness` builds via
/// `fire(amount)` and increases passively at `sinter_rate` per
/// second in `tick(dt)` or is degraded via `crack(amount)`.
///
/// Models sintered-ceramic hardness bars, kiln-firing progress
/// meters, refractory-material density gauges, dental-crown
/// strength fill levels, thermal-barrier coating integrity bars,
/// cutting-tool wear-resistance accumulators, piezoelectric-
/// crystal quality trackers, ceramic-matrix composite toughness
/// meters, zirconia-toughening phase-transformation bars, or any
/// mechanic where repeated thermal cycles and controlled phase
/// transformations build an exceptionally hard, brilliantly white
/// material that can withstand temperatures no metal can — right
/// up until a stress concentration propagates a crack through the
/// structure and the whole dense ceramic shatters into powder.
///
/// `fire(amount)` adds hardness; fires `just_sintered` when first
/// reaching `max_hardness`. No-op when disabled.
///
/// `crack(amount)` reduces hardness immediately; fires
/// `just_cracked` when reaching 0. No-op when disabled or
/// already cracked.
///
/// `tick(dt)` clears both flags, then increases hardness by
/// `sinter_rate * dt` (capped at `max_hardness`). Fires
/// `just_sintered` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_sintered()` returns `hardness >= max_hardness && enabled`.
///
/// `is_cracked()` returns `hardness == 0.0` (not gated by `enabled`).
///
/// `hardness_fraction()` returns `(hardness / max_hardness).clamp(0, 1)`.
///
/// `effective_durability(scale)` returns `scale * hardness_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — sinters at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zirconia {
    pub hardness: f32,
    pub max_hardness: f32,
    pub sinter_rate: f32,
    pub just_sintered: bool,
    pub just_cracked: bool,
    pub enabled: bool,
}

impl Zirconia {
    pub fn new(max_hardness: f32, sinter_rate: f32) -> Self {
        Self {
            hardness: 0.0,
            max_hardness: max_hardness.max(0.1),
            sinter_rate: sinter_rate.max(0.0),
            just_sintered: false,
            just_cracked: false,
            enabled: true,
        }
    }

    /// Add hardness; fires `just_sintered` when first reaching max.
    /// No-op when disabled.
    pub fn fire(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.hardness < self.max_hardness;
        self.hardness = (self.hardness + amount).min(self.max_hardness);
        if was_below && self.hardness >= self.max_hardness {
            self.just_sintered = true;
        }
    }

    /// Reduce hardness; fires `just_cracked` when reaching 0.
    /// No-op when disabled or already cracked.
    pub fn crack(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.hardness <= 0.0 {
            return;
        }
        self.hardness = (self.hardness - amount).max(0.0);
        if self.hardness <= 0.0 {
            self.just_cracked = true;
        }
    }

    /// Clear flags, then increase hardness by `sinter_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_sintered = false;
        self.just_cracked = false;
        if self.enabled && self.sinter_rate > 0.0 && self.hardness < self.max_hardness {
            let was_below = self.hardness < self.max_hardness;
            self.hardness = (self.hardness + self.sinter_rate * dt).min(self.max_hardness);
            if was_below && self.hardness >= self.max_hardness {
                self.just_sintered = true;
            }
        }
    }

    /// `true` when hardness is at maximum and component is enabled.
    pub fn is_sintered(&self) -> bool {
        self.hardness >= self.max_hardness && self.enabled
    }

    /// `true` when hardness is 0 (not gated by `enabled`).
    pub fn is_cracked(&self) -> bool {
        self.hardness == 0.0
    }

    /// Fraction of maximum hardness [0.0, 1.0].
    pub fn hardness_fraction(&self) -> f32 {
        (self.hardness / self.max_hardness).clamp(0.0, 1.0)
    }

    /// Returns `scale * hardness_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_durability(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.hardness_fraction()
    }
}

impl Default for Zirconia {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zirconia {
        Zirconia::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_cracked() {
        let z = z();
        assert_eq!(z.hardness, 0.0);
        assert!(z.is_cracked());
        assert!(!z.is_sintered());
    }

    #[test]
    fn new_clamps_max_hardness() {
        let z = Zirconia::new(-5.0, 1.5);
        assert!((z.max_hardness - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_sinter_rate() {
        let z = Zirconia::new(100.0, -1.5);
        assert_eq!(z.sinter_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zirconia::default();
        assert!((z.max_hardness - 100.0).abs() < 1e-5);
        assert!((z.sinter_rate - 1.5).abs() < 1e-5);
    }

    // --- fire ---

    #[test]
    fn fire_adds_hardness() {
        let mut z = z();
        z.fire(40.0);
        assert!((z.hardness - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fire_clamps_at_max() {
        let mut z = z();
        z.fire(200.0);
        assert!((z.hardness - 100.0).abs() < 1e-3);
    }

    #[test]
    fn fire_fires_just_sintered_at_max() {
        let mut z = z();
        z.fire(100.0);
        assert!(z.just_sintered);
        assert!(z.is_sintered());
    }

    #[test]
    fn fire_no_just_sintered_when_already_at_max() {
        let mut z = z();
        z.hardness = 100.0;
        z.fire(10.0);
        assert!(!z.just_sintered);
    }

    #[test]
    fn fire_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.fire(50.0);
        assert_eq!(z.hardness, 0.0);
    }

    #[test]
    fn fire_no_op_when_amount_zero() {
        let mut z = z();
        z.fire(0.0);
        assert_eq!(z.hardness, 0.0);
    }

    // --- crack ---

    #[test]
    fn crack_reduces_hardness() {
        let mut z = z();
        z.hardness = 60.0;
        z.crack(20.0);
        assert!((z.hardness - 40.0).abs() < 1e-3);
    }

    #[test]
    fn crack_clamps_at_zero() {
        let mut z = z();
        z.hardness = 30.0;
        z.crack(200.0);
        assert_eq!(z.hardness, 0.0);
    }

    #[test]
    fn crack_fires_just_cracked_at_zero() {
        let mut z = z();
        z.hardness = 30.0;
        z.crack(30.0);
        assert!(z.just_cracked);
    }

    #[test]
    fn crack_no_op_when_already_cracked() {
        let mut z = z();
        z.crack(10.0);
        assert!(!z.just_cracked);
    }

    #[test]
    fn crack_no_op_when_disabled() {
        let mut z = z();
        z.hardness = 50.0;
        z.enabled = false;
        z.crack(50.0);
        assert!((z.hardness - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_sinters_hardness() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.hardness - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_sintered_on_sinter_to_max() {
        let mut z = Zirconia::new(100.0, 200.0);
        z.hardness = 95.0;
        z.tick(1.0);
        assert!(z.just_sintered);
        assert!(z.is_sintered());
    }

    #[test]
    fn tick_no_sinter_when_already_sintered() {
        let mut z = z();
        z.hardness = 100.0;
        z.tick(1.0);
        assert!(!z.just_sintered);
    }

    #[test]
    fn tick_no_sinter_when_rate_zero() {
        let mut z = Zirconia::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.hardness, 0.0);
    }

    #[test]
    fn tick_no_sinter_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.hardness, 0.0);
    }

    #[test]
    fn tick_clears_just_sintered() {
        let mut z = Zirconia::new(100.0, 200.0);
        z.hardness = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_sintered);
    }

    #[test]
    fn tick_clears_just_cracked() {
        let mut z = z();
        z.hardness = 10.0;
        z.crack(10.0);
        z.tick(0.016);
        assert!(!z.just_cracked);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.hardness - 9.0).abs() < 1e-3);
    }

    // --- is_sintered / is_cracked ---

    #[test]
    fn is_sintered_false_when_disabled() {
        let mut z = z();
        z.hardness = 100.0;
        z.enabled = false;
        assert!(!z.is_sintered());
    }

    #[test]
    fn is_cracked_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_cracked());
    }

    // --- hardness_fraction / effective_durability ---

    #[test]
    fn hardness_fraction_zero_when_cracked() {
        assert_eq!(z().hardness_fraction(), 0.0);
    }

    #[test]
    fn hardness_fraction_half_at_midpoint() {
        let mut z = z();
        z.hardness = 50.0;
        assert!((z.hardness_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_durability_zero_when_cracked() {
        assert_eq!(z().effective_durability(100.0), 0.0);
    }

    #[test]
    fn effective_durability_scales_with_hardness() {
        let mut z = z();
        z.hardness = 75.0;
        assert!((z.effective_durability(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_durability_zero_when_disabled() {
        let mut z = z();
        z.hardness = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_durability(100.0), 0.0);
    }
}
