use bevy_ecs::prelude::Component;

/// Crystal-growth tracker named after the zinc-oxide mineral (ZnO).
/// `saturation` builds via `crystallize(amount)` and deposits
/// passively at `deposit_rate` per second in `tick(dt)` or is
/// dissolved immediately via `dissolve(amount)`.
///
/// Models zinc-oxide crystal saturation bars, mineral-deposition
/// fill levels, hydrothermal-vein crystal-growth gauges, ore-
/// precipitation progress trackers, metallurgical-oxidation bars,
/// ceramic-glaze development indicators, chemical-vapor-deposition
/// thickness meters, piezoelectric-crystal quality accumulators,
/// thin-film crystal-layer build-up bars, or any mechanic where
/// a supersaturated solution teases needle-thin hexagonal crystals
/// from the chemical chaos — blood-red if manganese-doped, yellow
/// if iron-tinted, pure white if uncontaminated — and deposits
/// them one by one into columns and clusters until a wall of tiny
/// oxidised pillars has formed on the substrate and the vein is
/// too full to precipitate another atom.
///
/// `crystallize(amount)` adds saturation; fires `just_crystallized`
/// when first reaching `max_saturation`. No-op when disabled.
///
/// `dissolve(amount)` reduces saturation immediately; fires
/// `just_depleted` when reaching 0. No-op when disabled or
/// already depleted.
///
/// `tick(dt)` clears both flags, then increases saturation by
/// `deposit_rate * dt` (capped at `max_saturation`). Fires
/// `just_crystallized` when first reaching max. No-op when
/// disabled or rate is 0.
///
/// `is_crystallized()` returns `saturation >= max_saturation && enabled`.
///
/// `is_depleted()` returns `saturation == 0.0` (not gated by `enabled`).
///
/// `saturation_fraction()` returns `(saturation / max_saturation).clamp(0, 1)`.
///
/// `effective_purity(scale)` returns `scale * saturation_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — deposits at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zincite {
    pub saturation: f32,
    pub max_saturation: f32,
    pub deposit_rate: f32,
    pub just_crystallized: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Zincite {
    pub fn new(max_saturation: f32, deposit_rate: f32) -> Self {
        Self {
            saturation: 0.0,
            max_saturation: max_saturation.max(0.1),
            deposit_rate: deposit_rate.max(0.0),
            just_crystallized: false,
            just_depleted: false,
            enabled: true,
        }
    }

    /// Add saturation; fires `just_crystallized` when first reaching max.
    /// No-op when disabled.
    pub fn crystallize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.saturation < self.max_saturation;
        self.saturation = (self.saturation + amount).min(self.max_saturation);
        if was_below && self.saturation >= self.max_saturation {
            self.just_crystallized = true;
        }
    }

    /// Reduce saturation; fires `just_depleted` when reaching 0.
    /// No-op when disabled or already depleted.
    pub fn dissolve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.saturation <= 0.0 {
            return;
        }
        self.saturation = (self.saturation - amount).max(0.0);
        if self.saturation <= 0.0 {
            self.just_depleted = true;
        }
    }

    /// Clear flags, then increase saturation by `deposit_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_crystallized = false;
        self.just_depleted = false;
        if self.enabled && self.deposit_rate > 0.0 && self.saturation < self.max_saturation {
            let was_below = self.saturation < self.max_saturation;
            self.saturation = (self.saturation + self.deposit_rate * dt).min(self.max_saturation);
            if was_below && self.saturation >= self.max_saturation {
                self.just_crystallized = true;
            }
        }
    }

    /// `true` when saturation is at maximum and component is enabled.
    pub fn is_crystallized(&self) -> bool {
        self.saturation >= self.max_saturation && self.enabled
    }

    /// `true` when saturation is 0 (not gated by `enabled`).
    pub fn is_depleted(&self) -> bool {
        self.saturation == 0.0
    }

    /// Fraction of maximum saturation [0.0, 1.0].
    pub fn saturation_fraction(&self) -> f32 {
        (self.saturation / self.max_saturation).clamp(0.0, 1.0)
    }

    /// Returns `scale * saturation_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_purity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.saturation_fraction()
    }
}

impl Default for Zincite {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zincite {
        Zincite::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_depleted() {
        let z = z();
        assert_eq!(z.saturation, 0.0);
        assert!(z.is_depleted());
        assert!(!z.is_crystallized());
    }

