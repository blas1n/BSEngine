use bevy_ecs::prelude::Component;

/// Performance-anxiety tracker. Accumulates `pressure` toward `max_pressure`
/// as an entity encounters stressful moments. High pressure degrades
/// `effective_output`. Pressure can be relieved via `compose()`.
///
/// Models the "yips" phenomenon — clutch-moment anxiety, stage-fright
/// mechanics, focus/concentration systems, or any state where accumulated
/// stress inhibits performance.
///
/// `tic(amount)` increases pressure. Fires `just_seized` on first reaching
/// `max_pressure`. No-op when disabled or already seized.
///
/// `compose(amount)` decreases pressure. Fires `just_composed` on first
/// returning to 0. No-op when disabled or already composed.
///
/// `tick(_dt)` clears `just_seized` and `just_composed`.
///
/// `is_seized()` returns `pressure >= max_pressure && enabled`.
///
/// `is_composed()` returns `pressure == 0.0` (not gated by `enabled`).
///
/// `pressure_fraction()` returns `(pressure / max_pressure).clamp(0, 1)`.
///
/// `effective_output(base)` returns `base * (1.0 - pressure_fraction())`
/// when enabled; `0.0` when disabled. Output is highest when calm and
/// collapses entirely under full pressure.
///
/// Default: `new(100.0)` — starts composed, no pressure.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yips {
    pub pressure: f32,
    pub max_pressure: f32,
    pub just_seized: bool,
    pub just_composed: bool,
    pub enabled: bool,
}

impl Yips {
    pub fn new(max_pressure: f32) -> Self {
        Self {
            pressure: 0.0,
            max_pressure: max_pressure.max(0.1),
            just_seized: false,
            just_composed: false,
            enabled: true,
        }
    }

    /// Increase pressure. Fires `just_seized` on first reaching
    /// `max_pressure`. No-op when disabled or already seized.
    pub fn tic(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pressure >= self.max_pressure {
            return;
        }
        self.pressure = (self.pressure + amount).min(self.max_pressure);
        if self.pressure >= self.max_pressure {
            self.just_seized = true;
        }
    }

    /// Decrease pressure. Fires `just_composed` on first returning to 0.
    /// No-op when disabled or already composed.
    pub fn compose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pressure <= 0.0 {
            return;
        }
        self.pressure = (self.pressure - amount).max(0.0);
        if self.pressure <= 0.0 {
            self.just_composed = true;
        }
    }

    /// Advance one frame: clear `just_seized` and `just_composed`.
    pub fn tick(&mut self, _dt: f32) {
        self.just_seized = false;
        self.just_composed = false;
    }

    /// `true` when pressure is at maximum and component is enabled.
    pub fn is_seized(&self) -> bool {
        self.pressure >= self.max_pressure && self.enabled
    }

    /// `true` when pressure is 0 (not gated by `enabled`).
    pub fn is_composed(&self) -> bool {
        self.pressure == 0.0
    }

    /// Fraction of maximum pressure [0.0, 1.0].
    pub fn pressure_fraction(&self) -> f32 {
        (self.pressure / self.max_pressure).clamp(0.0, 1.0)
    }

    /// Returns `base * (1.0 - pressure_fraction())` when enabled; `0.0` when
    /// disabled. Output degrades as pressure mounts.
    pub fn effective_output(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * (1.0 - self.pressure_fraction())
    }
}

impl Default for Yips {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yips {
        Yips::new(100.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_composed() {
        let y = y();
        assert_eq!(y.pressure, 0.0);
        assert!(y.is_composed());
        assert!(!y.is_seized());
    }

    #[test]
    fn new_clamps_max_pressure() {
        let y = Yips::new(-5.0);
        assert!((y.max_pressure - 0.1).abs() < 1e-5);
    }

    #[test]
    fn default_max_pressure_is_hundred() {
        assert!((Yips::default().max_pressure - 100.0).abs() < 1e-5);
    }

    // --- tic ---

    #[test]
    fn tic_increases_pressure() {
        let mut y = y();
        y.tic(30.0);
        assert!((y.pressure - 30.0).abs() < 1e-4);
    }

    #[test]
    fn tic_clamps_at_max() {
        let mut y = y();
        y.tic(200.0);
        assert!((y.pressure - 100.0).abs() < 1e-5);
    }

    #[test]
    fn tic_fires_just_seized_at_max() {
        let mut y = y();
        y.tic(100.0);
        assert!(y.just_seized);
        assert!(y.is_seized());
    }

    #[test]
    fn tic_no_refire_when_already_seized() {
        let mut y = y();
        y.tic(100.0);
        y.tick(0.016);
        y.tic(10.0); // already at max
        assert!(!y.just_seized);
    }

    #[test]
    fn tic_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.tic(50.0);
        assert_eq!(y.pressure, 0.0);
    }

