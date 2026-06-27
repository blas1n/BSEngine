use bevy_ecs::prelude::Component;

/// Cumulative exposure accumulator that saturates once and can be drained.
/// Models radiation dose, sound exposure, heat buildup, or any threshold
/// that accumulates over time and triggers a one-shot effect.
///
/// `expose(amount)` adds to `exposure`, capped at `max_exposure`. Fires
/// `just_saturated` the first time `exposure` reaches `max_exposure`. No-op
/// when disabled or amount <= 0.
///
/// `drain(amount)` reduces `exposure` toward 0. No-op when disabled or
/// amount <= 0.
///
/// `tick(_dt)` clears `just_saturated` only. No time-based logic.
///
/// `is_saturated()` returns `exposure >= max_exposure && enabled`.
///
/// `exposure_fraction()` returns `(exposure / max_exposure).clamp(0.0, 1.0)`.
///
/// `effective_exposure(base)` returns `base * exposure_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0)` — saturates at 100 units.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yew {
    /// Current accumulated exposure [0, max_exposure].
    pub exposure: f32,
    /// Saturation threshold. Clamped >= 0.1.
    pub max_exposure: f32,
    pub just_saturated: bool,
    pub enabled: bool,
}

impl Yew {
    pub fn new(max_exposure: f32) -> Self {
        Self {
            exposure: 0.0,
            max_exposure: max_exposure.max(0.1),
            just_saturated: false,
            enabled: true,
        }
    }

    /// Add exposure. Fires `just_saturated` the first time the cap is
    /// reached. No-op when disabled or amount <= 0.
    pub fn expose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.exposure < self.max_exposure;
        self.exposure = (self.exposure + amount).min(self.max_exposure);
        if was_below && self.exposure >= self.max_exposure {
            self.just_saturated = true;
        }
    }

    /// Remove exposure, flooring at 0. No-op when disabled or amount <= 0.
    pub fn drain(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.exposure = (self.exposure - amount).max(0.0);
    }

    /// Advance one frame: clear `just_saturated`. No time-based logic.
    pub fn tick(&mut self, _dt: f32) {
        self.just_saturated = false;
    }

    /// `true` when exposure has reached maximum and component is enabled.
    pub fn is_saturated(&self) -> bool {
        self.exposure >= self.max_exposure && self.enabled
    }

    /// Exposure as a fraction of maximum [0.0, 1.0].
    pub fn exposure_fraction(&self) -> f32 {
        (self.exposure / self.max_exposure).clamp(0.0, 1.0)
    }

    /// Returns `base * exposure_fraction()` when enabled; `0.0` when
    /// disabled.
    pub fn effective_exposure(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.exposure_fraction()
    }
}

impl Default for Yew {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yew {
        Yew::new(10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let y = y();
        assert_eq!(y.exposure, 0.0);
        assert!(!y.just_saturated);
        assert!(!y.is_saturated());
    }

    #[test]
    fn max_exposure_clamped_to_tenth() {
        let y = Yew::new(0.0);
        assert!((y.max_exposure - 0.1).abs() < 1e-5);
    }

    // --- expose ---

    #[test]
    fn expose_adds_to_exposure() {
        let mut y = y();
        y.expose(3.0);
        assert!((y.exposure - 3.0).abs() < 1e-4);
    }

    #[test]
    fn expose_caps_at_max() {
        let mut y = y();
        y.expose(15.0);
        assert!((y.exposure - 10.0).abs() < 1e-4);
    }

    #[test]
    fn expose_fires_just_saturated_at_cap() {
        let mut y = y();
        y.expose(10.0);
        assert!(y.just_saturated);
        assert!(y.is_saturated());
    }

    #[test]
    fn expose_fires_just_saturated_crossing_cap() {
        let mut y = y();
        y.expose(7.0);
        y.expose(5.0); // crosses 10
        assert!(y.just_saturated);
    }

    #[test]
    fn expose_does_not_refire_just_saturated() {
        let mut y = y();
        y.expose(10.0);
        y.tick(0.016);
        y.expose(1.0); // already at cap
        assert!(!y.just_saturated);
    }

    #[test]
    fn expose_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.expose(5.0);
        assert_eq!(y.exposure, 0.0);
        assert!(!y.just_saturated);
    }

    #[test]
    fn expose_no_op_with_zero_amount() {
        let mut y = y();
        y.expose(0.0);
        assert_eq!(y.exposure, 0.0);
    }

    #[test]
    fn expose_no_op_with_negative_amount() {
        let mut y = y();
        y.expose(-5.0);
        assert_eq!(y.exposure, 0.0);
    }

    // --- drain ---

    #[test]
    fn drain_reduces_exposure() {
        let mut y = y();
        y.expose(8.0);
        y.drain(3.0);
        assert!((y.exposure - 5.0).abs() < 1e-4);
    }

    #[test]
    fn drain_floors_at_zero() {
        let mut y = y();
        y.expose(3.0);
        y.drain(10.0);
        assert_eq!(y.exposure, 0.0);
    }

    #[test]
    fn drain_no_op_when_disabled() {
        let mut y = y();
        y.expose(5.0);
        y.enabled = false;
        y.drain(2.0);
        assert!((y.exposure - 5.0).abs() < 1e-4);
    }

    #[test]
    fn drain_no_op_with_zero_amount() {
        let mut y = y();
        y.expose(5.0);
        y.drain(0.0);
        assert!((y.exposure - 5.0).abs() < 1e-4);
    }

    #[test]
    fn drain_allows_resaturation() {
        let mut y = y();
        y.expose(10.0); // saturated
        y.tick(0.016);
        y.drain(5.0);
        y.expose(5.0); // saturates again
        assert!(y.just_saturated);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_saturated() {
        let mut y = y();
        y.expose(10.0);
        y.tick(0.016);
        assert!(!y.just_saturated);
    }

    #[test]
    fn tick_does_not_change_exposure() {
        let mut y = y();
        y.expose(5.0);
        y.tick(100.0);
        assert!((y.exposure - 5.0).abs() < 1e-4);
    }

    // --- is_saturated ---

    #[test]
    fn is_saturated_false_below_max() {
        let mut y = y();
        y.expose(9.0);
        assert!(!y.is_saturated());
    }

    #[test]
    fn is_saturated_true_at_max() {
        let mut y = y();
        y.expose(10.0);
        assert!(y.is_saturated());
    }

    #[test]
    fn is_saturated_false_when_disabled() {
        let mut y = y();
        y.expose(10.0);
        y.enabled = false;
        assert!(!y.is_saturated());
    }

    // --- exposure_fraction ---

    #[test]
    fn exposure_fraction_zero_when_empty() {
        assert_eq!(y().exposure_fraction(), 0.0);
    }

    #[test]
    fn exposure_fraction_at_half() {
        let mut y = y();
        y.expose(5.0);
        assert!((y.exposure_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn exposure_fraction_one_at_max() {
        let mut y = y();
        y.expose(10.0);
        assert!((y.exposure_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_exposure ---

    #[test]
    fn effective_exposure_zero_when_empty() {
        assert_eq!(y().effective_exposure(100.0), 0.0);
    }

    #[test]
    fn effective_exposure_scales_with_fraction() {
        let mut y = y();
        y.expose(5.0); // fraction=0.5
        assert!((y.effective_exposure(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_exposure_full_at_saturation() {
        let mut y = y();
        y.expose(10.0);
        assert!((y.effective_exposure(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_exposure_zero_when_disabled() {
        let mut y = y();
        y.expose(10.0);
        y.enabled = false;
        assert_eq!(y.effective_exposure(100.0), 0.0);
    }
}
