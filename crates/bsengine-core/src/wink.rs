use bevy_ecs::prelude::Component;

/// Timed vulnerability window. Represents a brief opening in an entity's
/// defense (or a brief moment of heightened attack opportunity) that closes
/// automatically after a fixed duration.
///
/// `open()` starts the window: sets `active` to `true` and resets the
/// countdown to `wink_duration`. No-op if already active or disabled.
///
/// `tick(dt)` clears `just_closed` first, then if active: decrements
/// `wink_timer` by `dt`. When `wink_timer` reaches 0, sets `active` to
/// `false` and fires `just_closed`. No-op (beyond flag clear) when disabled.
///
/// `is_open()` returns `active && enabled`.
///
/// `time_fraction()` returns `(wink_timer / wink_duration).clamp(0.0, 1.0)`
/// while open; returns `0.0` when closed.
///
/// `effective_bonus(base)` returns
/// `base * (1.0 + wink_power * time_fraction())` when enabled and open;
/// returns `base` unchanged otherwise. The bonus is greatest immediately
/// after `open()` and fades linearly to 0 as the window closes.
///
/// Distinct from `Flash` (instant one-frame burst), `Phase` (immunity window),
/// and `Blink` (teleport dash): Wink models a **countdown vulnerability
/// window** — brief, timed, and automatically expiring.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wink {
    /// Remaining countdown [0.0, wink_duration]. 0 when closed.
    pub wink_timer: f32,
    /// Full window duration in seconds. Clamped >= 0.01.
    pub wink_duration: f32,
    /// Bonus multiplier while window is open. Clamped >= 0.0.
    pub wink_power: f32,
    pub active: bool,
    pub just_closed: bool,
    pub enabled: bool,
}

impl Wink {
    pub fn new(wink_duration: f32, wink_power: f32) -> Self {
        Self {
            wink_timer: 0.0,
            wink_duration: wink_duration.max(0.01),
            wink_power: wink_power.max(0.0),
            active: false,
            just_closed: false,
            enabled: true,
        }
    }

    /// Open the vulnerability window. Resets the countdown to `wink_duration`.
    /// No-op if already active or disabled.
    pub fn open(&mut self) {
        if !self.enabled || self.active {
            return;
        }
        self.active = true;
        self.wink_timer = self.wink_duration;
    }

    /// Advance one frame: clear `just_closed`, then count down while active.
    /// No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_closed = false;

        if !self.enabled {
            return;
        }

        if self.active {
            self.wink_timer = (self.wink_timer - dt).max(0.0);
            if self.wink_timer <= 0.0 {
                self.active = false;
                self.just_closed = true;
            }
        }
    }

    /// `true` when the window is open and component is enabled.
    pub fn is_open(&self) -> bool {
        self.active && self.enabled
    }

    /// Fraction of window duration remaining [0.0, 1.0]. 0.0 when closed.
    pub fn time_fraction(&self) -> f32 {
        if !self.active {
            return 0.0;
        }
        (self.wink_timer / self.wink_duration).clamp(0.0, 1.0)
    }

    /// Scale `base` by window bonus. Returns
    /// `base * (1.0 + wink_power * time_fraction())` when enabled and open;
    /// `base` otherwise.
    pub fn effective_bonus(&self, base: f32) -> f32 {
        if !self.enabled || !self.active {
            return base;
        }
        base * (1.0 + self.wink_power * self.time_fraction())
    }
}

