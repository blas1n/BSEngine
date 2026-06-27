use bevy_ecs::prelude::Component;

/// Electroplating-bath concentration tracker named after zincate
/// (ZnO₂²⁻), the oxyanion formed when zinc dissolves in strong
/// alkaline solution to give salts such as sodium zincate
/// (Na₂ZnO₂) and potassium zincate (K₂ZnO₂). Zincate baths are
/// the working electrolyte in alkaline zinc-plating lines, hot-dip
/// galvanizing flux operations, zinc-alloy electrodeposition cells,
/// and conversion-coating pre-treatment stages used before painting.
/// `concentration` builds via `plate(amount)` and increases
/// passively at `deposit_rate` per second in `tick(dt)` or is
/// reduced via `rinse(amount)`.
///
/// Models alkaline zinc-plating bath concentration bars, zincate-
/// bath replenishment fill levels, metal pre-treatment bath
/// saturation gauges, galvanizing flux-bath strength trackers,
/// electrodeposition-cell ion-concentration meters, corrosion-
/// resistant coating build-up indicators, ceramic-glaze alkaline-
/// zinc-compound saturation bars, industrial-metal-finishing bath
/// chemistry trackers, or any mechanic where carefully maintaining
/// the zincate concentration in an alkaline plating bath determines
/// whether metal parts emerge with a bright, adherent zinc coating
/// or a powdery, poorly bonded one that flakes off the moment
/// the part leaves the rinse tank.
///
/// `plate(amount)` adds concentration; fires `just_saturated` when
/// first reaching `max_concentration`. No-op when disabled.
///
/// `rinse(amount)` reduces concentration immediately; fires
/// `just_depleted` when reaching 0. No-op when disabled or
/// already depleted.
///
/// `tick(dt)` clears both flags, then increases concentration by
/// `deposit_rate * dt` (capped at `max_concentration`). Fires
/// `just_saturated` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_saturated()` returns `concentration >= max_concentration && enabled`.
///
/// `is_depleted()` returns `concentration == 0.0` (not gated by `enabled`).
///
/// `concentration_fraction()` returns `(concentration / max_concentration).clamp(0, 1)`.
///
/// `effective_adhesion(scale)` returns `scale * concentration_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — deposits at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zincate {
    pub concentration: f32,
    pub max_concentration: f32,
    pub deposit_rate: f32,
    pub just_saturated: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Zincate {
    pub fn new(max_concentration: f32, deposit_rate: f32) -> Self {
        Self {
            concentration: 0.0,
            max_concentration: max_concentration.max(0.1),
            deposit_rate: deposit_rate.max(0.0),
            just_saturated: false,
            just_depleted: false,
            enabled: true,
        }
    }

    /// Add concentration; fires `just_saturated` when first reaching max.
    /// No-op when disabled.
    pub fn plate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.concentration < self.max_concentration;
        self.concentration = (self.concentration + amount).min(self.max_concentration);
        if was_below && self.concentration >= self.max_concentration {
            self.just_saturated = true;
        }
    }

    /// Reduce concentration; fires `just_depleted` when reaching 0.
    /// No-op when disabled or already depleted.
    pub fn rinse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.concentration <= 0.0 {
            return;
        }
        self.concentration = (self.concentration - amount).max(0.0);
        if self.concentration <= 0.0 {
            self.just_depleted = true;
        }
    }

    /// Clear flags, then increase concentration by `deposit_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_saturated = false;
        self.just_depleted = false;
        if self.enabled && self.deposit_rate > 0.0 && self.concentration < self.max_concentration {
            let was_below = self.concentration < self.max_concentration;
            self.concentration =
                (self.concentration + self.deposit_rate * dt).min(self.max_concentration);
            if was_below && self.concentration >= self.max_concentration {
                self.just_saturated = true;
            }
        }
    }

    /// `true` when concentration is at maximum and component is enabled.
    pub fn is_saturated(&self) -> bool {
        self.concentration >= self.max_concentration && self.enabled
    }

    /// `true` when concentration is 0 (not gated by `enabled`).
    pub fn is_depleted(&self) -> bool {
        self.concentration == 0.0
    }

    /// Fraction of maximum concentration [0.0, 1.0].
    pub fn concentration_fraction(&self) -> f32 {
        (self.concentration / self.max_concentration).clamp(0.0, 1.0)
    }

    /// Returns `scale * concentration_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_adhesion(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.concentration_fraction()
    }
}

impl Default for Zincate {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zincate {
        Zincate::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_depleted() {
        let z = z();
        assert_eq!(z.concentration, 0.0);
        assert!(z.is_depleted());
        assert!(!z.is_saturated());
    }

