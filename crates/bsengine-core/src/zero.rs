use bevy_ecs::prelude::Component;

/// Zero-crossing detector for a tracked floating-point value. Fires one-frame
/// events when the value crosses zero in either direction — useful for
/// oscillating mechanics (pendulums, sine waves, reversing forces) that need
/// to know the moment a value changes sign.
///
/// Unlike `Health` or `Lifetime` (which track depletion of a resource),
/// `Zero` makes no assumptions about direction or bounds — it monitors
/// **any** value and reports sign transitions.
///
/// `update(new_value)` compares `new_value` against the current `value`
/// sign. Fires `just_crossed_up` when transitioning from non-positive to
/// positive. Fires `just_crossed_down` when transitioning from non-negative
/// to negative. Then sets `value = new_value`. No-op when disabled.
///
/// `tick(_dt)` clears one-frame crossing flags only. No time-based changes.
///
/// `is_positive()` returns `value > 0.0 && enabled`.
///
/// `is_negative()` returns `value < 0.0 && enabled`.
///
/// `is_zero()` returns `value == 0.0 && enabled`.
///
/// `effective_sign(base)` returns `base * value.signum()` when enabled —
/// positive `base` for positive values, negative `base` for negative, zero
/// when exactly at zero; returns `0.0` when disabled.
///
/// Default: value starts at `0.0`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zero {
    /// Current tracked value. May be any finite f32.
    pub value: f32,
    pub just_crossed_up: bool,
    pub just_crossed_down: bool,
    pub enabled: bool,
}

impl Zero {
    pub fn new(initial: f32) -> Self {
        Self {
            value: initial,
            just_crossed_up: false,
            just_crossed_down: false,
            enabled: true,
        }
    }

    /// Set `value` to `new_value`, firing crossing flags when the sign
    /// changes. No-op when disabled.
    pub fn update(&mut self, new_value: f32) {
        if !self.enabled {
            return;
        }
        let was_non_positive = self.value <= 0.0;
        let was_non_negative = self.value >= 0.0;
        if was_non_positive && new_value > 0.0 {
            self.just_crossed_up = true;
        }
        if was_non_negative && new_value < 0.0 {
            self.just_crossed_down = true;
        }
        self.value = new_value;
    }

    /// Advance one frame: clear crossing flags only. No time-based logic.
    pub fn tick(&mut self, _dt: f32) {
        self.just_crossed_up = false;
        self.just_crossed_down = false;
    }

    /// `true` when `value > 0.0` and enabled.
    pub fn is_positive(&self) -> bool {
        self.value > 0.0 && self.enabled
    }

    /// `true` when `value < 0.0` and enabled.
    pub fn is_negative(&self) -> bool {
        self.value < 0.0 && self.enabled
    }

    /// `true` when `value == 0.0` and enabled.
    pub fn is_zero(&self) -> bool {
        self.value == 0.0 && self.enabled
    }

    /// Scale `base` by the sign of `value`. Returns `base * value.signum()`
    /// when enabled — `+base` for positive, `-base` for negative, `0.0` for
    /// exactly zero; `0.0` when disabled.
    pub fn effective_sign(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        if self.value > 0.0 {
            base
        } else if self.value < 0.0 {
            -base
        } else {
            0.0
        }
    }
}

