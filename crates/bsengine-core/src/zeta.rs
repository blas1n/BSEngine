use bevy_ecs::prelude::Component;

/// Electrostatic-potential tracker. `potential` builds via `charge(amount)`
/// and accumulates passively at `flux_rate` per second in `tick(dt)` or
/// discharges immediately via `discharge(amount)`.
///
/// Models particle-physics zeta-potential gauges, electrostatic surface-charge
/// build-up meters, colloidal-stability fill levels, ion-layer accumulation
/// bars, static-electricity charge trackers, plasma-field intensity gauges,
/// capacitor-charge progress indicators, or any mechanic where an electric
/// surface potential builds toward a critical threshold.
///
/// `charge(amount)` adds potential; fires `just_critical` when first
/// reaching `max_potential`. No-op when disabled.
///
/// `discharge(amount)` reduces potential immediately; fires `just_neutral`
/// when reaching 0. No-op when disabled or already neutral.
///
/// `tick(dt)` clears both flags, then increases potential by
/// `flux_rate * dt` (capped at `max_potential`). Fires `just_critical`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_critical()` returns `potential >= max_potential && enabled`.
///
/// `is_neutral()` returns `potential == 0.0` (not gated by `enabled`).
///
/// `potential_fraction()` returns `(potential / max_potential).clamp(0, 1)`.
///
/// `effective_field(scale)` returns `scale * potential_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 7.0)` — accumulates at 7 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeta {
    pub potential: f32,
    pub max_potential: f32,
    pub flux_rate: f32,
    pub just_critical: bool,
    pub just_neutral: bool,
    pub enabled: bool,
}

impl Zeta {
    pub fn new(max_potential: f32, flux_rate: f32) -> Self {
        Self {
            potential: 0.0,
            max_potential: max_potential.max(0.1),
            flux_rate: flux_rate.max(0.0),
            just_critical: false,
            just_neutral: false,
            enabled: true,
        }
    }

    /// Add potential; fires `just_critical` when first reaching max.
    /// No-op when disabled.
    pub fn charge(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.potential < self.max_potential;
        self.potential = (self.potential + amount).min(self.max_potential);
        if was_below && self.potential >= self.max_potential {
            self.just_critical = true;
        }
    }

    /// Reduce potential; fires `just_neutral` when reaching 0.
    /// No-op when disabled or already neutral.
    pub fn discharge(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.potential <= 0.0 {
            return;
        }
        self.potential = (self.potential - amount).max(0.0);
        if self.potential <= 0.0 {
            self.just_neutral = true;
        }
    }

    /// Clear flags, then increase potential by `flux_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_critical = false;
        self.just_neutral = false;
        if self.enabled && self.flux_rate > 0.0 && self.potential < self.max_potential {
            let was_below = self.potential < self.max_potential;
            self.potential = (self.potential + self.flux_rate * dt).min(self.max_potential);
            if was_below && self.potential >= self.max_potential {
                self.just_critical = true;
            }
        }
    }

    /// `true` when potential is at maximum and component is enabled.
    pub fn is_critical(&self) -> bool {
        self.potential >= self.max_potential && self.enabled
    }

    /// `true` when potential is 0 (not gated by `enabled`).
    pub fn is_neutral(&self) -> bool {
        self.potential == 0.0
    }

    /// Fraction of maximum potential [0.0, 1.0].
    pub fn potential_fraction(&self) -> f32 {
        (self.potential / self.max_potential).clamp(0.0, 1.0)
    }

    /// Returns `scale * potential_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_field(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.potential_fraction()
    }
}

