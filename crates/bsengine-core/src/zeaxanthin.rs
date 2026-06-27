use bevy_ecs::prelude::Component;

/// Macular-pigment photoprotection tracker. `pigment` builds via
/// `deposit(amount)` and accumulates passively at `absorb_rate` per
/// second in `tick(dt)` or bleaches immediately via `bleach(amount)`.
///
/// Models yellow carotenoid macular-pigment saturation bars, retinal
/// photoprotection fill levels, dietary carotenoid accumulation
/// gauges, blue-light-shielding density trackers, photoreceptor-
/// guard pigment intensity meters, lutein-zeaxanthin deposit build-
/// up indicators, ocular-antioxidant saturation trackers, macular
/// density fill levels, corneal-filter carotenoid concentration
/// bars, or any mechanic where patient dietary accumulation of a
/// yellow plant pigment slowly builds a dense protective lens across
/// the center of the retina — only for sustained photoxic exposure
/// to bleach years of careful accumulation back to a pale
/// insufficiency in a matter of days.
///
/// `deposit(amount)` adds pigment; fires `just_saturated` when
/// first reaching `max_pigment`. No-op when disabled.
///
/// `bleach(amount)` reduces pigment immediately; fires `just_depleted`
/// when reaching 0. No-op when disabled or already depleted.
///
/// `tick(dt)` clears both flags, then increases pigment by
/// `absorb_rate * dt` (capped at `max_pigment`). Fires
/// `just_saturated` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_saturated()` returns `pigment >= max_pigment && enabled`.
///
/// `is_depleted()` returns `pigment == 0.0` (not gated by `enabled`).
///
/// `pigment_fraction()` returns `(pigment / max_pigment).clamp(0, 1)`.
///
/// `effective_shielding(scale)` returns `scale * pigment_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — absorbs at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeaxanthin {
    pub pigment: f32,
    pub max_pigment: f32,
    pub absorb_rate: f32,
    pub just_saturated: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Zeaxanthin {
    pub fn new(max_pigment: f32, absorb_rate: f32) -> Self {
        Self {
            pigment: 0.0,
            max_pigment: max_pigment.max(0.1),
            absorb_rate: absorb_rate.max(0.0),
            just_saturated: false,
            just_depleted: false,
            enabled: true,
        }
    }

    /// Add pigment; fires `just_saturated` when first reaching max.
    /// No-op when disabled.
    pub fn deposit(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.pigment < self.max_pigment;
        self.pigment = (self.pigment + amount).min(self.max_pigment);
        if was_below && self.pigment >= self.max_pigment {
            self.just_saturated = true;
        }
    }

    /// Reduce pigment; fires `just_depleted` when reaching 0.
    /// No-op when disabled or already depleted.
    pub fn bleach(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pigment <= 0.0 {
            return;
        }
        self.pigment = (self.pigment - amount).max(0.0);
        if self.pigment <= 0.0 {
            self.just_depleted = true;
        }
    }

    /// Clear flags, then increase pigment by `absorb_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_saturated = false;
        self.just_depleted = false;
        if self.enabled && self.absorb_rate > 0.0 && self.pigment < self.max_pigment {
            let was_below = self.pigment < self.max_pigment;
            self.pigment = (self.pigment + self.absorb_rate * dt).min(self.max_pigment);
            if was_below && self.pigment >= self.max_pigment {
                self.just_saturated = true;
            }
        }
    }

    /// `true` when pigment is at maximum and component is enabled.
    pub fn is_saturated(&self) -> bool {
        self.pigment >= self.max_pigment && self.enabled
    }

    /// `true` when pigment is 0 (not gated by `enabled`).
    pub fn is_depleted(&self) -> bool {
        self.pigment == 0.0
    }

    /// Fraction of maximum pigment [0.0, 1.0].
    pub fn pigment_fraction(&self) -> f32 {
        (self.pigment / self.max_pigment).clamp(0.0, 1.0)
    }

    /// Returns `scale * pigment_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_shielding(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.pigment_fraction()
    }
}

impl Default for Zeaxanthin {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeaxanthin {
        Zeaxanthin::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_depleted() {
        let z = z();
        assert_eq!(z.pigment, 0.0);
        assert!(z.is_depleted());
        assert!(!z.is_saturated());
    }

