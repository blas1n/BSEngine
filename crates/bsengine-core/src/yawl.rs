use bevy_ecs::prelude::Component;

/// Two-phase oscillation tracker. Alternates between a low phase (`false`)
/// and a high phase (`true`) via `toggle()`. Counts complete cycles
/// (low → high → low). Models pendulums, alternating signals, flag-wave
/// mechanics, breathing patterns, or any binary-oscillating effect.
///
/// `toggle()` flips `phase` and sets `just_toggled`. When phase returns to
/// `false` (completing a cycle), `cycle_count` increments and `just_cycled`
/// fires. No-op when disabled.
///
/// `tick(_dt)` clears `just_toggled` and `just_cycled` only.
///
/// `is_high()` returns `phase && enabled`.
///
/// `is_low()` returns `!phase` (not gated by `enabled`).
///
/// `effective_signal(base)` returns `base` when `is_high()`; `0.0` otherwise.
///
/// Default: `new()` — starts low, no cycles completed.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yawl {
    /// Current oscillation phase: `false` = low, `true` = high.
    pub phase: bool,
    /// Number of complete low → high → low cycles.
    pub cycle_count: u32,
    pub just_toggled: bool,
    pub just_cycled: bool,
    pub enabled: bool,
}

impl Yawl {
    pub fn new() -> Self {
        Self {
            phase: false,
            cycle_count: 0,
            just_toggled: false,
            just_cycled: false,
            enabled: true,
        }
    }

    /// Flip phase. Fires `just_toggled`. When returning to low, increments
    /// `cycle_count` and fires `just_cycled`. No-op when disabled.
    pub fn toggle(&mut self) {
        if !self.enabled {
            return;
        }
        self.phase = !self.phase;
        self.just_toggled = true;
        if !self.phase {
            self.cycle_count += 1;
            self.just_cycled = true;
        }
    }

    /// Advance one frame: clear `just_toggled` and `just_cycled` only.
    pub fn tick(&mut self, _dt: f32) {
        self.just_toggled = false;
        self.just_cycled = false;
    }

    /// `true` when phase is high and component is enabled.
    pub fn is_high(&self) -> bool {
        self.phase && self.enabled
    }

    /// `true` when phase is low (not gated by `enabled`).
    pub fn is_low(&self) -> bool {
        !self.phase
    }

    /// Returns `base` when high and enabled; `0.0` otherwise.
    pub fn effective_signal(&self, base: f32) -> f32 {
        if self.is_high() {
            base
        } else {
            0.0
        }
    }
}

impl Default for Yawl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yawl {
        Yawl::new()
    }

    // --- construction ---

    #[test]
    fn new_starts_low() {
        let y = y();
        assert!(!y.phase);
        assert_eq!(y.cycle_count, 0);
        assert!(!y.just_toggled);
        assert!(!y.just_cycled);
        assert!(y.is_low());
        assert!(!y.is_high());
    }

    #[test]
    fn default_same_as_new() {
        let y = Yawl::default();
        assert!(!y.phase);
        assert_eq!(y.cycle_count, 0);
    }

    // --- toggle ---

    #[test]
    fn toggle_flips_phase_to_high() {
        let mut y = y();
        y.toggle();
        assert!(y.phase);
    }

    #[test]
    fn toggle_fires_just_toggled() {
        let mut y = y();
        y.toggle();
        assert!(y.just_toggled);
    }

    #[test]
    fn toggle_does_not_fire_just_cycled_on_going_high() {
        let mut y = y();
        y.toggle(); // low → high
        assert!(!y.just_cycled);
    }

    #[test]
    fn toggle_flips_phase_back_to_low() {
        let mut y = y();
        y.toggle(); // → high
        y.tick(0.016);
        y.toggle(); // → low
        assert!(!y.phase);
    }

    #[test]
    fn toggle_increments_cycle_count_on_returning_to_low() {
        let mut y = y();
        y.toggle(); // → high
        y.tick(0.016);
        y.toggle(); // → low, cycle complete
        assert_eq!(y.cycle_count, 1);
        assert!(y.just_cycled);
    }

    #[test]
    fn toggle_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.toggle();
        assert!(!y.phase);
        assert!(!y.just_toggled);
        assert_eq!(y.cycle_count, 0);
    }

    #[test]
    fn multiple_cycles_counted_correctly() {
        let mut y = y();
        for _ in 0..5 {
            y.toggle(); // → high
            y.tick(0.016);
            y.toggle(); // → low
            y.tick(0.016);
        }
        assert_eq!(y.cycle_count, 5);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_toggled() {
        let mut y = y();
        y.toggle();
        y.tick(0.016);
        assert!(!y.just_toggled);
    }

    #[test]
    fn tick_clears_just_cycled() {
        let mut y = y();
        y.toggle();
        y.toggle();
        y.tick(0.016);
        assert!(!y.just_cycled);
    }

    #[test]
    fn tick_does_not_change_phase_or_count() {
        let mut y = y();
        y.toggle();
        y.toggle();
        let count = y.cycle_count;
        let phase = y.phase;
        y.tick(1000.0);
        assert_eq!(y.cycle_count, count);
        assert_eq!(y.phase, phase);
    }

    // --- is_high / is_low ---

    #[test]
    fn is_high_false_when_low() {
        assert!(!y().is_high());
    }

    #[test]
    fn is_high_true_when_high_and_enabled() {
        let mut y = y();
        y.toggle();
        assert!(y.is_high());
    }

    #[test]
    fn is_high_false_when_high_but_disabled() {
        let mut y = y();
        y.toggle();
        y.enabled = false;
        assert!(!y.is_high());
    }

    #[test]
    fn is_low_true_when_low() {
        assert!(y().is_low());
    }

    #[test]
    fn is_low_false_when_high() {
        let mut y = y();
        y.toggle();
        assert!(!y.is_low());
    }

    #[test]
    fn is_low_true_even_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_low()); // not gated by enabled
    }

    // --- effective_signal ---

    #[test]
    fn effective_signal_zero_when_low() {
        assert_eq!(y().effective_signal(100.0), 0.0);
    }

    #[test]
    fn effective_signal_base_when_high() {
        let mut y = y();
        y.toggle();
        assert!((y.effective_signal(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_signal_zero_when_disabled_and_high() {
        let mut y = y();
        y.toggle();
        y.enabled = false;
        assert_eq!(y.effective_signal(100.0), 0.0);
    }
}
