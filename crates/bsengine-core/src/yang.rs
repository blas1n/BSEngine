use bevy_ecs::prelude::Component;

/// Charge-driven polarity flipper. Accumulates charge via `energize()`; when
/// charge reaches `threshold`, polarity toggles (positive ↔ negative), charge
/// resets to zero, and `just_flipped` fires. Models AC electricity, magnetic
/// reversal, directional switching, or any mechanic where accumulated energy
/// triggers a binary state flip.
///
/// `energize(amount)` adds charge. When charge reaches or exceeds `threshold`
/// the polarity flips, charge resets to 0, and `just_flipped` fires. No-op
/// when disabled or `amount <= 0`.
///
/// `discharge(amount)` drains charge toward 0. No-op when disabled or
/// `amount <= 0`.
///
/// `tick(_dt)` clears `just_flipped` only.
///
/// `is_positive()` returns `polarity && enabled`.
///
/// `is_negative()` returns `!polarity` (not gated by `enabled`).
///
/// `charge_fraction()` returns `(charge / threshold).clamp(0, 1)`.
///
/// `effective_output(base)` returns `base` when positive and enabled;
/// `-base` when negative and enabled; `0.0` when disabled.
///
/// Default: `new(10.0)` — starts negative (polarity = false), charge = 0.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yang {
    pub charge: f32,
    pub threshold: f32,
    /// Current polarity: `false` = negative, `true` = positive.
    pub polarity: bool,
    pub just_flipped: bool,
    pub enabled: bool,
}

impl Yang {
    pub fn new(threshold: f32) -> Self {
        Self {
            charge: 0.0,
            threshold: threshold.max(0.1),
            polarity: false,
            just_flipped: false,
            enabled: true,
        }
    }

    /// Add charge. When accumulated charge reaches `threshold`, flip polarity,
    /// reset charge to 0, and fire `just_flipped`. No-op when disabled or
    /// `amount <= 0`.
    pub fn energize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.charge += amount;
        if self.charge >= self.threshold {
            self.charge = 0.0;
            self.polarity = !self.polarity;
            self.just_flipped = true;
        }
    }

    /// Drain charge toward 0. No-op when disabled or `amount <= 0`.
    pub fn discharge(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.charge = (self.charge - amount).max(0.0);
    }

    /// Advance one frame: clear `just_flipped` only.
    pub fn tick(&mut self, _dt: f32) {
        self.just_flipped = false;
    }

    /// `true` when polarity is positive and component is enabled.
    pub fn is_positive(&self) -> bool {
        self.polarity && self.enabled
    }

    /// `true` when polarity is negative (not gated by `enabled`).
    pub fn is_negative(&self) -> bool {
        !self.polarity
    }

    /// Fraction of threshold filled by current charge [0.0, 1.0].
    pub fn charge_fraction(&self) -> f32 {
        (self.charge / self.threshold).clamp(0.0, 1.0)
    }

    /// Returns `base` when positive and enabled; `-base` when negative and
    /// enabled; `0.0` when disabled.
    pub fn effective_output(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        if self.polarity {
            base
        } else {
            -base
        }
    }
}

