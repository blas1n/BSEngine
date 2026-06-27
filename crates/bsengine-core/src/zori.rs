use bevy_ecs::prelude::Component;

/// Traction/footing tracker. `traction` builds via `grip(amount)` and
/// decreases via `slip(amount)` or passive `wear_rate` per second in
/// `tick(dt)`.
///
/// Models footwear condition, surface traction, climbing grip, vehicle
/// tire wear, or any mechanic where maintained contact quality degrades
/// over time and must be actively renewed.
///
/// `grip(amount)` adds traction; fires `just_gripped` when first reaching
/// `max_traction`. No-op when disabled.
///
/// `slip(amount)` subtracts traction; fires `just_slipped` when reaching 0.
/// No-op when disabled or already slipped.
///
/// `tick(dt)` clears both flags, then wears traction down by
/// `wear_rate * dt` (floored at 0). Fires `just_slipped` when reaching 0
/// via wear. No-op wear when disabled or rate is 0.
///
/// `is_gripped()` returns `traction >= max_traction && enabled`.
///
/// `is_slipped()` returns `traction == 0.0` (not gated by `enabled`).
///
/// `traction_fraction()` returns `(traction / max_traction).clamp(0, 1)`.
///
/// `effective_footing(base)` returns `base * traction_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — wears at 5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zori {
    pub traction: f32,
    pub max_traction: f32,
    pub wear_rate: f32,
    pub just_gripped: bool,
    pub just_slipped: bool,
    pub enabled: bool,
}

impl Zori {
    pub fn new(max_traction: f32, wear_rate: f32) -> Self {
        Self {
            traction: 0.0,
            max_traction: max_traction.max(0.1),
            wear_rate: wear_rate.max(0.0),
            just_gripped: false,
            just_slipped: false,
            enabled: true,
        }
    }

    /// Add traction; fires `just_gripped` when first reaching max.
    /// No-op when disabled.
    pub fn grip(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.traction < self.max_traction;
        self.traction = (self.traction + amount).min(self.max_traction);
        if was_below && self.traction >= self.max_traction {
            self.just_gripped = true;
        }
    }

    /// Subtract traction; fires `just_slipped` when reaching 0.
    /// No-op when disabled or already slipped.
    pub fn slip(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.traction <= 0.0 {
            return;
        }
        self.traction = (self.traction - amount).max(0.0);
        if self.traction <= 0.0 {
            self.just_slipped = true;
        }
    }

    /// Clear flags, then wear traction by `wear_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_gripped = false;
        self.just_slipped = false;
        if self.enabled && self.wear_rate > 0.0 && self.traction > 0.0 {
            self.traction = (self.traction - self.wear_rate * dt).max(0.0);
            if self.traction <= 0.0 {
                self.just_slipped = true;
            }
        }
    }

    /// `true` when traction is at maximum and component is enabled.
    pub fn is_gripped(&self) -> bool {
        self.traction >= self.max_traction && self.enabled
    }

    /// `true` when traction is 0 (not gated by `enabled`).
    pub fn is_slipped(&self) -> bool {
        self.traction == 0.0
    }

    /// Fraction of maximum traction [0.0, 1.0].
    pub fn traction_fraction(&self) -> f32 {
        (self.traction / self.max_traction).clamp(0.0, 1.0)
    }

    /// Returns `base * traction_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_footing(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.traction_fraction()
    }
}

