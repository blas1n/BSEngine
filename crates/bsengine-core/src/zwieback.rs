use bevy_ecs::prelude::Component;

/// Twice-baked crispness tracker. `crispness` builds via
/// `bake(amount)` and toasts passively at `toast_rate` per second
/// in `tick(dt)` or softens immediately via `dampen(amount)`.
///
/// Models twice-baked-bread texture saturation bars, biscotti-
/// hardness progression gauges, toasting-cycle completion trackers,
/// rusk-crispness fill levels, dried-bread durability indicators,
/// long-journey-provision condition meters, infants'-teething-food
/// hardness accumulators, survival-ration shelf-stability gauges,
/// dehydration-stage completion bars, culinary-drying-process
/// intensity trackers, or any mechanic where patient double-baking
/// slowly draws every last drop of moisture from a bread until it
/// rings like ceramic when tapped against a tin — only for a sudden
/// splash to dissolve weeks of careful toasting back into a
/// characterless sodden mass in moments.
///
/// `bake(amount)` adds crispness; fires `just_crisp` when first
/// reaching `max_crispness`. No-op when disabled.
///
/// `dampen(amount)` reduces crispness immediately; fires
/// `just_soggy` when reaching 0. No-op when disabled or already
/// soggy.
///
/// `tick(dt)` clears both flags, then increases crispness by
/// `toast_rate * dt` (capped at `max_crispness`). Fires `just_crisp`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_crisp()` returns `crispness >= max_crispness && enabled`.
///
/// `is_soggy()` returns `crispness == 0.0` (not gated by `enabled`).
///
/// `crispness_fraction()` returns `(crispness / max_crispness).clamp(0, 1)`.
///
/// `effective_texture(scale)` returns `scale * crispness_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — toasts at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zwieback {
    pub crispness: f32,
    pub max_crispness: f32,
    pub toast_rate: f32,
    pub just_crisp: bool,
    pub just_soggy: bool,
    pub enabled: bool,
}

impl Zwieback {
    pub fn new(max_crispness: f32, toast_rate: f32) -> Self {
        Self {
            crispness: 0.0,
            max_crispness: max_crispness.max(0.1),
            toast_rate: toast_rate.max(0.0),
            just_crisp: false,
            just_soggy: false,
            enabled: true,
        }
    }

    /// Add crispness; fires `just_crisp` when first reaching max.
    /// No-op when disabled.
    pub fn bake(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.crispness < self.max_crispness;
        self.crispness = (self.crispness + amount).min(self.max_crispness);
        if was_below && self.crispness >= self.max_crispness {
            self.just_crisp = true;
        }
    }

    /// Reduce crispness; fires `just_soggy` when reaching 0.
    /// No-op when disabled or already soggy.
    pub fn dampen(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.crispness <= 0.0 {
            return;
        }
        self.crispness = (self.crispness - amount).max(0.0);
        if self.crispness <= 0.0 {
            self.just_soggy = true;
        }
    }

    /// Clear flags, then increase crispness by `toast_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_crisp = false;
        self.just_soggy = false;
        if self.enabled && self.toast_rate > 0.0 && self.crispness < self.max_crispness {
            let was_below = self.crispness < self.max_crispness;
            self.crispness = (self.crispness + self.toast_rate * dt).min(self.max_crispness);
            if was_below && self.crispness >= self.max_crispness {
                self.just_crisp = true;
            }
        }
    }

    /// `true` when crispness is at maximum and component is enabled.
    pub fn is_crisp(&self) -> bool {
        self.crispness >= self.max_crispness && self.enabled
    }

    /// `true` when crispness is 0 (not gated by `enabled`).
    pub fn is_soggy(&self) -> bool {
        self.crispness == 0.0
    }

    /// Fraction of maximum crispness [0.0, 1.0].
    pub fn crispness_fraction(&self) -> f32 {
        (self.crispness / self.max_crispness).clamp(0.0, 1.0)
    }

    /// Returns `scale * crispness_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_texture(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.crispness_fraction()
    }
}

impl Default for Zwieback {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zwieback {
        Zwieback::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_soggy() {
        let z = z();
        assert_eq!(z.crispness, 0.0);
        assert!(z.is_soggy());
        assert!(!z.is_crisp());
    }

