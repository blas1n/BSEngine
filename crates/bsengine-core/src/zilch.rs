use bevy_ecs::prelude::Component;

/// Deprivation-pressure tracker. `deprivation` builds via `deprive(amount)`
/// and eases passively at `replenish_rate` per second in `tick(dt)` or
/// immediately via `nourish(amount)`.
///
/// Models starvation stress, fuel-empty warning pressure, ammo-dry-fire
/// penalty, drought meters, power-outage accumulators, or any mechanic
/// where the absence of a resource causes steadily mounting strain that
/// eases when the resource is restored.
///
/// `deprive(amount)` adds deprivation; fires `just_exhausted` when first
/// reaching `max_deprivation`. No-op when disabled.
///
/// `nourish(amount)` reduces deprivation immediately; fires `just_sated`
/// when reaching 0. No-op when disabled or already sated.
///
/// `tick(dt)` clears both flags, then eases deprivation by
/// `replenish_rate * dt` (floored at 0). Fires `just_sated` when reaching
/// 0. No-op when disabled or rate is 0.
///
/// `is_exhausted()` returns `deprivation >= max_deprivation && enabled`.
///
/// `is_sated()` returns `deprivation == 0.0` (not gated by `enabled`).
///
/// `deprivation_fraction()` returns
/// `(deprivation / max_deprivation).clamp(0, 1)`.
///
/// `effective_hunger(scale)` returns `scale * deprivation_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — eases at 5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zilch {
    pub deprivation: f32,
    pub max_deprivation: f32,
    pub replenish_rate: f32,
    pub just_exhausted: bool,
    pub just_sated: bool,
    pub enabled: bool,
}

impl Zilch {
    pub fn new(max_deprivation: f32, replenish_rate: f32) -> Self {
        Self {
            deprivation: 0.0,
            max_deprivation: max_deprivation.max(0.1),
            replenish_rate: replenish_rate.max(0.0),
            just_exhausted: false,
            just_sated: false,
            enabled: true,
        }
    }

    /// Add deprivation; fires `just_exhausted` when first reaching max.
    /// No-op when disabled.
    pub fn deprive(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.deprivation < self.max_deprivation;
        self.deprivation = (self.deprivation + amount).min(self.max_deprivation);
        if was_below && self.deprivation >= self.max_deprivation {
            self.just_exhausted = true;
        }
    }

    /// Reduce deprivation; fires `just_sated` when reaching 0.
    /// No-op when disabled or already sated.
    pub fn nourish(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.deprivation <= 0.0 {
            return;
        }
        self.deprivation = (self.deprivation - amount).max(0.0);
        if self.deprivation <= 0.0 {
            self.just_sated = true;
        }
    }

    /// Clear flags, then ease deprivation by `replenish_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_exhausted = false;
        self.just_sated = false;
        if self.enabled && self.replenish_rate > 0.0 && self.deprivation > 0.0 {
            self.deprivation = (self.deprivation - self.replenish_rate * dt).max(0.0);
            if self.deprivation <= 0.0 {
                self.just_sated = true;
            }
        }
    }

    /// `true` when deprivation is at maximum and component is enabled.
    pub fn is_exhausted(&self) -> bool {
        self.deprivation >= self.max_deprivation && self.enabled
    }

    /// `true` when deprivation is 0 (not gated by `enabled`).
    pub fn is_sated(&self) -> bool {
        self.deprivation == 0.0
    }

    /// Fraction of maximum deprivation [0.0, 1.0].
    pub fn deprivation_fraction(&self) -> f32 {
        (self.deprivation / self.max_deprivation).clamp(0.0, 1.0)
    }

    /// Returns `scale * deprivation_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_hunger(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.deprivation_fraction()
    }
}

