use bevy_ecs::prelude::Component;

/// Bipolar balance meter in [-1.0, 1.0]. Tracks a continuous duality value
/// between two opposing extremes — dark (−1) and light (+1) — with a neutral
/// centre at 0. Models alignment (good/evil), emotional valence, elemental
/// opposition, light/shadow intensity, or any two-sided scalar state.
///
/// `darken(amount)` shifts balance toward −1. Fires `just_darkened` on
/// first reaching −1. No-op when disabled or already at −1.
///
/// `lighten(amount)` shifts balance toward +1. Fires `just_lightened` on
/// first reaching +1. No-op when disabled or already at +1.
///
/// `center(amount)` moves balance toward 0 by the given step. No-op when
/// disabled or already neutral.
///
/// `tick(_dt)` clears `just_darkened` and `just_lightened` only.
///
/// `is_dark()` returns `balance < 0.0`.
///
/// `is_light()` returns `balance > 0.0`.
///
/// `is_neutral()` returns `balance == 0.0`.
///
/// `dark_fraction()` returns `(-balance).clamp(0.0, 1.0)`.
///
/// `light_fraction()` returns `balance.clamp(0.0, 1.0)`.
///
/// `effective_dark(base)` returns `base * dark_fraction()` when enabled;
/// `0.0` when disabled.
///
/// `effective_light(base)` returns `base * light_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new()` — starts neutral at 0.0.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yin {
    /// Current balance in [-1.0, 1.0]. 0 = neutral, -1 = fully dark, +1 = fully light.
    pub balance: f32,
    pub just_darkened: bool,
    pub just_lightened: bool,
    pub enabled: bool,
}

impl Yin {
    pub fn new() -> Self {
        Self {
            balance: 0.0,
            just_darkened: false,
            just_lightened: false,
            enabled: true,
        }
    }

    /// Shift toward dark (−1). Fires `just_darkened` on reaching −1. No-op
    /// when disabled or already fully dark.
    pub fn darken(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.balance <= -1.0 {
            return;
        }
        self.balance = (self.balance - amount).max(-1.0);
        if self.balance <= -1.0 {
            self.just_darkened = true;
        }
    }

    /// Shift toward light (+1). Fires `just_lightened` on reaching +1. No-op
    /// when disabled or already fully light.
    pub fn lighten(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.balance >= 1.0 {
            return;
        }
        self.balance = (self.balance + amount).min(1.0);
        if self.balance >= 1.0 {
            self.just_lightened = true;
        }
    }

    /// Move balance toward 0 by the given step. No-op when disabled or
    /// already neutral.
    pub fn center(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.balance == 0.0 {
            return;
        }
        if self.balance < 0.0 {
            self.balance = (self.balance + amount).min(0.0);
        } else {
            self.balance = (self.balance - amount).max(0.0);
        }
    }

    /// Advance one frame: clear `just_darkened` and `just_lightened` only.
    pub fn tick(&mut self, _dt: f32) {
        self.just_darkened = false;
        self.just_lightened = false;
    }

    /// `true` when balance is negative.
    pub fn is_dark(&self) -> bool {
        self.balance < 0.0
    }

    /// `true` when balance is positive.
    pub fn is_light(&self) -> bool {
        self.balance > 0.0
    }

    /// `true` when balance is exactly 0.
    pub fn is_neutral(&self) -> bool {
        self.balance == 0.0
    }

    /// Magnitude of darkness [0.0, 1.0].
    pub fn dark_fraction(&self) -> f32 {
        (-self.balance).clamp(0.0, 1.0)
    }

    /// Magnitude of light [0.0, 1.0].
    pub fn light_fraction(&self) -> f32 {
        self.balance.clamp(0.0, 1.0)
    }

    /// Returns `base * dark_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_dark(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.dark_fraction()
    }

    /// Returns `base * light_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_light(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.light_fraction()
    }
}

