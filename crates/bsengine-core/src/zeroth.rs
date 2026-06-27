use bevy_ecs::prelude::Component;

/// Baseline-drift tracker. `deviation` builds via `drift(amount)` and
/// creeps up passively at `drift_rate` per second in `tick(dt)` or is
/// corrected immediately via `correct(amount)`.
///
/// Models entropy / calibration drift for precision machinery, signal-
/// noise accumulation in sensor components, radiation-exposure meters,
/// quantum-decoherence gauges, accumulated-error trackers for navigation
/// systems, or any mechanic where small deviations from an ideal baseline
/// build up over time and must be periodically corrected.
///
/// `drift(amount)` adds deviation; fires `just_saturated` when first
/// reaching `max_deviation`. No-op when disabled.
///
/// `correct(amount)` reduces deviation immediately; fires `just_grounded`
/// when reaching 0. No-op when disabled or already grounded.
///
/// `tick(dt)` clears both flags, then increases deviation by
/// `drift_rate * dt` (capped at `max_deviation`). Fires `just_saturated`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_saturated()` returns `deviation >= max_deviation && enabled`.
///
/// `is_grounded()` returns `deviation == 0.0` (not gated by `enabled`).
///
/// `deviation_fraction()` returns `(deviation / max_deviation).clamp(0, 1)`.
///
/// `effective_noise(scale)` returns `scale * deviation_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — drifts at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeroth {
    pub deviation: f32,
    pub max_deviation: f32,
    pub drift_rate: f32,
    pub just_saturated: bool,
    pub just_grounded: bool,
    pub enabled: bool,
}

impl Zeroth {
    pub fn new(max_deviation: f32, drift_rate: f32) -> Self {
        Self {
            deviation: 0.0,
            max_deviation: max_deviation.max(0.1),
            drift_rate: drift_rate.max(0.0),
            just_saturated: false,
            just_grounded: false,
            enabled: true,
        }
    }

    /// Add deviation; fires `just_saturated` when first reaching max.
    /// No-op when disabled.
    pub fn drift(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.deviation < self.max_deviation;
        self.deviation = (self.deviation + amount).min(self.max_deviation);
        if was_below && self.deviation >= self.max_deviation {
            self.just_saturated = true;
        }
    }

    /// Reduce deviation; fires `just_grounded` when reaching 0.
    /// No-op when disabled or already grounded.
    pub fn correct(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.deviation <= 0.0 {
            return;
        }
        self.deviation = (self.deviation - amount).max(0.0);
        if self.deviation <= 0.0 {
            self.just_grounded = true;
        }
    }

    /// Clear flags, then increase deviation by `drift_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_saturated = false;
        self.just_grounded = false;
        if self.enabled && self.drift_rate > 0.0 && self.deviation < self.max_deviation {
            let was_below = self.deviation < self.max_deviation;
            self.deviation = (self.deviation + self.drift_rate * dt).min(self.max_deviation);
            if was_below && self.deviation >= self.max_deviation {
                self.just_saturated = true;
            }
        }
    }

    /// `true` when deviation is at maximum and component is enabled.
    pub fn is_saturated(&self) -> bool {
        self.deviation >= self.max_deviation && self.enabled
    }

    /// `true` when deviation is 0 (not gated by `enabled`).
    pub fn is_grounded(&self) -> bool {
        self.deviation == 0.0
    }

    /// Fraction of maximum deviation [0.0, 1.0].
    pub fn deviation_fraction(&self) -> f32 {
        (self.deviation / self.max_deviation).clamp(0.0, 1.0)
    }

    /// Returns `scale * deviation_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_noise(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.deviation_fraction()
    }
}

impl Default for Zeroth {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeroth {
        Zeroth::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_grounded() {
        let z = z();
        assert_eq!(z.deviation, 0.0);
        assert!(z.is_grounded());
        assert!(!z.is_saturated());
    }