impl Default for Zeta {
    fn default() -> Self {
        Self::new(100.0, 7.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeta {
        Zeta::new(100.0, 7.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_neutral() {
        let z = z();
        assert_eq!(z.potential, 0.0);
        assert!(z.is_neutral());
        assert!(!z.is_critical());
    }

    #[test]
    fn new_clamps_max_potential() {
        let z = Zeta::new(-5.0, 7.0);
        assert!((z.max_potential - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_flux_rate() {
        let z = Zeta::new(100.0, -3.0);
        assert_eq!(z.flux_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeta::default();
        assert!((z.max_potential - 100.0).abs() < 1e-5);
        assert!((z.flux_rate - 7.0).abs() < 1e-5);
    }

    // --- charge ---

    #[test]
    fn charge_adds_potential() {
        let mut z = z();
        z.charge(40.0);
        assert!((z.potential - 40.0).abs() < 1e-3);
    }

    #[test]
    fn charge_clamps_at_max() {
        let mut z = z();
        z.charge(200.0);
        assert!((z.potential - 100.0).abs() < 1e-3);
    }

    #[test]
    fn charge_fires_just_critical_at_max() {
        let mut z = z();
        z.charge(100.0);
        assert!(z.just_critical);
        assert!(z.is_critical());
    }

    #[test]
    fn charge_no_just_critical_when_already_at_max() {
        let mut z = z();
        z.potential = 100.0;
        z.charge(10.0);
        assert!(!z.just_critical);
    }

    #[test]
    fn charge_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.charge(50.0);
        assert_eq!(z.potential, 0.0);
    }

    #[test]
    fn charge_no_op_when_amount_zero() {
        let mut z = z();
        z.charge(0.0);
        assert_eq!(z.potential, 0.0);
    }

    // --- discharge ---

    #[test]
    fn discharge_reduces_potential() {
        let mut z = z();
        z.potential = 60.0;
        z.discharge(20.0);
        assert!((z.potential - 40.0).abs() < 1e-3);
    }

    #[test]
    fn discharge_clamps_at_zero() {
        let mut z = z();
        z.potential = 30.0;
        z.discharge(200.0);
        assert_eq!(z.potential, 0.0);
    }

    #[test]
    fn discharge_fires_just_neutral_at_zero() {
        let mut z = z();
        z.potential = 30.0;
        z.discharge(30.0);
        assert!(z.just_neutral);
    }

    #[test]
    fn discharge_no_op_when_already_neutral() {
        let mut z = z();
        z.discharge(10.0);
        assert!(!z.just_neutral);
    }

    #[test]
    fn discharge_no_op_when_disabled() {
        let mut z = z();
        z.potential = 50.0;
        z.enabled = false;
        z.discharge(50.0);
        assert!((z.potential - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_accumulates_potential() {
        let mut z = z(); // rate=7
        z.tick(1.0); // 0 + 7 = 7
        assert!((z.potential - 7.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_critical_on_flux_to_max() {
        let mut z = Zeta::new(100.0, 200.0);
        z.potential = 95.0;
        z.tick(1.0);
        assert!(z.just_critical);
        assert!(z.is_critical());
    }

    #[test]
    fn tick_no_flux_when_already_critical() {
        let mut z = z();
        z.potential = 100.0;
        z.tick(1.0);
        assert!(!z.just_critical);
    }

    #[test]
    fn tick_no_flux_when_rate_zero() {
        let mut z = Zeta::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.potential, 0.0);
    }

    #[test]
    fn tick_no_flux_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.potential, 0.0);
    }

    #[test]
    fn tick_clears_just_critical() {
        let mut z = Zeta::new(100.0, 200.0);
        z.potential = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_critical);
    }

    #[test]
    fn tick_clears_just_neutral() {
        let mut z = z();
        z.potential = 10.0;
        z.discharge(10.0);
        z.tick(0.016);
        assert!(!z.just_neutral);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=7
        z.tick(3.0); // 7*3 = 21
        assert!((z.potential - 21.0).abs() < 1e-3);
    }

    // --- is_critical / is_neutral ---

    #[test]
    fn is_critical_false_when_disabled() {
        let mut z = z();
        z.potential = 100.0;
        z.enabled = false;
        assert!(!z.is_critical());
    }

    #[test]
    fn is_neutral_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_neutral());
    }

    // --- potential_fraction / effective_field ---

    #[test]
    fn potential_fraction_zero_when_neutral() {
        assert_eq!(z().potential_fraction(), 0.0);
    }

    #[test]
    fn potential_fraction_half_at_midpoint() {
        let mut z = z();
        z.potential = 50.0;
        assert!((z.potential_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_field_zero_when_neutral() {
        assert_eq!(z().effective_field(100.0), 0.0);
    }

    #[test]
    fn effective_field_scales_with_potential() {
        let mut z = z();
        z.potential = 70.0;
        assert!((z.effective_field(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_field_zero_when_disabled() {
        let mut z = z();
        z.potential = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_field(100.0), 0.0);
    }
}
