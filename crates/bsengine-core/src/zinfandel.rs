use bevy_ecs::prelude::Component;

/// Fermentation-progress tracker. `ferment` builds via `brew(amount)` and
/// matures passively at `mature_rate` per second in `tick(dt)` or is
/// racked immediately via `rack(amount)`.
///
/// Models wine/cider brewing meters, alchemy-fermentation progress bars,
/// potion-aging trackers, yeast-culture fill levels, fermentation-tank
/// charge gauges, barrel-maturation timers, kombucha-readiness indicators,
/// or any mechanic where a substance must accumulate complexity over time
/// before reaching peak potency.
///
/// `brew(amount)` adds ferment; fires `just_matured` when first reaching
/// `max_ferment`. No-op when disabled.
///
/// `rack(amount)` reduces ferment immediately; fires `just_racked` when
/// reaching 0. No-op when disabled or already racked.
///
/// `tick(dt)` clears both flags, then increases ferment by
/// `mature_rate * dt` (capped at `max_ferment`). Fires `just_matured`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_matured()` returns `ferment >= max_ferment && enabled`.
///
/// `is_racked()` returns `ferment == 0.0` (not gated by `enabled`).
///
/// `ferment_fraction()` returns `(ferment / max_ferment).clamp(0, 1)`.
///
/// `effective_vintage(scale)` returns `scale * ferment_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — matures at 5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zinfandel {
    pub ferment: f32,
    pub max_ferment: f32,
    pub mature_rate: f32,
    pub just_matured: bool,
    pub just_racked: bool,
    pub enabled: bool,
}

impl Zinfandel {
    pub fn new(max_ferment: f32, mature_rate: f32) -> Self {
        Self {
            ferment: 0.0,
            max_ferment: max_ferment.max(0.1),
            mature_rate: mature_rate.max(0.0),
            just_matured: false,
            just_racked: false,
            enabled: true,
        }
    }

    /// Add ferment; fires `just_matured` when first reaching max.
    /// No-op when disabled.
    pub fn brew(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.ferment < self.max_ferment;
        self.ferment = (self.ferment + amount).min(self.max_ferment);
        if was_below && self.ferment >= self.max_ferment {
            self.just_matured = true;
        }
    }

    /// Reduce ferment; fires `just_racked` when reaching 0.
    /// No-op when disabled or already racked.
    pub fn rack(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.ferment <= 0.0 {
            return;
        }
        self.ferment = (self.ferment - amount).max(0.0);
        if self.ferment <= 0.0 {
            self.just_racked = true;
        }
    }

    /// Clear flags, then increase ferment by `mature_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_matured = false;
        self.just_racked = false;
        if self.enabled && self.mature_rate > 0.0 && self.ferment < self.max_ferment {
            let was_below = self.ferment < self.max_ferment;
            self.ferment = (self.ferment + self.mature_rate * dt).min(self.max_ferment);
            if was_below && self.ferment >= self.max_ferment {
                self.just_matured = true;
            }
        }
    }

    /// `true` when ferment is at maximum and component is enabled.
    pub fn is_matured(&self) -> bool {
        self.ferment >= self.max_ferment && self.enabled
    }

    /// `true` when ferment is 0 (not gated by `enabled`).
    pub fn is_racked(&self) -> bool {
        self.ferment == 0.0
    }

    /// Fraction of maximum ferment [0.0, 1.0].
    pub fn ferment_fraction(&self) -> f32 {
        (self.ferment / self.max_ferment).clamp(0.0, 1.0)
    }

    /// Returns `scale * ferment_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vintage(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.ferment_fraction()
    }
}

