use bevy_ecs::prelude::Component;

/// Charge-separation tracker named after the zwitterion — a molecule
/// that simultaneously carries both a positive and a negative charge
/// on different sites, resulting in net-zero overall charge but a
/// strong internal electric dipole. `charge` builds via
/// `separate(amount)` and increases passively at `polarize_rate`
/// per second in `tick(dt)` or collapses via `recombine(amount)`.
///
/// Models molecular-dipole-moment fill levels, electrostatic-
/// polarization intensity bars, amino-acid charge-separation
/// gauges, pH-dependent ionization trackers, zwitterionic-
/// surfactant efficiency meters, protein-folding electrostatic-
/// energy accumulators, amphoteric-compound charge-state trackers,
/// ion-pair formation bars, betaine-type charge-build-up gauges,
/// or any mechanic where equal and opposite charges pile up on the
/// same molecule and tear it in two directions at once, creating
/// a perfect internal tension that drives everything else in the
/// system by sheer polarity — until something forces the charges
/// to recombine and the tension collapses back to neutral.
///
/// `separate(amount)` adds charge-separation; fires `just_polarized`
/// when first reaching `max_charge`. No-op when disabled.
///
/// `recombine(amount)` reduces charge-separation immediately; fires
/// `just_neutral` when reaching 0. No-op when disabled or already
/// neutral.
///
/// `tick(dt)` clears both flags, then increases charge by
/// `polarize_rate * dt` (capped at `max_charge`). Fires
/// `just_polarized` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_polarized()` returns `charge >= max_charge && enabled`.
///
/// `is_neutral()` returns `charge == 0.0` (not gated by `enabled`).
///
/// `charge_fraction()` returns `(charge / max_charge).clamp(0, 1)`.
///
/// `effective_dipole(scale)` returns `scale * charge_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — polarizes at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zwitterion {
    pub charge: f32,
    pub max_charge: f32,
    pub polarize_rate: f32,
    pub just_polarized: bool,
    pub just_neutral: bool,
    pub enabled: bool,
}

impl Zwitterion {
    pub fn new(max_charge: f32, polarize_rate: f32) -> Self {
        Self {
            charge: 0.0,
            max_charge: max_charge.max(0.1),
            polarize_rate: polarize_rate.max(0.0),
            just_polarized: false,
            just_neutral: false,
            enabled: true,
        }
    }

    /// Add charge-separation; fires `just_polarized` when first reaching max.
    /// No-op when disabled.
    pub fn separate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.charge < self.max_charge;
        self.charge = (self.charge + amount).min(self.max_charge);
        if was_below && self.charge >= self.max_charge {
            self.just_polarized = true;
        }
    }

    /// Reduce charge-separation; fires `just_neutral` when reaching 0.
    /// No-op when disabled or already neutral.
    pub fn recombine(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.charge <= 0.0 {
            return;
        }
        self.charge = (self.charge - amount).max(0.0);
        if self.charge <= 0.0 {
            self.just_neutral = true;
        }
    }

    /// Clear flags, then increase charge by `polarize_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_polarized = false;
        self.just_neutral = false;
        if self.enabled && self.polarize_rate > 0.0 && self.charge < self.max_charge {
            let was_below = self.charge < self.max_charge;
            self.charge = (self.charge + self.polarize_rate * dt).min(self.max_charge);
            if was_below && self.charge >= self.max_charge {
                self.just_polarized = true;
            }
        }
    }

    /// `true` when charge is at maximum and component is enabled.
    pub fn is_polarized(&self) -> bool {
        self.charge >= self.max_charge && self.enabled
    }

    /// `true` when charge is 0 (not gated by `enabled`).
    pub fn is_neutral(&self) -> bool {
        self.charge == 0.0
    }

    /// Fraction of maximum charge [0.0, 1.0].
    pub fn charge_fraction(&self) -> f32 {
        (self.charge / self.max_charge).clamp(0.0, 1.0)
    }

    /// Returns `scale * charge_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_dipole(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.charge_fraction()
    }
}

impl Default for Zwitterion {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zwitterion {
        Zwitterion::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_neutral() {
        let z = z();
        assert_eq!(z.charge, 0.0);
        assert!(z.is_neutral());
        assert!(!z.is_polarized());
    }

