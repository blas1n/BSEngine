use bevy_ecs::prelude::Component;

/// Signed lateral-offset tracker. Unlike accumulators that clamp at [0, max],
/// `offset` is signed and clamps at [-max_offset, +max_offset], making it
/// the natural primitive for oscillating or weaving movement patterns.
///
/// `push(amount)` shifts `offset` in the signed direction — positive values
/// move right, negative move left. Fires `just_peaked` when first reaching
/// `+max_offset`, and `just_bottomed` when first reaching `-max_offset`.
/// No-op when disabled.
///
/// `tick(dt)` clears both flags, then drifts `offset` toward 0 at
/// `drift_rate` per second (the sign of the drift is automatic). No-op
/// when disabled or `drift_rate` is 0.
///
/// `is_peaked()` returns `offset >= max_offset && enabled`.
///
/// `is_bottomed()` returns `offset <= -max_offset && enabled`.
///
/// `is_centered()` returns `offset == 0.0` (not gated by `enabled`).
///
/// `offset_fraction()` returns `(offset / max_offset).clamp(-1.0, 1.0)` —
/// a signed fraction in [-1, 1] where 0 is center.
///
/// `signed_intensity(scale)` returns `scale * offset_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 10.0)` — max ±100, drifts to center at 10/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zag {
    pub offset: f32,
    pub max_offset: f32,
    pub drift_rate: f32,
    pub just_peaked: bool,
    pub just_bottomed: bool,
    pub enabled: bool,
}

impl Zag {
    pub fn new(max_offset: f32, drift_rate: f32) -> Self {
        Self {
            offset: 0.0,
            max_offset: max_offset.max(0.1),
            drift_rate: drift_rate.max(0.0),
            just_peaked: false,
            just_bottomed: false,
            enabled: true,
        }
    }

    /// Shift `offset` by `amount` (signed). Fires `just_peaked` or
    /// `just_bottomed` on first contact with ±`max_offset`. No-op when disabled.
    pub fn push(&mut self, amount: f32) {
        if !self.enabled || amount == 0.0 {
            return;
        }
        let prev = self.offset;
        self.offset = (self.offset + amount).clamp(-self.max_offset, self.max_offset);
        if prev < self.max_offset && self.offset >= self.max_offset {
            self.just_peaked = true;
        }
        if prev > -self.max_offset && self.offset <= -self.max_offset {
            self.just_bottomed = true;
        }
    }

    /// Clear flags, then drift `offset` toward 0 by `drift_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_bottomed = false;
        if !self.enabled || self.drift_rate <= 0.0 || self.offset == 0.0 {
            return;
        }
        let drift = self.drift_rate * dt;
        if self.offset > 0.0 {
            self.offset = (self.offset - drift).max(0.0);
        } else {
            self.offset = (self.offset + drift).min(0.0);
        }
    }

    /// `true` when offset is at positive maximum and component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.offset >= self.max_offset && self.enabled
    }

    /// `true` when offset is at negative maximum and component is enabled.
    pub fn is_bottomed(&self) -> bool {
        self.offset <= -self.max_offset && self.enabled
    }

    /// `true` when offset is exactly 0 (not gated by `enabled`).
    pub fn is_centered(&self) -> bool {
        self.offset == 0.0
    }

    /// Signed fraction of maximum offset [-1.0, 1.0].
    pub fn offset_fraction(&self) -> f32 {
        (self.offset / self.max_offset).clamp(-1.0, 1.0)
    }

    /// Returns `scale * offset_fraction()` when enabled; `0.0` when disabled.
    pub fn signed_intensity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.offset_fraction()
    }
}