impl Default for Zori {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Zori {
        Zori::new(100.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_slipped() {
        let y = y();
        assert_eq!(y.traction, 0.0);
        assert!(y.is_slipped());
        assert!(!y.is_gripped());
    }

    #[test]
    fn new_clamps_max_traction() {
        let y = Zori::new(-5.0, 5.0);
        assert!((y.max_traction - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_wear_rate() {
        let y = Zori::new(100.0, -2.0);
        assert_eq!(y.wear_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Zori::default();
        assert!((y.max_traction - 100.0).abs() < 1e-5);
        assert!((y.wear_rate - 5.0).abs() < 1e-5);
    }

    // --- grip ---

    #[test]
    fn grip_adds_traction() {
        let mut y = y();
        y.grip(40.0);
        assert!((y.traction - 40.0).abs() < 1e-3);
    }

    #[test]
    fn grip_clamps_at_max() {
        let mut y = y();
        y.grip(200.0);
        assert!((y.traction - 100.0).abs() < 1e-3);
    }

    #[test]
    fn grip_fires_just_gripped_at_max() {
        let mut y = y();
        y.grip(100.0);
        assert!(y.just_gripped);
        assert!(y.is_gripped());
    }

    #[test]
    fn grip_no_just_gripped_when_already_at_max() {
        let mut y = y();
        y.traction = 100.0;
        y.grip(10.0);
        assert!(!y.just_gripped);
    }

    #[test]
    fn grip_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.grip(50.0);
        assert_eq!(y.traction, 0.0);
    }

    #[test]
    fn grip_no_op_when_amount_zero() {
        let mut y = y();
        y.grip(0.0);
        assert_eq!(y.traction, 0.0);
    }

    // --- slip ---

    #[test]
    fn slip_reduces_traction() {
        let mut y = y();
        y.traction = 60.0;
        y.slip(20.0);
        assert!((y.traction - 40.0).abs() < 1e-3);
    }

    #[test]
    fn slip_clamps_at_zero() {
        let mut y = y();
        y.traction = 30.0;
        y.slip(200.0);
        assert_eq!(y.traction, 0.0);
    }

    #[test]
    fn slip_fires_just_slipped_at_zero() {
        let mut y = y();
        y.traction = 30.0;
        y.slip(30.0);
        assert!(y.just_slipped);
    }

    #[test]
    fn slip_no_op_when_already_slipped() {
        let mut y = y();
        y.slip(10.0);
        assert!(!y.just_slipped);
    }

    #[test]
    fn slip_no_op_when_disabled() {
        let mut y = y();
        y.traction = 50.0;
        y.enabled = false;
        y.slip(50.0);
        assert!((y.traction - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_wears_traction() {
        let mut y = y(); // wear=5
        y.traction = 60.0;
        y.tick(1.0); // 60 - 5 = 55
        assert!((y.traction - 55.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_slipped_on_wear_to_zero() {
        let mut y = Zori::new(100.0, 200.0);
        y.traction = 5.0;
        y.tick(1.0);
        assert!(y.just_slipped);
        assert!(y.is_slipped());
    }

    #[test]
    fn tick_no_wear_when_already_slipped() {
        let mut y = y();
        y.tick(10.0);
        assert!(!y.just_slipped);
    }

    #[test]
    fn tick_no_wear_when_rate_zero() {
        let mut y = Zori::new(100.0, 0.0);
        y.traction = 50.0;
        y.tick(100.0);
        assert!((y.traction - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_wear_when_disabled() {
        let mut y = y();
        y.traction = 50.0;
        y.enabled = false;
        y.tick(1.0);
        assert!((y.traction - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_gripped() {
        let mut y = y();
        y.grip(100.0);
        y.tick(0.016);
        assert!(!y.just_gripped);
    }

    #[test]
    fn tick_clears_just_slipped() {
        let mut y = Zori::new(100.0, 200.0);
        y.traction = 5.0;
        y.tick(1.0);
        y.tick(0.016);
        assert!(!y.just_slipped);
    }

    #[test]
    fn tick_scales_wear_with_dt() {
        let mut y = y(); // wear=5
        y.traction = 100.0;
        y.tick(2.0); // 100 - 5*2 = 90
        assert!((y.traction - 90.0).abs() < 1e-3);
    }

    // --- is_gripped / is_slipped ---

    #[test]
    fn is_gripped_false_when_disabled() {
        let mut y = y();
        y.traction = 100.0;
        y.enabled = false;
        assert!(!y.is_gripped());
    }

    #[test]
    fn is_slipped_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_slipped());
    }

    // --- traction_fraction / effective_footing ---

    #[test]
    fn traction_fraction_zero_when_slipped() {
        assert_eq!(y().traction_fraction(), 0.0);
    }

    #[test]
    fn traction_fraction_half_at_midpoint() {
        let mut y = y();
        y.traction = 50.0;
        assert!((y.traction_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_footing_zero_when_slipped() {
        assert_eq!(y().effective_footing(100.0), 0.0);
    }

    #[test]
    fn effective_footing_scales_with_traction() {
        let mut y = y();
        y.traction = 80.0;
        assert!((y.effective_footing(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_footing_zero_when_disabled() {
        let mut y = y();
        y.traction = 50.0;
        y.enabled = false;
        assert_eq!(y.effective_footing(100.0), 0.0);
    }
}