    #[test]
    fn new_clamps_max_deviation() {
        let z = Zeroth::new(-5.0, 4.0);
        assert!((z.max_deviation - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_drift_rate() {
        let z = Zeroth::new(100.0, -3.0);
        assert_eq!(z.drift_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeroth::default();
        assert!((z.max_deviation - 100.0).abs() < 1e-5);
        assert!((z.drift_rate - 4.0).abs() < 1e-5);
    }

    // --- drift ---

    #[test]
    fn drift_adds_deviation() {
        let mut z = z();
        z.drift(40.0);
        assert!((z.deviation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn drift_clamps_at_max() {
        let mut z = z();
        z.drift(200.0);
        assert!((z.deviation - 100.0).abs() < 1e-3);
    }

    #[test]
    fn drift_fires_just_saturated_at_max() {
        let mut z = z();
        z.drift(100.0);
        assert!(z.just_saturated);
        assert!(z.is_saturated());
    }

    #[test]
    fn drift_no_just_saturated_when_already_at_max() {
        let mut z = z();
        z.deviation = 100.0;
        z.drift(10.0);
        assert!(!z.just_saturated);
    }

    #[test]
    fn drift_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.drift(50.0);
        assert_eq!(z.deviation, 0.0);
    }

    #[test]
    fn drift_no_op_when_amount_zero() {
        let mut z = z();
        z.drift(0.0);
        assert_eq!(z.deviation, 0.0);
    }

    // --- correct ---

    #[test]
    fn correct_reduces_deviation() {
        let mut z = z();
        z.deviation = 60.0;
        z.correct(20.0);
        assert!((z.deviation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn correct_clamps_at_zero() {
        let mut z = z();
        z.deviation = 30.0;
        z.correct(200.0);
        assert_eq!(z.deviation, 0.0);
    }

    #[test]
    fn correct_fires_just_grounded_at_zero() {
        let mut z = z();
        z.deviation = 30.0;
        z.correct(30.0);
        assert!(z.just_grounded);
    }

    #[test]
    fn correct_no_op_when_already_grounded() {
        let mut z = z();
        z.correct(10.0);
        assert!(!z.just_grounded);
    }

    #[test]
    fn correct_no_op_when_disabled() {
        let mut z = z();
        z.deviation = 50.0;
        z.enabled = false;
        z.correct(50.0);
        assert!((z.deviation - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_increases_deviation() {
        let mut z = z(); // rate=4
        z.tick(1.0); // 0 + 4 = 4
        assert!((z.deviation - 4.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_saturated_on_drift_to_max() {
        let mut z = Zeroth::new(100.0, 200.0);
        z.deviation = 95.0;
        z.tick(1.0);
        assert!(z.just_saturated);
        assert!(z.is_saturated());
    }

    #[test]
    fn tick_no_drift_when_already_saturated() {
        let mut z = z();
        z.deviation = 100.0;
        z.tick(1.0);
        assert!(!z.just_saturated);
    }

    #[test]
    fn tick_no_drift_when_rate_zero() {
        let mut z = Zeroth::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.deviation, 0.0);
    }

    #[test]
    fn tick_no_drift_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.deviation, 0.0);
    }

    #[test]
    fn tick_clears_just_saturated() {
        let mut z = Zeroth::new(100.0, 200.0);
        z.deviation = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_saturated);
    }

    #[test]
    fn tick_clears_just_grounded() {
        let mut z = z();
        z.deviation = 10.0;
        z.correct(10.0);
        z.tick(0.016);
        assert!(!z.just_grounded);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=4
        z.tick(5.0); // 4*5 = 20
        assert!((z.deviation - 20.0).abs() < 1e-3);
    }

    // --- is_saturated / is_grounded ---

    #[test]
    fn is_saturated_false_when_disabled() {
        let mut z = z();
        z.deviation = 100.0;
        z.enabled = false;
        assert!(!z.is_saturated());
    }

    #[test]
    fn is_grounded_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_grounded());
    }

    // --- deviation_fraction / effective_noise ---

    #[test]
    fn deviation_fraction_zero_when_grounded() {
        assert_eq!(z().deviation_fraction(), 0.0);
    }

    #[test]
    fn deviation_fraction_half_at_midpoint() {
        let mut z = z();
        z.deviation = 50.0;
        assert!((z.deviation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_noise_zero_when_grounded() {
        assert_eq!(z().effective_noise(100.0), 0.0);
    }

    #[test]
    fn effective_noise_scales_with_deviation() {
        let mut z = z();
        z.deviation = 60.0;
        assert!((z.effective_noise(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_noise_zero_when_disabled() {
        let mut z = z();
        z.deviation = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_noise(100.0), 0.0);
    }
}