impl Default for Zag {
    fn default() -> Self {
        Self::new(100.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Zag {
        Zag::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_centered() {
        let y = y();
        assert_eq!(y.offset, 0.0);
        assert!(y.is_centered());
        assert!(!y.is_peaked());
        assert!(!y.is_bottomed());
    }

    #[test]
    fn new_clamps_max_offset() {
        let y = Zag::new(-5.0, 10.0);
        assert!((y.max_offset - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_drift_rate() {
        let y = Zag::new(100.0, -3.0);
        assert_eq!(y.drift_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Zag::default();
        assert!((y.max_offset - 100.0).abs() < 1e-5);
        assert!((y.drift_rate - 10.0).abs() < 1e-5);
    }

    // --- push positive ---

    #[test]
    fn push_positive_shifts_right() {
        let mut y = y();
        y.push(30.0);
        assert!((y.offset - 30.0).abs() < 1e-3);
    }

    #[test]
    fn push_positive_clamps_at_max() {
        let mut y = y();
        y.push(200.0);
        assert!((y.offset - 100.0).abs() < 1e-3);
    }

    #[test]
    fn push_positive_fires_just_peaked() {
        let mut y = y();
        y.push(100.0);
        assert!(y.just_peaked);
        assert!(y.is_peaked());
    }

    #[test]
    fn push_positive_no_just_peaked_when_already_peaked() {
        let mut y = y();
        y.offset = 100.0;
        y.push(10.0);
        assert!(!y.just_peaked);
    }

    // --- push negative ---

    #[test]
    fn push_negative_shifts_left() {
        let mut y = y();
        y.push(-40.0);
        assert!((y.offset - (-40.0)).abs() < 1e-3);
    }

    #[test]
    fn push_negative_clamps_at_min() {
        let mut y = y();
        y.push(-200.0);
        assert!((y.offset - (-100.0)).abs() < 1e-3);
    }

    #[test]
    fn push_negative_fires_just_bottomed() {
        let mut y = y();
        y.push(-100.0);
        assert!(y.just_bottomed);
        assert!(y.is_bottomed());
    }

    #[test]
    fn push_negative_no_just_bottomed_when_already_bottomed() {
        let mut y = y();
        y.offset = -100.0;
        y.push(-10.0);
        assert!(!y.just_bottomed);
    }

    // --- push guard conditions ---

    #[test]
    fn push_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.push(50.0);
        assert_eq!(y.offset, 0.0);
    }

    #[test]
    fn push_no_op_when_amount_zero() {
        let mut y = y();
        y.push(0.0);
        assert_eq!(y.offset, 0.0);
        assert!(!y.just_peaked);
    }

    // --- tick drift ---

    #[test]
    fn tick_drifts_right_offset_toward_center() {
        let mut y = y(); // drift_rate=10
        y.offset = 60.0;
        y.tick(1.0); // 60 - 10 = 50
        assert!((y.offset - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_drifts_left_offset_toward_center() {
        let mut y = y();
        y.offset = -60.0;
        y.tick(1.0); // -60 + 10 = -50
        assert!((y.offset - (-50.0)).abs() < 1e-3);
    }

    #[test]
    fn tick_floors_right_drift_at_zero() {
        let mut y = Zag::new(100.0, 200.0);
        y.offset = 5.0;
        y.tick(1.0);
        assert_eq!(y.offset, 0.0);
    }

    #[test]
    fn tick_floors_left_drift_at_zero() {
        let mut y = Zag::new(100.0, 200.0);
        y.offset = -5.0;
        y.tick(1.0);
        assert_eq!(y.offset, 0.0);
    }

    #[test]
    fn tick_no_drift_when_centered() {
        let mut y = y();
        y.tick(100.0); // already centered
        assert_eq!(y.offset, 0.0);
        assert!(!y.just_peaked);
        assert!(!y.just_bottomed);
    }

    #[test]
    fn tick_no_drift_when_rate_zero() {
        let mut y = Zag::new(100.0, 0.0);
        y.offset = 50.0;
        y.tick(100.0);
        assert!((y.offset - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_drift_when_disabled() {
        let mut y = y();
        y.offset = 50.0;
        y.enabled = false;
        y.tick(1.0);
        assert!((y.offset - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut y = y();
        y.push(100.0); // just_peaked fires
        y.tick(0.016);
        assert!(!y.just_peaked);
    }

    #[test]
    fn tick_clears_just_bottomed() {
        let mut y = y();
        y.push(-100.0); // just_bottomed fires
        y.tick(0.016);
        assert!(!y.just_bottomed);
    }

    #[test]
    fn tick_scales_drift_with_dt() {
        let mut y = y(); // drift=10
        y.offset = 80.0;
        y.tick(2.0); // 80 - 10*2 = 60
        assert!((y.offset - 60.0).abs() < 1e-3);
    }

    // --- is_peaked / is_bottomed / is_centered ---

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut y = y();
        y.offset = 100.0;
        y.enabled = false;
        assert!(!y.is_peaked());
    }

    #[test]
    fn is_bottomed_false_when_disabled() {
        let mut y = y();
        y.offset = -100.0;
        y.enabled = false;
        assert!(!y.is_bottomed());
    }

    #[test]
    fn is_centered_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_centered());
    }

    // --- offset_fraction / signed_intensity ---

    #[test]
    fn offset_fraction_zero_when_centered() {
        assert_eq!(y().offset_fraction(), 0.0);
    }

    #[test]
    fn offset_fraction_positive_one_when_peaked() {
        let mut y = y();
        y.offset = 100.0;
        assert!((y.offset_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn offset_fraction_negative_one_when_bottomed() {
        let mut y = y();
        y.offset = -100.0;
        assert!((y.offset_fraction() - (-1.0)).abs() < 1e-4);
    }

    #[test]
    fn offset_fraction_half_at_midpoint() {
        let mut y = y();
        y.offset = 50.0;
        assert!((y.offset_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn signed_intensity_zero_when_centered() {
        assert_eq!(y().signed_intensity(100.0), 0.0);
    }

    #[test]
    fn signed_intensity_scales_with_offset() {
        let mut y = y();
        y.offset = 75.0;
        assert!((y.signed_intensity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn signed_intensity_negative_when_left() {
        let mut y = y();
        y.offset = -50.0;
        assert!((y.signed_intensity(100.0) - (-50.0)).abs() < 1e-3);
    }

    #[test]
    fn signed_intensity_zero_when_disabled() {
        let mut y = y();
        y.offset = 50.0;
        y.enabled = false;
        assert_eq!(y.signed_intensity(100.0), 0.0);
    }
}