    #[test]
    fn new_clamps_max_concentration() {
        let z = Zincate::new(-5.0, 1.5);
        assert!((z.max_concentration - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_deposit_rate() {
        let z = Zincate::new(100.0, -1.5);
        assert_eq!(z.deposit_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zincate::default();
        assert!((z.max_concentration - 100.0).abs() < 1e-5);
        assert!((z.deposit_rate - 1.5).abs() < 1e-5);
    }

    // --- plate ---

    #[test]
    fn plate_adds_concentration() {
        let mut z = z();
        z.plate(40.0);
        assert!((z.concentration - 40.0).abs() < 1e-3);
    }

    #[test]
    fn plate_clamps_at_max() {
        let mut z = z();
        z.plate(200.0);
        assert!((z.concentration - 100.0).abs() < 1e-3);
    }

    #[test]
    fn plate_fires_just_saturated_at_max() {
        let mut z = z();
        z.plate(100.0);
        assert!(z.just_saturated);
        assert!(z.is_saturated());
    }

    #[test]
    fn plate_no_just_saturated_when_already_at_max() {
        let mut z = z();
        z.concentration = 100.0;
        z.plate(10.0);
        assert!(!z.just_saturated);
    }

    #[test]
    fn plate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.plate(50.0);
        assert_eq!(z.concentration, 0.0);
    }

    #[test]
    fn plate_no_op_when_amount_zero() {
        let mut z = z();
        z.plate(0.0);
        assert_eq!(z.concentration, 0.0);
    }

    // --- rinse ---

    #[test]
    fn rinse_reduces_concentration() {
        let mut z = z();
        z.concentration = 60.0;
        z.rinse(20.0);
        assert!((z.concentration - 40.0).abs() < 1e-3);
    }

    #[test]
    fn rinse_clamps_at_zero() {
        let mut z = z();
        z.concentration = 30.0;
        z.rinse(200.0);
        assert_eq!(z.concentration, 0.0);
    }

    #[test]
    fn rinse_fires_just_depleted_at_zero() {
        let mut z = z();
        z.concentration = 30.0;
        z.rinse(30.0);
        assert!(z.just_depleted);
    }

    #[test]
    fn rinse_no_op_when_already_depleted() {
        let mut z = z();
        z.rinse(10.0);
        assert!(!z.just_depleted);
    }

    #[test]
    fn rinse_no_op_when_disabled() {
        let mut z = z();
        z.concentration = 50.0;
        z.enabled = false;
        z.rinse(50.0);
        assert!((z.concentration - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_deposits_concentration() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.concentration - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_saturated_on_deposit_to_max() {
        let mut z = Zincate::new(100.0, 200.0);
        z.concentration = 95.0;
        z.tick(1.0);
        assert!(z.just_saturated);
        assert!(z.is_saturated());
    }

    #[test]
    fn tick_no_deposit_when_already_saturated() {
        let mut z = z();
        z.concentration = 100.0;
        z.tick(1.0);
        assert!(!z.just_saturated);
    }

    #[test]
    fn tick_no_deposit_when_rate_zero() {
        let mut z = Zincate::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.concentration, 0.0);
    }

    #[test]
    fn tick_no_deposit_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.concentration, 0.0);
    }

    #[test]
    fn tick_clears_just_saturated() {
        let mut z = Zincate::new(100.0, 200.0);
        z.concentration = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_saturated);
    }

    #[test]
    fn tick_clears_just_depleted() {
        let mut z = z();
        z.concentration = 10.0;
        z.rinse(10.0);
        z.tick(0.016);
        assert!(!z.just_depleted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.concentration - 9.0).abs() < 1e-3);
    }

    // --- is_saturated / is_depleted ---

    #[test]
    fn is_saturated_false_when_disabled() {
        let mut z = z();
        z.concentration = 100.0;
        z.enabled = false;
        assert!(!z.is_saturated());
    }

    #[test]
    fn is_depleted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_depleted());
    }

    // --- concentration_fraction / effective_adhesion ---

    #[test]
    fn concentration_fraction_zero_when_depleted() {
        assert_eq!(z().concentration_fraction(), 0.0);
    }

    #[test]
    fn concentration_fraction_half_at_midpoint() {
        let mut z = z();
        z.concentration = 50.0;
        assert!((z.concentration_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_adhesion_zero_when_depleted() {
        assert_eq!(z().effective_adhesion(100.0), 0.0);
    }

    #[test]
    fn effective_adhesion_scales_with_concentration() {
        let mut z = z();
        z.concentration = 75.0;
        assert!((z.effective_adhesion(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_adhesion_zero_when_disabled() {
        let mut z = z();
        z.concentration = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_adhesion(100.0), 0.0);
    }
}