impl Default for Zinfandel {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zinfandel {
        Zinfandel::new(100.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_racked() {
        let z = z();
        assert_eq!(z.ferment, 0.0);
        assert!(z.is_racked());
        assert!(!z.is_matured());
    }

    #[test]
    fn new_clamps_max_ferment() {
        let z = Zinfandel::new(-5.0, 5.0);
        assert!((z.max_ferment - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_mature_rate() {
        let z = Zinfandel::new(100.0, -3.0);
        assert_eq!(z.mature_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zinfandel::default();
        assert!((z.max_ferment - 100.0).abs() < 1e-5);
        assert!((z.mature_rate - 5.0).abs() < 1e-5);
    }

    // --- brew ---

    #[test]
    fn brew_adds_ferment() {
        let mut z = z();
        z.brew(40.0);
        assert!((z.ferment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn brew_clamps_at_max() {
        let mut z = z();
        z.brew(200.0);
        assert!((z.ferment - 100.0).abs() < 1e-3);
    }

    #[test]
    fn brew_fires_just_matured_at_max() {
        let mut z = z();
        z.brew(100.0);
        assert!(z.just_matured);
        assert!(z.is_matured());
    }

    #[test]
    fn brew_no_just_matured_when_already_at_max() {
        let mut z = z();
        z.ferment = 100.0;
        z.brew(10.0);
        assert!(!z.just_matured);
    }

    #[test]
    fn brew_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.brew(50.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn brew_no_op_when_amount_zero() {
        let mut z = z();
        z.brew(0.0);
        assert_eq!(z.ferment, 0.0);
    }

    // --- rack ---

    #[test]
    fn rack_reduces_ferment() {
        let mut z = z();
        z.ferment = 60.0;
        z.rack(20.0);
        assert!((z.ferment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn rack_clamps_at_zero() {
        let mut z = z();
        z.ferment = 30.0;
        z.rack(200.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn rack_fires_just_racked_at_zero() {
        let mut z = z();
        z.ferment = 30.0;
        z.rack(30.0);
        assert!(z.just_racked);
    }

    #[test]
    fn rack_no_op_when_already_racked() {
        let mut z = z();
        z.rack(10.0);
        assert!(!z.just_racked);
    }

    #[test]
    fn rack_no_op_when_disabled() {
        let mut z = z();
        z.ferment = 50.0;
        z.enabled = false;
        z.rack(50.0);
        assert!((z.ferment - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_matures_ferment() {
        let mut z = z(); // rate=5
        z.tick(1.0); // 0 + 5 = 5
        assert!((z.ferment - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_matured_on_mature_to_max() {
        let mut z = Zinfandel::new(100.0, 200.0);
        z.ferment = 95.0;
        z.tick(1.0);
        assert!(z.just_matured);
        assert!(z.is_matured());
    }

    #[test]
    fn tick_no_mature_when_already_matured() {
        let mut z = z();
        z.ferment = 100.0;
        z.tick(1.0);
        assert!(!z.just_matured);
    }

    #[test]
    fn tick_no_mature_when_rate_zero() {
        let mut z = Zinfandel::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn tick_no_mature_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn tick_clears_just_matured() {
        let mut z = Zinfandel::new(100.0, 200.0);
        z.ferment = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_matured);
    }

    #[test]
    fn tick_clears_just_racked() {
        let mut z = z();
        z.ferment = 10.0;
        z.rack(10.0);
        z.tick(0.016);
        assert!(!z.just_racked);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=5
        z.tick(3.0); // 5*3 = 15
        assert!((z.ferment - 15.0).abs() < 1e-3);
    }

    // --- is_matured / is_racked ---

    #[test]
    fn is_matured_false_when_disabled() {
        let mut z = z();
        z.ferment = 100.0;
        z.enabled = false;
        assert!(!z.is_matured());
    }

    #[test]
    fn is_racked_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_racked());
    }

    // --- ferment_fraction / effective_vintage ---

    #[test]
    fn ferment_fraction_zero_when_racked() {
        assert_eq!(z().ferment_fraction(), 0.0);
    }

    #[test]
    fn ferment_fraction_half_at_midpoint() {
        let mut z = z();
        z.ferment = 50.0;
        assert!((z.ferment_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_vintage_zero_when_racked() {
        assert_eq!(z().effective_vintage(100.0), 0.0);
    }

    #[test]
    fn effective_vintage_scales_with_ferment() {
        let mut z = z();
        z.ferment = 70.0;
        assert!((z.effective_vintage(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vintage_zero_when_disabled() {
        let mut z = z();
        z.ferment = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_vintage(100.0), 0.0);
    }
}