impl Default for Zilch {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zilch {
        Zilch::new(100.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_sated() {
        let z = z();
        assert_eq!(z.deprivation, 0.0);
        assert!(z.is_sated());
        assert!(!z.is_exhausted());
    }

    #[test]
    fn new_clamps_max_deprivation() {
        let z = Zilch::new(-5.0, 5.0);
        assert!((z.max_deprivation - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_replenish_rate() {
        let z = Zilch::new(100.0, -3.0);
        assert_eq!(z.replenish_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zilch::default();
        assert!((z.max_deprivation - 100.0).abs() < 1e-5);
        assert!((z.replenish_rate - 5.0).abs() < 1e-5);
    }

    // --- deprive ---

    #[test]
    fn deprive_adds_deprivation() {
        let mut z = z();
        z.deprive(40.0);
        assert!((z.deprivation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn deprive_clamps_at_max() {
        let mut z = z();
        z.deprive(200.0);
        assert!((z.deprivation - 100.0).abs() < 1e-3);
    }

    #[test]
    fn deprive_fires_just_exhausted_at_max() {
        let mut z = z();
        z.deprive(100.0);
        assert!(z.just_exhausted);
        assert!(z.is_exhausted());
    }

    #[test]
    fn deprive_no_just_exhausted_when_already_at_max() {
        let mut z = z();
        z.deprivation = 100.0;
        z.deprive(10.0);
        assert!(!z.just_exhausted);
    }

    #[test]
    fn deprive_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.deprive(50.0);
        assert_eq!(z.deprivation, 0.0);
    }

    #[test]
    fn deprive_no_op_when_amount_zero() {
        let mut z = z();
        z.deprive(0.0);
        assert_eq!(z.deprivation, 0.0);
    }

    // --- nourish ---

    #[test]
    fn nourish_reduces_deprivation() {
        let mut z = z();
        z.deprivation = 60.0;
        z.nourish(20.0);
        assert!((z.deprivation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn nourish_clamps_at_zero() {
        let mut z = z();
        z.deprivation = 30.0;
        z.nourish(200.0);
        assert_eq!(z.deprivation, 0.0);
    }

    #[test]
    fn nourish_fires_just_sated_at_zero() {
        let mut z = z();
        z.deprivation = 30.0;
        z.nourish(30.0);
        assert!(z.just_sated);
    }

    #[test]
    fn nourish_no_op_when_already_sated() {
        let mut z = z();
        z.nourish(10.0);
        assert!(!z.just_sated);
    }

    #[test]
    fn nourish_no_op_when_disabled() {
        let mut z = z();
        z.deprivation = 50.0;
        z.enabled = false;
        z.nourish(50.0);
        assert!((z.deprivation - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_eases_deprivation() {
        let mut z = z(); // replenish=5
        z.deprivation = 60.0;
        z.tick(1.0); // 60 - 5 = 55
        assert!((z.deprivation - 55.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_sated_on_ease_to_zero() {
        let mut z = Zilch::new(100.0, 200.0);
        z.deprivation = 5.0;
        z.tick(1.0);
        assert!(z.just_sated);
        assert!(z.is_sated());
    }

    #[test]
    fn tick_no_ease_when_already_sated() {
        let mut z = z();
        z.tick(10.0);
        assert!(!z.just_sated);
    }

    #[test]
    fn tick_no_ease_when_rate_zero() {
        let mut z = Zilch::new(100.0, 0.0);
        z.deprivation = 50.0;
        z.tick(100.0);
        assert!((z.deprivation - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_ease_when_disabled() {
        let mut z = z();
        z.deprivation = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.deprivation - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_exhausted() {
        let mut z = z();
        z.deprive(100.0);
        z.tick(0.016);
        assert!(!z.just_exhausted);
    }

    #[test]
    fn tick_clears_just_sated() {
        let mut z = Zilch::new(100.0, 200.0);
        z.deprivation = 5.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_sated);
    }

    #[test]
    fn tick_scales_ease_with_dt() {
        let mut z = z(); // replenish=5
        z.deprivation = 100.0;
        z.tick(4.0); // 100 - 5*4 = 80
        assert!((z.deprivation - 80.0).abs() < 1e-3);
    }

    // --- is_exhausted / is_sated ---

    #[test]
    fn is_exhausted_false_when_disabled() {
        let mut z = z();
        z.deprivation = 100.0;
        z.enabled = false;
        assert!(!z.is_exhausted());
    }

    #[test]
    fn is_sated_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_sated());
    }

    // --- deprivation_fraction / effective_hunger ---

    #[test]
    fn deprivation_fraction_zero_when_sated() {
        assert_eq!(z().deprivation_fraction(), 0.0);
    }

    #[test]
    fn deprivation_fraction_half_at_midpoint() {
        let mut z = z();
        z.deprivation = 50.0;
        assert!((z.deprivation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_hunger_zero_when_sated() {
        assert_eq!(z().effective_hunger(100.0), 0.0);
    }

    #[test]
    fn effective_hunger_scales_with_deprivation() {
        let mut z = z();
        z.deprivation = 75.0;
        assert!((z.effective_hunger(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_hunger_zero_when_disabled() {
        let mut z = z();
        z.deprivation = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_hunger(100.0), 0.0);
    }
}
