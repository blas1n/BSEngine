use bevy_ecs::prelude::Component;

/// High-water mark tracker. Records the peak value ever observed via
/// `observe()` and exposes how far the current value has fallen from that
/// peak. Models high-score records, peak-damage tracking, endurance milestones,
/// or any "fall-from-grace" mechanic.
///
/// `observe(value)` updates `current_value`. If `value > peak_value`, also
/// updates `peak_value` and fires `just_peaked`. No-op when disabled.
///
/// `reset_peak()` resets `peak_value` to `current_value`. No-op when
/// disabled.
///
/// `tick(_dt)` clears `just_peaked` only. No time-based logic.
///
/// `drawdown()` returns `(peak_value - current_value).max(0.0)`.
///
/// `drawdown_fraction()` returns `drawdown() / peak_value` when
/// `peak_value > 0.0`; `0.0` otherwise.
///
/// `effective_legacy(base)` returns `base * (1.0 - drawdown_fraction())`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(0.0)` — peak and current both start at 0.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yore {
    /// Current observed value.
    pub current_value: f32,
    /// Highest value ever observed since creation or last `reset_peak()`.
    pub peak_value: f32,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Yore {
    pub fn new(initial: f32) -> Self {
        Self {
            current_value: initial,
            peak_value: initial,
            just_peaked: false,
            enabled: true,
        }
    }

    /// Feed a new observation. Updates current; updates peak and fires
    /// `just_peaked` when a new high is reached. No-op when disabled.
    pub fn observe(&mut self, value: f32) {
        if !self.enabled {
            return;
        }
        self.current_value = value;
        if value > self.peak_value {
            self.peak_value = value;
            self.just_peaked = true;
        }
    }

    /// Reset `peak_value` to `current_value`. No-op when disabled.
    pub fn reset_peak(&mut self) {
        if !self.enabled {
            return;
        }
        self.peak_value = self.current_value;
    }

    /// Advance one frame: clear `just_peaked`. No time-based logic.
    pub fn tick(&mut self, _dt: f32) {
        self.just_peaked = false;
    }

    /// Distance the current value has fallen below peak, floored at 0.
    pub fn drawdown(&self) -> f32 {
        (self.peak_value - self.current_value).max(0.0)
    }

    /// Drawdown as a fraction of peak [0.0, 1.0]. `0.0` when peak is 0.
    pub fn drawdown_fraction(&self) -> f32 {
        if self.peak_value <= 0.0 {
            return 0.0;
        }
        (self.drawdown() / self.peak_value).clamp(0.0, 1.0)
    }

    /// Returns `base * (1.0 - drawdown_fraction())` when enabled — full base
    /// at peak, scales down as current falls below peak; `0.0` when disabled.
    pub fn effective_legacy(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * (1.0 - self.drawdown_fraction())
    }
}

