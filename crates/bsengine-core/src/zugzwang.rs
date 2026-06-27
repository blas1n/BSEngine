use bevy_ecs::prelude::Component;

/// Strategic-pressure tracker named after the chess concept of
/// compulsion to move. `pressure` builds via `press(amount)` and
/// intensifies passively at `compel_rate` per second in `tick(dt)`
/// or is relieved immediately via `relieve(amount)`.
///
/// Models decision-pressure saturation bars, obligation-force
/// escalation gauges, forced-choice tension fill levels,
/// move-compulsion intensity trackers, strategic-deadline pressure
/// meters, time-pressure accumulation bars, action-obligation
/// saturation indicators, mandatory-move tension trackers,
/// penalty-of-inaction fill levels, or any mechanic where the
/// slow accumulation of pressure makes every available option
/// worse than the last — leaving the player in the characteristic
/// chess position where the right move is whatever does the least
/// additional damage and the correct strategy is to have been
/// somewhere else entirely two turns ago.
///
/// `press(amount)` adds pressure; fires `just_compelled` when
/// first reaching `max_pressure`. No-op when disabled.
///
/// `relieve(amount)` reduces pressure immediately; fires
/// `just_stalled` when reaching 0. No-op when disabled or already
/// stalled.
///
/// `tick(dt)` clears both flags, then increases pressure by
/// `compel_rate * dt` (capped at `max_pressure`). Fires
/// `just_compelled` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_compelled()` returns `pressure >= max_pressure && enabled`.
///
/// `is_stalled()` returns `pressure == 0.0` (not gated by `enabled`).
///
/// `pressure_fraction()` returns `(pressure / max_pressure).clamp(0, 1)`.
///
/// `effective_urgency(scale)` returns `scale * pressure_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — compels at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zugzwang {
    pub pressure: f32,
    pub max_pressure: f32,
    pub compel_rate: f32,
    pub just_compelled: bool,
    pub just_stalled: bool,
    pub enabled: bool,
}

impl Zugzwang {
    pub fn new(max_pressure: f32, compel_rate: f32) -> Self {
        Self {
            pressure: 0.0,
            max_pressure: max_pressure.max(0.1),
            compel_rate: compel_rate.max(0.0),
            just_compelled: false,
            just_stalled: false,
            enabled: true,
        }
    }

    /// Add pressure; fires `just_compelled` when first reaching max.
    /// No-op when disabled.
    pub fn press(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.pressure < self.max_pressure;
        self.pressure = (self.pressure + amount).min(self.max_pressure);
        if was_below && self.pressure >= self.max_pressure {
            self.just_compelled = true;
        }
    }

    /// Reduce pressure; fires `just_stalled` when reaching 0.
    /// No-op when disabled or already stalled.
    pub fn relieve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pressure <= 0.0 {
            return;
        }
        self.pressure = (self.pressure - amount).max(0.0);
        if self.pressure <= 0.0 {
            self.just_stalled = true;
        }
    }

    /// Clear flags, then increase pressure by `compel_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_compelled = false;
        self.just_stalled = false;
        if self.enabled && self.compel_rate > 0.0 && self.pressure < self.max_pressure {
            let was_below = self.pressure < self.max_pressure;
            self.pressure = (self.pressure + self.compel_rate * dt).min(self.max_pressure);
            if was_below && self.pressure >= self.max_pressure {
                self.just_compelled = true;
            }
        }
    }

    /// `true` when pressure is at maximum and component is enabled.
    pub fn is_compelled(&self) -> bool {
        self.pressure >= self.max_pressure && self.enabled
    }

    /// `true` when pressure is 0 (not gated by `enabled`).
    pub fn is_stalled(&self) -> bool {
        self.pressure == 0.0
    }

    /// Fraction of maximum pressure [0.0, 1.0].
    pub fn pressure_fraction(&self) -> f32 {
        (self.pressure / self.max_pressure).clamp(0.0, 1.0)
    }

    /// Returns `scale * pressure_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_urgency(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.pressure_fraction()
    }
}

impl Default for Zugzwang {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zugzwang {
        Zugzwang::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_stalled() {
        let z = z();
        assert_eq!(z.pressure, 0.0);
        assert!(z.is_stalled());
        assert!(!z.is_compelled());
    }