    #[test]
    fn tic_no_op_for_zero_amount() {
        let mut y = y();
        y.tic(0.0);
        assert_eq!(y.pressure, 0.0);
    }

    // --- compose ---

    #[test]
    fn compose_decreases_pressure() {
        let mut y = y();
        y.tic(80.0);
        y.tick(0.016);
        y.compose(30.0);
        assert!((y.pressure - 50.0).abs() < 1e-4);
    }

    #[test]
    fn compose_clamps_at_zero() {
        let mut y = y();
        y.tic(40.0);
        y.tick(0.016);
        y.compose(100.0);
        assert_eq!(y.pressure, 0.0);
    }

    #[test]
    fn compose_fires_just_composed_at_zero() {
        let mut y = y();
        y.tic(40.0);
        y.tick(0.016);
        y.compose(40.0);
        assert!(y.just_composed);
        assert!(y.is_composed());
    }

    #[test]
    fn compose_no_refire_when_already_zero() {
        let mut y = y();
        y.compose(10.0); // already 0
        assert!(!y.just_composed);
    }

    #[test]
    fn compose_no_op_when_disabled() {
        let mut y = y();
        y.tic(50.0);
        y.enabled = false;
        y.compose(20.0);
        assert!((y.pressure - 50.0).abs() < 1e-4);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_seized() {
        let mut y = y();
        y.tic(100.0);
        y.tick(0.016);
        assert!(!y.just_seized);
    }

    #[test]
    fn tick_clears_just_composed() {
        let mut y = y();
        y.tic(40.0);
        y.compose(40.0);
        y.tick(0.016);
        assert!(!y.just_composed);
    }

    #[test]
    fn tick_does_not_change_pressure() {
        let mut y = y();
        y.tic(60.0);
        y.tick(1000.0);
        assert!((y.pressure - 60.0).abs() < 1e-5);
    }

    // --- is_seized / is_composed ---

    #[test]
    fn is_seized_false_below_max() {
        let mut y = y();
        y.tic(50.0);
        assert!(!y.is_seized());
    }

    #[test]
    fn is_seized_false_when_disabled() {
        let mut y = y();
        y.tic(100.0);
        y.enabled = false;
        assert!(!y.is_seized());
    }

    #[test]
    fn is_composed_true_at_zero() {
        assert!(y().is_composed());
    }

    #[test]
    fn is_composed_true_even_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_composed()); // not gated
    }

    #[test]
    fn is_composed_false_with_pressure() {
        let mut y = y();
        y.tic(10.0);
        assert!(!y.is_composed());
    }

    // --- fractions / effective_output ---

    #[test]
    fn pressure_fraction_zero_when_composed() {
        assert_eq!(y().pressure_fraction(), 0.0);
    }

    #[test]
    fn pressure_fraction_half_at_midpoint() {
        let mut y = y();
        y.tic(50.0);
        assert!((y.pressure_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn pressure_fraction_one_when_seized() {
        let mut y = y();
        y.tic(100.0);
        assert!((y.pressure_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_full_when_composed() {
        assert!((y().effective_output(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_half_under_half_pressure() {
        let mut y = y();
        y.tic(50.0);
        assert!((y.effective_output(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_zero_when_seized() {
        let mut y = y();
        y.tic(100.0);
        assert_eq!(y.effective_output(100.0), 0.0);
    }

    #[test]
    fn effective_output_zero_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert_eq!(y.effective_output(100.0), 0.0);
    }
}
