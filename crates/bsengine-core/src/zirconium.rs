use bevy_ecs::prelude::Component;

/// Nuclear-grade metal purity tracker named after zirconium (Zr),
/// atomic number 40, a lustrous grey-white transition metal prized
/// above all others for its almost vanishingly small thermal-neutron
/// absorption cross-section — a property that makes it uniquely
/// suited to cladding nuclear fuel rods, where any material that
/// captures neutrons wastes fission events and reduces reactor
/// efficiency. Zirconium is also exceptionally corrosion-resistant,
/// forms a self-healing oxide layer on contact with air, and
/// maintains its structural integrity at temperatures that would
/// dissolve lesser metals. `purity` builds via `smelt(amount)` and
/// increases passively at `refine_rate` per second in `tick(dt)` or
/// is reduced via `corrode(amount)`.
///
/// Models nuclear-fuel-cladding purity bars, reactor-grade-metal
/// smelting fill levels, corrosion-resistance saturation trackers,
/// zirconium-alloy (zircaloy) refinement progress meters, nuclear-
/// materials-purity completion gauges, chemical-processing-vessel
/// integrity bars, hafnium-separation completeness trackers (hafnium
/// must be removed to achieve the low neutron-absorption cross-
/// section), surgical-instrument-grade metal-purity fill levels,
/// or any mechanic where the slow accumulation of refining passes
/// drives a metal's purity toward the narrow specification window
/// that separates a functional reactor component from a
/// catastrophic one.
///
/// `smelt(amount)` adds purity; fires `just_refined` when first
/// reaching `max_purity`. No-op when disabled.
///
/// `corrode(amount)` reduces purity immediately; fires
/// `just_tarnished` when reaching 0. No-op when disabled or
/// already tarnished.
///
/// `tick(dt)` clears both flags, then increases purity by
/// `refine_rate * dt` (capped at `max_purity`). Fires
/// `just_refined` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_refined()` returns `purity >= max_purity && enabled`.
///
/// `is_tarnished()` returns `purity == 0.0` (not gated by `enabled`).
///
/// `purity_fraction()` returns `(purity / max_purity).clamp(0, 1)`.
///
/// `effective_shielding(scale)` returns `scale * purity_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — refines at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zirconium {
    pub purity: f32,
    pub max_purity: f32,
    pub refine_rate: f32,
    pub just_refined: bool,
    pub just_tarnished: bool,
    pub enabled: bool,
}

impl Zirconium {
    pub fn new(max_purity: f32, refine_rate: f32) -> Self {
        Self {
            purity: 0.0,
            max_purity: max_purity.max(0.1),
            refine_rate: refine_rate.max(0.0),
            just_refined: false,
            just_tarnished: false,
            enabled: true,
        }
    }

    /// Add purity; fires `just_refined` when first reaching max.
    /// No-op when disabled.
    pub fn smelt(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.purity < self.max_purity;
        self.purity = (self.purity + amount).min(self.max_purity);
        if was_below && self.purity >= self.max_purity {
            self.just_refined = true;
        }
    }

    /// Reduce purity; fires `just_tarnished` when reaching 0.
    /// No-op when disabled or already tarnished.
    pub fn corrode(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.purity <= 0.0 {
            return;
        }
        self.purity = (self.purity - amount).max(0.0);
        if self.purity <= 0.0 {
            self.just_tarnished = true;
        }
    }

    /// Clear flags, then increase purity by `refine_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_refined = false;
        self.just_tarnished = false;
        if self.enabled && self.refine_rate > 0.0 && self.purity < self.max_purity {
            let was_below = self.purity < self.max_purity;
            self.purity = (self.purity + self.refine_rate * dt).min(self.max_purity);
            if was_below && self.purity >= self.max_purity {
                self.just_refined = true;
            }
        }
    }

    /// `true` when purity is at maximum and component is enabled.
    pub fn is_refined(&self) -> bool {
        self.purity >= self.max_purity && self.enabled
    }

    /// `true` when purity is 0 (not gated by `enabled`).
    pub fn is_tarnished(&self) -> bool {
        self.purity == 0.0
    }

    /// Fraction of maximum purity [0.0, 1.0].
    pub fn purity_fraction(&self) -> f32 {
        (self.purity / self.max_purity).clamp(0.0, 1.0)
    }

    /// Returns `scale * purity_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_shielding(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.purity_fraction()
    }
}

impl Default for Zirconium {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zirconium {
        Zirconium::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_tarnished() {
        let z = z();
        assert_eq!(z.purity, 0.0);
        assert!(z.is_tarnished());
        assert!(!z.is_refined());
    }