impl Default for Yin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yin {
        Yin::new()
    }

    // --- construction ---

    #[test]
    fn new_starts_neutral() {
        let y = y();
        assert_eq!(y.balance, 0.0);
        assert!(y.is_neutral());
        assert!(!y.is_dark());
        assert!(!y.is_light());
    }

    #[test]
    fn default_same_as_new() {
        let y = Yin::default();
        assert_eq!(y.balance, 0.0);
    }

    // --- darken ---

    #[test]
    fn darken_shifts_balance_negative() {
        let mut y = y();
        y.darken(0.5);
        assert!((y.balance + 0.5).abs() < 1e-5);
    }

    #[test]
    fn darken_clamps_at_neg_one() {
        let mut y = y();
        y.darken(5.0);
        assert!((y.balance + 1.0).abs() < 1e-5);
    }

    #[test]
    fn darken_fires_just_darkened_at_limit() {
        let mut y = y();
        y.darken(1.0);
        assert!(y.just_darkened);
        assert!(y.is_dark());
    }

    #[test]
    fn darken_no_op_when_already_at_limit() {
        let mut y = y();
        y.darken(1.0);
        y.tick(0.016);
        y.darken(0.1);
        assert!(!y.just_darkened); // no refire
    }

    #[test]
    fn darken_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.darken(0.5);
        assert_eq!(y.balance, 0.0);
    }

    #[test]
    fn darken_no_op_for_zero_amount() {
        let mut y = y();
        y.darken(0.0);
        assert_eq!(y.balance, 0.0);
    }

    // --- lighten ---

    #[test]
    fn lighten_shifts_balance_positive() {
        let mut y = y();
        y.lighten(0.5);
        assert!((y.balance - 0.5).abs() < 1e-5);
    }

    #[test]
    fn lighten_clamps_at_pos_one() {
        let mut y = y();
        y.lighten(5.0);
        assert!((y.balance - 1.0).abs() < 1e-5);
    }

    #[test]
    fn lighten_fires_just_lightened_at_limit() {
        let mut y = y();
        y.lighten(1.0);
        assert!(y.just_lightened);
        assert!(y.is_light());
    }

    #[test]
    fn lighten_no_op_when_already_at_limit() {
        let mut y = y();
        y.lighten(1.0);
        y.tick(0.016);
        y.lighten(0.1);
        assert!(!y.just_lightened);
    }

    #[test]
    fn lighten_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.lighten(0.5);
        assert_eq!(y.balance, 0.0);
    }

    // --- center ---

    #[test]
    fn center_moves_dark_toward_neutral() {
        let mut y = y();
        y.darken(0.8);
        y.center(0.3);
        assert!((y.balance + 0.5).abs() < 1e-4);
    }

    #[test]
    fn center_moves_light_toward_neutral() {
        let mut y = y();
        y.lighten(0.8);
        y.center(0.3);
        assert!((y.balance - 0.5).abs() < 1e-4);
    }

    #[test]
    fn center_clamps_at_zero_from_dark() {
        let mut y = y();
        y.darken(0.5);
        y.center(1.0);
        assert_eq!(y.balance, 0.0);
    }

    #[test]
    fn center_clamps_at_zero_from_light() {
        let mut y = y();
        y.lighten(0.5);
        y.center(1.0);
        assert_eq!(y.balance, 0.0);
    }

    #[test]
    fn center_no_op_when_disabled() {
        let mut y = y();
        y.darken(0.5);
        y.enabled = false;
        y.center(0.3);
        assert!((y.balance + 0.5).abs() < 1e-5);
    }

    #[test]
    fn center_no_op_when_neutral() {
        let mut y = y();
        y.center(0.5);
        assert_eq!(y.balance, 0.0);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_darkened() {
        let mut y = y();
        y.darken(1.0);
        y.tick(0.016);
        assert!(!y.just_darkened);
    }

    #[test]
    fn tick_clears_just_lightened() {
        let mut y = y();
        y.lighten(1.0);
        y.tick(0.016);
        assert!(!y.just_lightened);
    }

    #[test]
    fn tick_does_not_change_balance() {
        let mut y = y();
        y.darken(0.5);
        y.tick(1000.0);
        assert!((y.balance + 0.5).abs() < 1e-5);
    }

    // --- fractions ---

    #[test]
    fn dark_fraction_zero_when_neutral() {
        assert_eq!(y().dark_fraction(), 0.0);
    }

    #[test]
    fn dark_fraction_one_when_fully_dark() {
        let mut y = y();
        y.darken(1.0);
        assert!((y.dark_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn dark_fraction_half_at_midpoint() {
        let mut y = y();
        y.darken(0.5);
        assert!((y.dark_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn light_fraction_zero_when_neutral() {
        assert_eq!(y().light_fraction(), 0.0);
    }

    #[test]
    fn light_fraction_one_when_fully_light() {
        let mut y = y();
        y.lighten(1.0);
        assert!((y.light_fraction() - 1.0).abs() < 1e-5);
    }

    // --- effective ---

    #[test]
    fn effective_dark_zero_when_neutral() {
        assert_eq!(y().effective_dark(100.0), 0.0);
    }

    #[test]
    fn effective_dark_scales_with_fraction() {
        let mut y = y();
        y.darken(0.5);
        assert!((y.effective_dark(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_dark_zero_when_disabled() {
        let mut y = y();
        y.darken(1.0);
        y.enabled = false;
        assert_eq!(y.effective_dark(100.0), 0.0);
    }

    #[test]
    fn effective_light_scales_with_fraction() {
        let mut y = y();
        y.lighten(0.75);
        assert!((y.effective_light(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_light_zero_when_disabled() {
        let mut y = y();
        y.lighten(1.0);
        y.enabled = false;
        assert_eq!(y.effective_light(100.0), 0.0);
    }
}