impl Default for Yore {
    fn default() -> Self {
        Self::new(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yore {
        Yore::new(0.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_at_initial() {
        let y = Yore::new(50.0);
        assert!((y.current_value - 50.0).abs() < 1e-5);
        assert!((y.peak_value - 50.0).abs() < 1e-5);
        assert!(!y.just_peaked);
    }

    #[test]
    fn default_starts_at_zero() {
        let y = y();
        assert_eq!(y.current_value, 0.0);
        assert_eq!(y.peak_value, 0.0);
    }

    // --- observe ---

    #[test]
    fn observe_updates_current() {
        let mut y = y();
        y.observe(42.0);
        assert!((y.current_value - 42.0).abs() < 1e-5);
    }

    #[test]
    fn observe_updates_peak_when_higher() {
        let mut y = y();
        y.observe(100.0);
        assert!((y.peak_value - 100.0).abs() < 1e-5);
    }

    #[test]
    fn observe_fires_just_peaked_on_new_high() {
        let mut y = y();
        y.observe(10.0);
        assert!(y.just_peaked);
    }

    #[test]
    fn observe_does_not_update_peak_when_lower() {
        let mut y = y();
        y.observe(100.0);
        y.tick(0.016);
        y.observe(50.0);
        assert!((y.peak_value - 100.0).abs() < 1e-5);
        assert!(!y.just_peaked);
    }

    #[test]
    fn observe_does_not_fire_just_peaked_at_equal() {
        let mut y = y();
        y.observe(10.0);
        y.tick(0.016);
        y.observe(10.0); // equal, not strictly greater
        assert!(!y.just_peaked);
    }

    #[test]
    fn observe_no_op_when_disabled() {
        let mut y = Yore::new(0.0);
        y.enabled = false;
        y.observe(50.0);
        assert_eq!(y.current_value, 0.0);
        assert!(!y.just_peaked);
    }

    // --- reset_peak ---

    #[test]
    fn reset_peak_sets_peak_to_current() {
        let mut y = Yore::new(0.0);
        y.observe(100.0);
        y.observe(60.0); // current=60, peak=100
        y.reset_peak();
        assert!((y.peak_value - 60.0).abs() < 1e-5);
    }

    #[test]
    fn reset_peak_no_op_when_disabled() {
        let mut y = Yore::new(0.0);
        y.observe(100.0);
        y.observe(60.0);
        y.enabled = false;
        y.reset_peak();
        assert!((y.peak_value - 100.0).abs() < 1e-5);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_peaked() {
        let mut y = y();
        y.observe(10.0);
        y.tick(0.016);
        assert!(!y.just_peaked);
    }

    #[test]
    fn tick_does_not_change_values() {
        let mut y = y();
        y.observe(10.0);
        y.observe(5.0);
        y.tick(1000.0);
        assert!((y.current_value - 5.0).abs() < 1e-5);
        assert!((y.peak_value - 10.0).abs() < 1e-5);
    }

    // --- drawdown ---

    #[test]
    fn drawdown_zero_at_peak() {
        let mut y = y();
        y.observe(50.0);
        assert_eq!(y.drawdown(), 0.0);
    }

    #[test]
    fn drawdown_positive_below_peak() {
        let mut y = y();
        y.observe(100.0);
        y.observe(60.0);
        assert!((y.drawdown() - 40.0).abs() < 1e-4);
    }

    #[test]
    fn drawdown_floors_at_zero() {
        let mut y = y();
        y.observe(10.0); // both current and peak = 10
        assert_eq!(y.drawdown(), 0.0);
    }

    // --- drawdown_fraction ---

    #[test]
    fn drawdown_fraction_zero_at_peak() {
        let mut y = y();
        y.observe(100.0);
        assert_eq!(y.drawdown_fraction(), 0.0);
    }

    #[test]
    fn drawdown_fraction_at_half_peak() {
        let mut y = Yore::new(0.0);
        y.observe(100.0);
        y.observe(50.0); // 50/100 drawdown = 0.5
        assert!((y.drawdown_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn drawdown_fraction_zero_when_peak_zero() {
        let y = y(); // peak=0
        assert_eq!(y.drawdown_fraction(), 0.0);
    }

    // --- effective_legacy ---

    #[test]
    fn effective_legacy_full_at_peak() {
        let mut y = Yore::new(0.0);
        y.observe(100.0);
        assert!((y.effective_legacy(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_legacy_half_at_half_peak() {
        let mut y = Yore::new(0.0);
        y.observe(100.0);
        y.observe(50.0); // drawdown_fraction=0.5 → base*(1-0.5)=50
        assert!((y.effective_legacy(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_legacy_zero_when_disabled() {
        let mut y = Yore::new(0.0);
        y.observe(100.0);
        y.enabled = false;
        assert_eq!(y.effective_legacy(100.0), 0.0);
    }

    // --- peak recovery cycle ---

    #[test]
    fn reset_and_new_peak_cycle() {
        let mut y = Yore::new(0.0);
        y.observe(100.0);
        y.observe(50.0);
        y.reset_peak(); // peak now 50
        y.observe(80.0); // new peak
        assert!(y.just_peaked);
        assert!((y.peak_value - 80.0).abs() < 1e-5);
    }
}