    #[test]
    fn new_clamps_max_purity() {
        let z = Zirconium::new(-5.0, 1.5);
        assert!((z.max_purity - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_refine_rate() {
        let z = Zirconium::new(100.0, -1.5);
        assert_eq!(z.refine_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zirconium::default();
        assert!((z.max_purity - 100.0).abs() < 1e-5);
        assert!((z.refine_rate - 1.5).abs() < 1e-5);
    }

    // --- smelt ---

    #[test]
    fn smelt_adds_purity() {
        let mut z = z();
        z.smelt(40.0);
        assert!((z.purity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn smelt_clamps_at_max() {
        let mut z = z();
        z.smelt(200.0);
        assert!((z.purity - 100.0).abs() < 1e-3);
    }

    #[test]
    fn smelt_fires_just_refined_at_max() {
        let mut z = z();
        z.smelt(100.0);
        assert!(z.just_refined);
        assert!(z.is_refined());
    }

    #[test]
    fn smelt_no_just_refined_when_already_at_max() {
        let mut z = z();
        z.purity = 100.0;
        z.smelt(10.0);
        assert!(!z.just_refined);
    }

    #[test]
    fn smelt_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.smelt(50.0);
        assert_eq!(z.purity, 0.0);
    }

    #[test]
    fn smelt_no_op_when_amount_zero() {
        let mut z = z();
        z.smelt(0.0);
        assert_eq!(z.purity, 0.0);
    }

    // --- corrode ---

    #[test]
    fn corrode_reduces_purity() {
        let mut z = z();
        z.purity = 60.0;
        z.corrode(20.0);
        assert!((z.purity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn corrode_clamps_at_zero() {
        let mut z = z();
        z.purity = 30.0;
        z.corrode(200.0);
        assert_eq!(z.purity, 0.0);
    }

    #[test]
    fn corrode_fires_just_tarnished_at_zero() {
        let mut z = z();
        z.purity = 30.0;
        z.corrode(30.0);
        assert!(z.just_tarnished);
    }

    #[test]
    fn corrode_no_op_when_already_tarnished() {
        let mut z = z();
        z.corrode(10.0);
        assert!(!z.just_tarnished);
    }

    #[test]
    fn corrode_no_op_when_disabled() {
        let mut z = z();
        z.purity = 50.0;
        z.enabled = false;
        z.corrode(50.0);
        assert!((z.purity - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_refines_purity() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.purity - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_refined_on_refine_to_max() {
        let mut z = Zirconium::new(100.0, 200.0);
        z.purity = 95.0;
        z.tick(1.0);
        assert!(z.just_refined);
        assert!(z.is_refined());
    }

    #[test]
    fn tick_no_refine_when_already_refined() {
        let mut z = z();
        z.purity = 100.0;
        z.tick(1.0);
        assert!(!z.just_refined);
    }

    #[test]
    fn tick_no_refine_when_rate_zero() {
        let mut z = Zirconium::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.purity, 0.0);
    }

    #[test]
    fn tick_no_refine_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.purity, 0.0);
    }

    #[test]
    fn tick_clears_just_refined() {
        let mut z = Zirconium::new(100.0, 200.0);
        z.purity = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_refined);
    }

    #[test]
    fn tick_clears_just_tarnished() {
        let mut z = z();
        z.purity = 10.0;
        z.corrode(10.0);
        z.tick(0.016);
        assert!(!z.just_tarnished);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.purity - 9.0).abs() < 1e-3);
    }

    // --- is_refined / is_tarnished ---

    #[test]
    fn is_refined_false_when_disabled() {
        let mut z = z();
        z.purity = 100.0;
        z.enabled = false;
        assert!(!z.is_refined());
    }

    #[test]
    fn is_tarnished_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_tarnished());
    }

    // --- purity_fraction / effective_shielding ---

    #[test]
    fn purity_fraction_zero_when_tarnished() {
        assert_eq!(z().purity_fraction(), 0.0);
    }

    #[test]
    fn purity_fraction_half_at_midpoint() {
        let mut z = z();
        z.purity = 50.0;
        assert!((z.purity_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_shielding_zero_when_tarnished() {
        assert_eq!(z().effective_shielding(100.0), 0.0);
    }

    #[test]
    fn effective_shielding_scales_with_purity() {
        let mut z = z();
        z.purity = 75.0;
        assert!((z.effective_shielding(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_shielding_zero_when_disabled() {
        let mut z = z();
        z.purity = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_shielding(100.0), 0.0);
    }
}