    #[test]
    fn new_clamps_max_saturation() {
        let z = Zincite::new(-5.0, 1.5);
        assert!((z.max_saturation - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_deposit_rate() {
        let z = Zincite::new(100.0, -1.5);
        assert_eq!(z.deposit_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zincite::default();
        assert!((z.max_saturation - 100.0).abs() < 1e-5);
        assert!((z.deposit_rate - 1.5).abs() < 1e-5);
    }

    // --- crystallize ---

    #[test]
    fn crystallize_adds_saturation() {
        let mut z = z();
        z.crystallize(40.0);
        assert!((z.saturation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn crystallize_clamps_at_max() {
        let mut z = z();
        z.crystallize(200.0);
        assert!((z.saturation - 100.0).abs() < 1e-3);
    }

    #[test]
    fn crystallize_fires_just_crystallized_at_max() {
        let mut z = z();
        z.crystallize(100.0);
        assert!(z.just_crystallized);
        assert!(z.is_crystallized());
    }

    #[test]
    fn crystallize_no_just_crystallized_when_already_at_max() {
        let mut z = z();
        z.saturation = 100.0;
        z.crystallize(10.0);
        assert!(!z.just_crystallized);
    }

    #[test]
    fn crystallize_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.crystallize(50.0);
        assert_eq!(z.saturation, 0.0);
    }

    #[test]
    fn crystallize_no_op_when_amount_zero() {
        let mut z = z();
        z.crystallize(0.0);
        assert_eq!(z.saturation, 0.0);
    }

    // --- dissolve ---

    #[test]
    fn dissolve_reduces_saturation() {
        let mut z = z();
        z.saturation = 60.0;
        z.dissolve(20.0);
        assert!((z.saturation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dissolve_clamps_at_zero() {
        let mut z = z();
        z.saturation = 30.0;
        z.dissolve(200.0);
        assert_eq!(z.saturation, 0.0);
    }

    #[test]
    fn dissolve_fires_just_depleted_at_zero() {
        let mut z = z();
        z.saturation = 30.0;
        z.dissolve(30.0);
        assert!(z.just_depleted);
    }

    #[test]
    fn dissolve_no_op_when_already_depleted() {
        let mut z = z();
        z.dissolve(10.0);
        assert!(!z.just_depleted);
    }

    #[test]
    fn dissolve_no_op_when_disabled() {
        let mut z = z();
        z.saturation = 50.0;
        z.enabled = false;
        z.dissolve(50.0);
        assert!((z.saturation - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_deposits_saturation() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.saturation - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_crystallized_on_deposit_to_max() {
        let mut z = Zincite::new(100.0, 200.0);
        z.saturation = 95.0;
        z.tick(1.0);
        assert!(z.just_crystallized);
        assert!(z.is_crystallized());
    }

    #[test]
    fn tick_no_deposit_when_already_crystallized() {
        let mut z = z();
        z.saturation = 100.0;
        z.tick(1.0);
        assert!(!z.just_crystallized);
    }

    #[test]
    fn tick_no_deposit_when_rate_zero() {
        let mut z = Zincite::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.saturation, 0.0);
    }

    #[test]
    fn tick_no_deposit_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.saturation, 0.0);
    }

    #[test]
    fn tick_clears_just_crystallized() {
        let mut z = Zincite::new(100.0, 200.0);
        z.saturation = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_crystallized);
    }

    #[test]
    fn tick_clears_just_depleted() {
        let mut z = z();
        z.saturation = 10.0;
        z.dissolve(10.0);
        z.tick(0.016);
        assert!(!z.just_depleted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.saturation - 9.0).abs() < 1e-3);
    }

    // --- is_crystallized / is_depleted ---

    #[test]
    fn is_crystallized_false_when_disabled() {
        let mut z = z();
        z.saturation = 100.0;
        z.enabled = false;
        assert!(!z.is_crystallized());
    }

    #[test]
    fn is_depleted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_depleted());
    }

    // --- saturation_fraction / effective_purity ---

    #[test]
    fn saturation_fraction_zero_when_depleted() {
        assert_eq!(z().saturation_fraction(), 0.0);
    }

    #[test]
    fn saturation_fraction_half_at_midpoint() {
        let mut z = z();
        z.saturation = 50.0;
        assert!((z.saturation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_purity_zero_when_depleted() {
        assert_eq!(z().effective_purity(100.0), 0.0);
    }

    #[test]
    fn effective_purity_scales_with_saturation() {
        let mut z = z();
        z.saturation = 75.0;
        assert!((z.effective_purity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_purity_zero_when_disabled() {
        let mut z = z();
        z.saturation = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_purity(100.0), 0.0);
    }
}