    #[test]
    fn new_clamps_max_pigment() {
        let z = Zeaxanthin::new(-5.0, 1.5);
        assert!((z.max_pigment - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_absorb_rate() {
        let z = Zeaxanthin::new(100.0, -1.5);
        assert_eq!(z.absorb_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeaxanthin::default();
        assert!((z.max_pigment - 100.0).abs() < 1e-5);
        assert!((z.absorb_rate - 1.5).abs() < 1e-5);
    }

    // --- deposit ---

    #[test]
    fn deposit_adds_pigment() {
        let mut z = z();
        z.deposit(40.0);
        assert!((z.pigment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn deposit_clamps_at_max() {
        let mut z = z();
        z.deposit(200.0);
        assert!((z.pigment - 100.0).abs() < 1e-3);
    }

    #[test]
    fn deposit_fires_just_saturated_at_max() {
        let mut z = z();
        z.deposit(100.0);
        assert!(z.just_saturated);
        assert!(z.is_saturated());
    }

    #[test]
    fn deposit_no_just_saturated_when_already_at_max() {
        let mut z = z();
        z.pigment = 100.0;
        z.deposit(10.0);
        assert!(!z.just_saturated);
    }

    #[test]
    fn deposit_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.deposit(50.0);
        assert_eq!(z.pigment, 0.0);
    }

    #[test]
    fn deposit_no_op_when_amount_zero() {
        let mut z = z();
        z.deposit(0.0);
        assert_eq!(z.pigment, 0.0);
    }

    // --- bleach ---

    #[test]
    fn bleach_reduces_pigment() {
        let mut z = z();
        z.pigment = 60.0;
        z.bleach(20.0);
        assert!((z.pigment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn bleach_clamps_at_zero() {
        let mut z = z();
        z.pigment = 30.0;
        z.bleach(200.0);
        assert_eq!(z.pigment, 0.0);
    }

    #[test]
    fn bleach_fires_just_depleted_at_zero() {
        let mut z = z();
        z.pigment = 30.0;
        z.bleach(30.0);
        assert!(z.just_depleted);
    }

    #[test]
    fn bleach_no_op_when_already_depleted() {
        let mut z = z();
        z.bleach(10.0);
        assert!(!z.just_depleted);
    }

    #[test]
    fn bleach_no_op_when_disabled() {
        let mut z = z();
        z.pigment = 50.0;
        z.enabled = false;
        z.bleach(50.0);
        assert!((z.pigment - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_absorbs_pigment() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.pigment - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_saturated_on_absorb_to_max() {
        let mut z = Zeaxanthin::new(100.0, 200.0);
        z.pigment = 95.0;
        z.tick(1.0);
        assert!(z.just_saturated);
        assert!(z.is_saturated());
    }

    #[test]
    fn tick_no_absorb_when_already_saturated() {
        let mut z = z();
        z.pigment = 100.0;
        z.tick(1.0);
        assert!(!z.just_saturated);
    }

    #[test]
    fn tick_no_absorb_when_rate_zero() {
        let mut z = Zeaxanthin::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.pigment, 0.0);
    }

    #[test]
    fn tick_no_absorb_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.pigment, 0.0);
    }

    #[test]
    fn tick_clears_just_saturated() {
        let mut z = Zeaxanthin::new(100.0, 200.0);
        z.pigment = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_saturated);
    }

    #[test]
    fn tick_clears_just_depleted() {
        let mut z = z();
        z.pigment = 10.0;
        z.bleach(10.0);
        z.tick(0.016);
        assert!(!z.just_depleted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.pigment - 9.0).abs() < 1e-3);
    }

    // --- is_saturated / is_depleted ---

    #[test]
    fn is_saturated_false_when_disabled() {
        let mut z = z();
        z.pigment = 100.0;
        z.enabled = false;
        assert!(!z.is_saturated());
    }

    #[test]
    fn is_depleted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_depleted());
    }

    // --- pigment_fraction / effective_shielding ---

    #[test]
    fn pigment_fraction_zero_when_depleted() {
        assert_eq!(z().pigment_fraction(), 0.0);
    }

    #[test]
    fn pigment_fraction_half_at_midpoint() {
        let mut z = z();
        z.pigment = 50.0;
        assert!((z.pigment_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_shielding_zero_when_depleted() {
        assert_eq!(z().effective_shielding(100.0), 0.0);
    }

    #[test]
    fn effective_shielding_scales_with_pigment() {
        let mut z = z();
        z.pigment = 75.0;
        assert!((z.effective_shielding(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_shielding_zero_when_disabled() {
        let mut z = z();
        z.pigment = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_shielding(100.0), 0.0);
    }
}