    #[test]
    fn new_clamps_max_crispness() {
        let z = Zwieback::new(-5.0, 1.5);
        assert!((z.max_crispness - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_toast_rate() {
        let z = Zwieback::new(100.0, -1.5);
        assert_eq!(z.toast_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zwieback::default();
        assert!((z.max_crispness - 100.0).abs() < 1e-5);
        assert!((z.toast_rate - 1.5).abs() < 1e-5);
    }

    // --- bake ---

    #[test]
    fn bake_adds_crispness() {
        let mut z = z();
        z.bake(40.0);
        assert!((z.crispness - 40.0).abs() < 1e-3);
    }

    #[test]
    fn bake_clamps_at_max() {
        let mut z = z();
        z.bake(200.0);
        assert!((z.crispness - 100.0).abs() < 1e-3);
    }

    #[test]
    fn bake_fires_just_crisp_at_max() {
        let mut z = z();
        z.bake(100.0);
        assert!(z.just_crisp);
        assert!(z.is_crisp());
    }

    #[test]
    fn bake_no_just_crisp_when_already_at_max() {
        let mut z = z();
        z.crispness = 100.0;
        z.bake(10.0);
        assert!(!z.just_crisp);
    }

    #[test]
    fn bake_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.bake(50.0);
        assert_eq!(z.crispness, 0.0);
    }

    #[test]
    fn bake_no_op_when_amount_zero() {
        let mut z = z();
        z.bake(0.0);
        assert_eq!(z.crispness, 0.0);
    }

    // --- dampen ---

    #[test]
    fn dampen_reduces_crispness() {
        let mut z = z();
        z.crispness = 60.0;
        z.dampen(20.0);
        assert!((z.crispness - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dampen_clamps_at_zero() {
        let mut z = z();
        z.crispness = 30.0;
        z.dampen(200.0);
        assert_eq!(z.crispness, 0.0);
    }

    #[test]
    fn dampen_fires_just_soggy_at_zero() {
        let mut z = z();
        z.crispness = 30.0;
        z.dampen(30.0);
        assert!(z.just_soggy);
    }

    #[test]
    fn dampen_no_op_when_already_soggy() {
        let mut z = z();
        z.dampen(10.0);
        assert!(!z.just_soggy);
    }

    #[test]
    fn dampen_no_op_when_disabled() {
        let mut z = z();
        z.crispness = 50.0;
        z.enabled = false;
        z.dampen(50.0);
        assert!((z.crispness - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_toasts_crispness() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.crispness - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_crisp_on_toast_to_max() {
        let mut z = Zwieback::new(100.0, 200.0);
        z.crispness = 95.0;
        z.tick(1.0);
        assert!(z.just_crisp);
        assert!(z.is_crisp());
    }

    #[test]
    fn tick_no_toast_when_already_crisp() {
        let mut z = z();
        z.crispness = 100.0;
        z.tick(1.0);
        assert!(!z.just_crisp);
    }

    #[test]
    fn tick_no_toast_when_rate_zero() {
        let mut z = Zwieback::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.crispness, 0.0);
    }

    #[test]
    fn tick_no_toast_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.crispness, 0.0);
    }

    #[test]
    fn tick_clears_just_crisp() {
        let mut z = Zwieback::new(100.0, 200.0);
        z.crispness = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_crisp);
    }

    #[test]
    fn tick_clears_just_soggy() {
        let mut z = z();
        z.crispness = 10.0;
        z.dampen(10.0);
        z.tick(0.016);
        assert!(!z.just_soggy);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.crispness - 9.0).abs() < 1e-3);
    }

    // --- is_crisp / is_soggy ---

    #[test]
    fn is_crisp_false_when_disabled() {
        let mut z = z();
        z.crispness = 100.0;
        z.enabled = false;
        assert!(!z.is_crisp());
    }

    #[test]
    fn is_soggy_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_soggy());
    }

    // --- crispness_fraction / effective_texture ---

    #[test]
    fn crispness_fraction_zero_when_soggy() {
        assert_eq!(z().crispness_fraction(), 0.0);
    }

    #[test]
    fn crispness_fraction_half_at_midpoint() {
        let mut z = z();
        z.crispness = 50.0;
        assert!((z.crispness_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_texture_zero_when_soggy() {
        assert_eq!(z().effective_texture(100.0), 0.0);
    }

    #[test]
    fn effective_texture_scales_with_crispness() {
        let mut z = z();
        z.crispness = 75.0;
        assert!((z.effective_texture(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_texture_zero_when_disabled() {
        let mut z = z();
        z.crispness = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_texture(100.0), 0.0);
    }
}