impl Default for Yang {
    fn default() -> Self {
        Self::new(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yang {
        Yang::new(10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_negative_at_zero_charge() {
        let y = y();
        assert_eq!(y.charge, 0.0);
        assert!(!y.polarity);
        assert!(y.is_negative());
        assert!(!y.is_positive());
    }

    #[test]
    fn new_clamps_threshold() {
        let y = Yang::new(-1.0);
        assert!((y.threshold - 0.1).abs() < 1e-5);
    }

    #[test]
    fn default_threshold_is_ten() {
        assert!((Yang::default().threshold - 10.0).abs() < 1e-5);
    }

    // --- energize ---

    #[test]
    fn energize_increases_charge() {
        let mut y = y();
        y.energize(3.0);
        assert!((y.charge - 3.0).abs() < 1e-4);
    }

    #[test]
    fn energize_flips_polarity_at_threshold() {
        let mut y = y();
        y.energize(10.0);
        assert!(y.polarity);
        assert!(y.just_flipped);
    }

    #[test]
    fn energize_resets_charge_on_flip() {
        let mut y = y();
        y.energize(10.0);
        assert_eq!(y.charge, 0.0);
    }

    #[test]
    fn energize_flips_back_on_second_threshold() {
        let mut y = y();
        y.energize(10.0); // → positive
        y.tick(0.016);
        y.energize(10.0); // → negative
        assert!(!y.polarity);
        assert!(y.just_flipped);
    }

    #[test]
    fn energize_over_threshold_still_resets_charge() {
        let mut y = y();
        y.energize(15.0); // over threshold
        assert_eq!(y.charge, 0.0); // reset, not stored as excess
    }

    #[test]
    fn energize_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.energize(10.0);
        assert_eq!(y.charge, 0.0);
        assert!(!y.polarity);
    }

    #[test]
    fn energize_no_op_for_zero_amount() {
        let mut y = y();
        y.energize(0.0);
        assert_eq!(y.charge, 0.0);
    }

    #[test]
    fn energize_partial_charge_does_not_flip() {
        let mut y = y();
        y.energize(5.0);
        assert!(!y.polarity);
        assert!(!y.just_flipped);
    }

    // --- discharge ---

    #[test]
    fn discharge_reduces_charge() {
        let mut y = y();
        y.energize(7.0);
        y.discharge(3.0);
        assert!((y.charge - 4.0).abs() < 1e-4);
    }

    #[test]
    fn discharge_clamps_at_zero() {
        let mut y = y();
        y.energize(5.0);
        y.discharge(10.0);
        assert_eq!(y.charge, 0.0);
    }

    #[test]
    fn discharge_no_op_when_disabled() {
        let mut y = y();
        y.energize(5.0);
        y.enabled = false;
        y.discharge(3.0);
        assert!((y.charge - 5.0).abs() < 1e-4);
    }

    #[test]
    fn discharge_no_op_for_zero_amount() {
        let mut y = y();
        y.energize(5.0);
        y.discharge(0.0);
        assert!((y.charge - 5.0).abs() < 1e-4);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_flipped() {
        let mut y = y();
        y.energize(10.0);
        y.tick(0.016);
        assert!(!y.just_flipped);
    }

    #[test]
    fn tick_does_not_change_polarity_or_charge() {
        let mut y = y();
        y.energize(5.0);
        y.tick(1000.0);
        assert!(!y.polarity);
        assert!((y.charge - 5.0).abs() < 1e-4);
    }

    // --- is_positive / is_negative ---

    #[test]
    fn is_positive_after_flip() {
        let mut y = y();
        y.energize(10.0);
        assert!(y.is_positive());
    }

    #[test]
    fn is_positive_false_when_disabled() {
        let mut y = y();
        y.energize(10.0);
        y.enabled = false;
        assert!(!y.is_positive());
    }

    #[test]
    fn is_negative_true_at_start() {
        assert!(y().is_negative());
    }

    #[test]
    fn is_negative_true_even_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_negative()); // not gated by enabled
    }

    #[test]
    fn is_negative_false_when_positive() {
        let mut y = y();
        y.energize(10.0);
        assert!(!y.is_negative());
    }

    // --- charge_fraction ---

    #[test]
    fn charge_fraction_zero_at_start() {
        assert_eq!(y().charge_fraction(), 0.0);
    }

    #[test]
    fn charge_fraction_half_at_midpoint() {
        let mut y = y();
        y.energize(5.0);
        assert!((y.charge_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn charge_fraction_zero_after_flip() {
        let mut y = y();
        y.energize(10.0);
        assert_eq!(y.charge_fraction(), 0.0); // charge reset to 0
    }

    // --- effective_output ---

    #[test]
    fn effective_output_negative_base_when_negative() {
        assert!((y().effective_output(10.0) + 10.0).abs() < 1e-4);
    }

    #[test]
    fn effective_output_positive_base_when_positive() {
        let mut y = y();
        y.energize(10.0);
        assert!((y.effective_output(10.0) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn effective_output_zero_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert_eq!(y.effective_output(10.0), 0.0);
    }
}