    #[test]
    fn new_clamps_max_pressure() {
        let z = Zugzwang::new(-5.0, 1.5);
        assert!((z.max_pressure - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_compel_rate() {
        let z = Zugzwang::new(100.0, -1.5);
        assert_eq!(z.compel_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zugzwang::default();
        assert!((z.max_pressure - 100.0).abs() < 1e-5);
        assert!((z.compel_rate - 1.5).abs() < 1e-5);
    }

    // --- press ---

    #[test]
    fn press_adds_pressure() {
        let mut z = z();
        z.press(40.0);
        assert!((z.pressure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn press_clamps_at_max() {
        let mut z = z();
        z.press(200.0);
        assert!((z.pressure - 100.0).abs() < 1e-3);
    }

    #[test]
    fn press_fires_just_compelled_at_max() {
        let mut z = z();
        z.press(100.0);
        assert!(z.just_compelled);
        assert!(z.is_compelled());
    }

    #[test]
    fn press_no_just_compelled_when_already_at_max() {
        let mut z = z();
        z.pressure = 100.0;
        z.press(10.0);
        assert!(!z.just_compelled);
    }

    #[test]
    fn press_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.press(50.0);
        assert_eq!(z.pressure, 0.0);
    }

    #[test]
    fn press_no_op_when_amount_zero() {
        let mut z = z();
        z.press(0.0);
        assert_eq!(z.pressure, 0.0);
    }

    // --- relieve ---

    #[test]
    fn relieve_reduces_pressure() {
        let mut z = z();
        z.pressure = 60.0;
        z.relieve(20.0);
        assert!((z.pressure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn relieve_clamps_at_zero() {
        let mut z = z();
        z.pressure = 30.0;
        z.relieve(200.0);
        assert_eq!(z.pressure, 0.0);
    }

    #[test]
    fn relieve_fires_just_stalled_at_zero() {
        let mut z = z();
        z.pressure = 30.0;
        z.relieve(30.0);
        assert!(z.just_stalled);
    }

    #[test]
    fn relieve_no_op_when_already_stalled() {
        let mut z = z();
        z.relieve(10.0);
        assert!(!z.just_stalled);
    }

    #[test]
    fn relieve_no_op_when_disabled() {
        let mut z = z();
        z.pressure = 50.0;
        z.enabled = false;
        z.relieve(50.0);
        assert!((z.pressure - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_pressure() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.pressure - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_compelled_on_build_to_max() {
        let mut z = Zugzwang::new(100.0, 200.0);
        z.pressure = 95.0;
        z.tick(1.0);
        assert!(z.just_compelled);
        assert!(z.is_compelled());
    }

    #[test]
    fn tick_no_build_when_already_compelled() {
        let mut z = z();
        z.pressure = 100.0;
        z.tick(1.0);
        assert!(!z.just_compelled);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut z = Zugzwang::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.pressure, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.pressure, 0.0);
    }

    #[test]
    fn tick_clears_just_compelled() {
        let mut z = Zugzwang::new(100.0, 200.0);
        z.pressure = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_compelled);
    }

    #[test]
    fn tick_clears_just_stalled() {
        let mut z = z();
        z.pressure = 10.0;
        z.relieve(10.0);
        z.tick(0.016);
        assert!(!z.just_stalled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.pressure - 9.0).abs() < 1e-3);
    }

    // --- is_compelled / is_stalled ---

    #[test]
    fn is_compelled_false_when_disabled() {
        let mut z = z();
        z.pressure = 100.0;
        z.enabled = false;
        assert!(!z.is_compelled());
    }

    #[test]
    fn is_stalled_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_stalled());
    }

    // --- pressure_fraction / effective_urgency ---

    #[test]
    fn pressure_fraction_zero_when_stalled() {
        assert_eq!(z().pressure_fraction(), 0.0);
    }

    #[test]
    fn pressure_fraction_half_at_midpoint() {
        let mut z = z();
        z.pressure = 50.0;
        assert!((z.pressure_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_urgency_zero_when_stalled() {
        assert_eq!(z().effective_urgency(100.0), 0.0);
    }

    #[test]
    fn effective_urgency_scales_with_pressure() {
        let mut z = z();
        z.pressure = 75.0;
        assert!((z.effective_urgency(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_urgency_zero_when_disabled() {
        let mut z = z();
        z.pressure = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_urgency(100.0), 0.0);
    }
}