impl Default for Zero {
    fn default() -> Self {
        Self::new(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zero {
        Zero::new(0.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_at_given_value() {
        let z = Zero::new(3.0);
        assert!((z.value - 3.0).abs() < 1e-6);
        assert!(!z.just_crossed_up);
        assert!(!z.just_crossed_down);
    }

    #[test]
    fn default_starts_at_zero() {
        let z = Zero::default();
        assert_eq!(z.value, 0.0);
        assert!(z.is_zero());
    }

    // --- update: crossing up ---

    #[test]
    fn update_fires_just_crossed_up_from_zero() {
        let mut z = z(); // value=0
        z.update(5.0);
        assert!(z.just_crossed_up);
        assert!(!z.just_crossed_down);
    }

    #[test]
    fn update_fires_just_crossed_up_from_negative() {
        let mut z = Zero::new(-3.0);
        z.update(2.0);
        assert!(z.just_crossed_up);
    }

    #[test]
    fn update_no_crossed_up_when_already_positive() {
        let mut z = Zero::new(1.0);
        z.update(5.0);
        assert!(!z.just_crossed_up);
    }

    #[test]
    fn update_no_crossed_up_when_going_to_zero() {
        let mut z = Zero::new(-3.0);
        z.update(0.0); // zero is not positive
        assert!(!z.just_crossed_up);
    }

    // --- update: crossing down ---

    #[test]
    fn update_fires_just_crossed_down_from_zero() {
        let mut z = z(); // value=0
        z.update(-5.0);
        assert!(z.just_crossed_down);
        assert!(!z.just_crossed_up);
    }

    #[test]
    fn update_fires_just_crossed_down_from_positive() {
        let mut z = Zero::new(3.0);
        z.update(-2.0);
        assert!(z.just_crossed_down);
    }

    #[test]
    fn update_no_crossed_down_when_already_negative() {
        let mut z = Zero::new(-1.0);
        z.update(-5.0);
        assert!(!z.just_crossed_down);
    }

    #[test]
    fn update_no_crossed_down_when_going_to_zero() {
        let mut z = Zero::new(3.0);
        z.update(0.0); // zero is not negative
        assert!(!z.just_crossed_down);
    }

    // --- update: no crossing ---

    #[test]
    fn update_no_crossing_positive_to_positive() {
        let mut z = Zero::new(2.0);
        z.update(8.0);
        assert!(!z.just_crossed_up);
        assert!(!z.just_crossed_down);
    }

    #[test]
    fn update_no_crossing_negative_to_negative() {
        let mut z = Zero::new(-2.0);
        z.update(-8.0);
        assert!(!z.just_crossed_up);
        assert!(!z.just_crossed_down);
    }

    #[test]
    fn update_no_crossing_zero_to_zero() {
        let mut z = z();
        z.update(0.0);
        assert!(!z.just_crossed_up);
        assert!(!z.just_crossed_down);
    }

    // --- update: updates value ---

    #[test]
    fn update_sets_value() {
        let mut z = z();
        z.update(7.5);
        assert!((z.value - 7.5).abs() < 1e-6);
    }

    #[test]
    fn update_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.update(5.0);
        assert_eq!(z.value, 0.0);
        assert!(!z.just_crossed_up);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_crossed_up() {
        let mut z = z();
        z.update(5.0); // crossed up
        z.tick(0.016);
        assert!(!z.just_crossed_up);
    }

    #[test]
    fn tick_clears_just_crossed_down() {
        let mut z = z();
        z.update(-5.0); // crossed down
        z.tick(0.016);
        assert!(!z.just_crossed_down);
    }

    #[test]
    fn tick_does_not_change_value() {
        let mut z = Zero::new(3.0);
        z.tick(100.0);
        assert!((z.value - 3.0).abs() < 1e-6);
    }

    // --- is_positive / is_negative / is_zero ---

    #[test]
    fn is_positive_true_for_positive() {
        let z = Zero::new(1.0);
        assert!(z.is_positive());
    }

    #[test]
    fn is_positive_false_at_zero() {
        let z = z();
        assert!(!z.is_positive());
    }

    #[test]
    fn is_positive_false_for_negative() {
        let z = Zero::new(-1.0);
        assert!(!z.is_positive());
    }

    #[test]
    fn is_positive_false_when_disabled() {
        let mut z = Zero::new(1.0);
        z.enabled = false;
        assert!(!z.is_positive());
    }

    #[test]
    fn is_negative_true_for_negative() {
        let z = Zero::new(-1.0);
        assert!(z.is_negative());
    }

    #[test]
    fn is_negative_false_at_zero() {
        let z = z();
        assert!(!z.is_negative());
    }

    #[test]
    fn is_negative_false_for_positive() {
        let z = Zero::new(1.0);
        assert!(!z.is_negative());
    }

    #[test]
    fn is_negative_false_when_disabled() {
        let mut z = Zero::new(-1.0);
        z.enabled = false;
        assert!(!z.is_negative());
    }

    #[test]
    fn is_zero_true_at_zero() {
        let z = z();
        assert!(z.is_zero());
    }

    #[test]
    fn is_zero_false_when_positive() {
        let z = Zero::new(0.001);
        assert!(!z.is_zero());
    }

    #[test]
    fn is_zero_false_when_negative() {
        let z = Zero::new(-0.001);
        assert!(!z.is_zero());
    }

    #[test]
    fn is_zero_false_when_disabled() {
        let mut z = z();
        z.enabled = false;
        assert!(!z.is_zero());
    }

    // --- effective_sign ---

    #[test]
    fn effective_sign_positive_for_positive_value() {
        let z = Zero::new(3.0);
        assert!((z.effective_sign(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_sign_negative_for_negative_value() {
        let z = Zero::new(-3.0);
        assert!((z.effective_sign(100.0) - (-100.0)).abs() < 1e-5);
    }

    #[test]
    fn effective_sign_zero_at_exact_zero() {
        let z = z();
        assert_eq!(z.effective_sign(100.0), 0.0);
    }

    #[test]
    fn effective_sign_zero_when_disabled() {
        let mut z = Zero::new(5.0);
        z.enabled = false;
        assert_eq!(z.effective_sign(100.0), 0.0);
    }

    // --- oscillation cycle ---

    #[test]
    fn oscillation_detects_both_crossings() {
        let mut z = z();
        z.update(5.0); // crossed up
        assert!(z.just_crossed_up);
        z.tick(0.016);
        z.update(-3.0); // crossed down
        assert!(z.just_crossed_down);
        assert!(!z.just_crossed_up);
        z.tick(0.016);
        z.update(2.0); // crossed up again
        assert!(z.just_crossed_up);
    }

    #[test]
    fn large_jump_fires_only_one_crossing() {
        let mut z = Zero::new(-100.0);
        z.update(100.0); // skips zero in one update
        assert!(z.just_crossed_up);
        assert!(!z.just_crossed_down);
    }
}