    #[test]
    fn new_clamps_max_charge() {
        let z = Zwitterion::new(-5.0, 1.5);
        assert!((z.max_charge - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_polarize_rate() {
        let z = Zwitterion::new(100.0, -1.5);
        assert_eq!(z.polarize_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zwitterion::default();
        assert!((z.max_charge - 100.0).abs() < 1e-5);
        assert!((z.polarize_rate - 1.5).abs() < 1e-5);
    }

    // --- separate ---

    #[test]
    fn separate_adds_charge() {
        let mut z = z();
        z.separate(40.0);
        assert!((z.charge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn separate_clamps_at_max() {
        let mut z = z();
        z.separate(200.0);
        assert!((z.charge - 100.0).abs() < 1e-3);
    }

    #[test]
    fn separate_fires_just_polarized_at_max() {
        let mut z = z();
        z.separate(100.0);
        assert!(z.just_polarized);
        assert!(z.is_polarized());
    }

    #[test]
    fn separate_no_just_polarized_when_already_at_max() {
        let mut z = z();
        z.charge = 100.0;
        z.separate(10.0);
        assert!(!z.just_polarized);
    }

    #[test]
    fn separate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.separate(50.0);
        assert_eq!(z.charge, 0.0);
    }

    #[test]
    fn separate_no_op_when_amount_zero() {
        let mut z = z();
        z.separate(0.0);
        assert_eq!(z.charge, 0.0);
    }

    // --- recombine ---

    #[test]
    fn recombine_reduces_charge() {
        let mut z = z();
        z.charge = 60.0;
        z.recombine(20.0);
        assert!((z.charge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn recombine_clamps_at_zero() {
        let mut z = z();
        z.charge = 30.0;
        z.recombine(200.0);
        assert_eq!(z.charge, 0.0);
    }

    #[test]
    fn recombine_fires_just_neutral_at_zero() {
        let mut z = z();
        z.charge = 30.0;
        z.recombine(30.0);
        assert!(z.just_neutral);
    }

    #[test]
    fn recombine_no_op_when_already_neutral() {
        let mut z = z();
        z.recombine(10.0);
        assert!(!z.just_neutral);
    }

    #[test]
    fn recombine_no_op_when_disabled() {
        let mut z = z();
        z.charge = 50.0;
        z.enabled = false;
        z.recombine(50.0);
        assert!((z.charge - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_polarizes_charge() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.charge - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_polarized_on_polarize_to_max() {
        let mut z = Zwitterion::new(100.0, 200.0);
        z.charge = 95.0;
        z.tick(1.0);
        assert!(z.just_polarized);
        assert!(z.is_polarized());
    }

    #[test]
    fn tick_no_polarize_when_already_polarized() {
        let mut z = z();
        z.charge = 100.0;
        z.tick(1.0);
        assert!(!z.just_polarized);
    }

    #[test]
    fn tick_no_polarize_when_rate_zero() {
        let mut z = Zwitterion::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.charge, 0.0);
    }

    #[test]
    fn tick_no_polarize_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.charge, 0.0);
    }

    #[test]
    fn tick_clears_just_polarized() {
        let mut z = Zwitterion::new(100.0, 200.0);
        z.charge = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_polarized);
    }

    #[test]
    fn tick_clears_just_neutral() {
        let mut z = z();
        z.charge = 10.0;
        z.recombine(10.0);
        z.tick(0.016);
        assert!(!z.just_neutral);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.charge - 9.0).abs() < 1e-3);
    }

    // --- is_polarized / is_neutral ---

    #[test]
    fn is_polarized_false_when_disabled() {
        let mut z = z();
        z.charge = 100.0;
        z.enabled = false;
        assert!(!z.is_polarized());
    }

    #[test]
    fn is_neutral_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_neutral());
    }

    // --- charge_fraction / effective_dipole ---

    #[test]
    fn charge_fraction_zero_when_neutral() {
        assert_eq!(z().charge_fraction(), 0.0);
    }

    #[test]
    fn charge_fraction_half_at_midpoint() {
        let mut z = z();
        z.charge = 50.0;
        assert!((z.charge_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_dipole_zero_when_neutral() {
        assert_eq!(z().effective_dipole(100.0), 0.0);
    }

    #[test]
    fn effective_dipole_scales_with_charge() {
        let mut z = z();
        z.charge = 75.0;
        assert!((z.effective_dipole(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_dipole_zero_when_disabled() {
        let mut z = z();
        z.charge = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_dipole(100.0), 0.0);
    }
}