impl Default for Wink {
    fn default() -> Self {
        Self::new(2.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wink {
        Wink::new(2.0, 1.0)
    }

    #[test]
    fn new_starts_closed() {
        let w = w();
        assert!(!w.active);
        assert_eq!(w.wink_timer, 0.0);
        assert!(!w.just_closed);
        assert!(!w.is_open());
    }

    #[test]
    fn open_activates_window() {
        let mut w = w();
        w.open();
        assert!(w.active);
        assert!((w.wink_timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn open_no_op_when_already_active() {
        let mut w = w();
        w.open();
        w.tick(0.5); // 2.0 - 0.5 = 1.5
        w.open(); // no-op
        assert!((w.wink_timer - 1.5).abs() < 1e-4);
    }

    #[test]
    fn open_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.open();
        assert!(!w.active);
    }

    #[test]
    fn tick_counts_down_timer() {
        let mut w = w(); // duration=2.0
        w.open();
        w.tick(0.5);
        assert!((w.wink_timer - 1.5).abs() < 1e-4);
    }

    #[test]
    fn tick_closes_window_at_zero() {
        let mut w = w();
        w.open();
        w.tick(2.0);
        assert!(!w.active);
        assert_eq!(w.wink_timer, 0.0);
    }

    #[test]
    fn tick_fires_just_closed_when_expiring() {
        let mut w = w();
        w.open();
        w.tick(2.0);
        assert!(w.just_closed);
    }

    #[test]
    fn tick_fires_just_closed_even_overshoot() {
        let mut w = w();
        w.open();
        w.tick(100.0); // far overshoots
        assert!(w.just_closed);
        assert!(!w.active);
    }

    #[test]
    fn tick_does_not_close_before_duration() {
        let mut w = w();
        w.open();
        w.tick(1.0); // half-elapsed, still open
        assert!(w.active);
        assert!(!w.just_closed);
    }

    #[test]
    fn tick_no_countdown_when_inactive() {
        let mut w = w();
        w.tick(5.0);
        assert_eq!(w.wink_timer, 0.0);
        assert!(!w.just_closed);
    }

    #[test]
    fn just_closed_clears_next_tick() {
        let mut w = w();
        w.open();
        w.tick(2.0); // closes
        w.tick(0.016); // clears
        assert!(!w.just_closed);
    }

    #[test]
    fn tick_no_op_when_disabled_no_countdown() {
        let mut w = w();
        w.open();
        w.enabled = false;
        let timer_before = w.wink_timer;
        w.tick(1.0);
        assert!((w.wink_timer - timer_before).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_closed = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_closed);
    }

    #[test]
    fn is_open_true_when_active() {
        let mut w = w();
        w.open();
        assert!(w.is_open());
    }

    #[test]
    fn is_open_false_when_inactive() {
        let w = w();
        assert!(!w.is_open());
    }

    #[test]
    fn is_open_false_when_disabled() {
        let mut w = w();
        w.open();
        w.enabled = false;
        assert!(!w.is_open());
    }

    #[test]
    fn is_open_false_after_window_expires() {
        let mut w = w();
        w.open();
        w.tick(2.0);
        assert!(!w.is_open());
    }

    #[test]
    fn time_fraction_one_immediately_after_open() {
        let mut w = w();
        w.open();
        assert!((w.time_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn time_fraction_half_at_midpoint() {
        let mut w = w(); // duration=2.0
        w.open();
        w.tick(1.0); // 1.0 left of 2.0
        assert!((w.time_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn time_fraction_zero_when_closed() {
        let w = w();
        assert_eq!(w.time_fraction(), 0.0);
    }

    #[test]
    fn time_fraction_zero_after_expiry() {
        let mut w = w();
        w.open();
        w.tick(2.0);
        assert_eq!(w.time_fraction(), 0.0);
    }

    #[test]
    fn effective_bonus_base_when_closed() {
        let w = w(); // closed
        assert!((w.effective_bonus(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_bonus_full_immediately_after_open() {
        let mut w = Wink::new(2.0, 1.0);
        w.open(); // fraction=1.0
                  // 100 * (1 + 1.0 * 1.0) = 200
        assert!((w.effective_bonus(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_bonus_half_at_midpoint() {
        let mut w = Wink::new(2.0, 1.0);
        w.open();
        w.tick(1.0); // fraction=0.5
                     // 100 * (1 + 1.0 * 0.5) = 150
        assert!((w.effective_bonus(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_bonus_passthrough_when_disabled() {
        let mut w = w();
        w.open();
        w.enabled = false;
        assert!((w.effective_bonus(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_bonus_passthrough_when_closed_after_expiry() {
        let mut w = w();
        w.open();
        w.tick(2.0);
        assert!((w.effective_bonus(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn wink_duration_clamped_to_min() {
        let w = Wink::new(0.0, 1.0);
        assert!((w.wink_duration - 0.01).abs() < 1e-5);
    }

    #[test]
    fn wink_power_clamped_to_zero() {
        let w = Wink::new(2.0, -1.0);
        assert_eq!(w.wink_power, 0.0);
    }

    #[test]
    fn reopen_after_close() {
        let mut w = w();
        w.open();
        w.tick(2.0); // closed
        w.tick(0.016); // clear flags
        w.open(); // reopen
        assert!(w.active);
        assert!((w.wink_timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn successive_ticks_count_down_correctly() {
        let mut w = w(); // duration=2.0
        w.open();
        w.tick(0.5); // 1.5
        w.tick(0.5); // 1.0
        w.tick(0.5); // 0.5
        assert!((w.wink_timer - 0.5).abs() < 1e-4);
        assert!(w.active);
        w.tick(0.5); // 0.0 → closed
        assert!(!w.active);
        assert!(w.just_closed);
    }

    #[test]
    fn effective_bonus_with_high_power() {
        let mut w = Wink::new(2.0, 3.0);
        w.open(); // fraction=1.0
                  // 100 * (1 + 3.0 * 1.0) = 400
        assert!((w.effective_bonus(100.0) - 400.0).abs() < 1e-3);
    }

    #[test]
    fn window_countdown_matches_duration_precisely() {
        let mut w = Wink::new(1.0, 1.0);
        w.open();
        w.tick(0.25);
        w.tick(0.25);
        w.tick(0.25);
        w.tick(0.25); // should close exactly at 1.0s
        assert!(!w.active);
        assert!(w.just_closed);
    }
}
