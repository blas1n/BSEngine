use bevy_ecs::prelude::Component;

/// Reaction-window tracker for dodge / parry timing. When an incoming attack
/// is telegraphed, the combat system calls `trigger(duration)` to open a brief
/// evasion window. The player or AI must then call `evade()` before the window
/// closes to register a successful dodge.
///
/// `trigger(duration)` opens an evasion window for `duration` seconds
/// (high-watermark: only replaces the current timer when `duration > timer`).
/// Fires `just_triggered` on the inactive → active transition. No-op when
/// disabled or `duration ≤ 0`.
///
/// `evade()` registers a dodge attempt. Sets `just_evaded` and returns `true`
/// when a window is active; returns `false` otherwise (no double-dodge reward
/// if the window was already consumed via `just_evaded` — the flag clears on
/// the next `tick()`).
///
/// `tick(dt)` clears one-frame flags at the start, then counts down the timer.
/// Sets `just_missed` when the window expires without a successful `evade()`.
///
/// `has_window()` returns `timer > 0.0 && enabled`.
///
/// `window_fraction(original_duration)` returns the fraction of the window
/// remaining: `(timer / original_duration).clamp(0, 1)`. Returns 0.0 when
/// inactive or `original_duration ≤ 0`.
///
/// Distinct from `Dodge` (a movement-based directional evade), `Parry`
/// (a block that reflects damage or staggers the attacker), and `Deflect`
/// (passive projectile redirection): Reflex is a **timed reaction window** —
/// the telegraph opens the window and the player has a brief deadline to
/// respond; missing the window fires `just_missed` for a near-miss feedback.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Reflex {
    pub timer: f32,
    pub just_triggered: bool,
    pub just_evaded: bool,
    /// Fires when the window expires naturally without a successful `evade()`.
    pub just_missed: bool,
    pub enabled: bool,
}

impl Reflex {
    pub fn new() -> Self {
        Self {
            timer: 0.0,
            just_triggered: false,
            just_evaded: false,
            just_missed: false,
            enabled: true,
        }
    }

    /// Open or extend the evasion window for `duration` seconds.
    /// High-watermark: only replaces the timer when `duration > timer`. Fires
    /// `just_triggered` on the inactive → active transition. No-op when
    /// disabled or `duration ≤ 0`.
    pub fn trigger(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_inactive = !self.has_window();
            self.timer = duration;
            if was_inactive {
                self.just_triggered = true;
            }
        }
    }

    /// Attempt a dodge. Returns `true` and sets `just_evaded` when a window is
    /// active; returns `false` when the window is closed or the component is
    /// disabled.
    pub fn evade(&mut self) -> bool {
        if !self.has_window() {
            return false;
        }
        self.just_evaded = true;
        true
    }

    /// Advance the reflex timer. Clears one-frame flags at start. Sets
    /// `just_missed` when the timer expires naturally without `evade()` having
    /// been called this window.
    pub fn tick(&mut self, dt: f32) {
        let evaded_this_window = self.just_evaded;
        self.just_triggered = false;
        self.just_evaded = false;
        self.just_missed = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                if !evaded_this_window {
                    self.just_missed = true;
                }
            }
        }
    }

    /// `true` when the evasion window is open and the component is enabled.
    pub fn has_window(&self) -> bool {
        self.timer > 0.0 && self.enabled
    }

    /// Fraction of the current window remaining relative to `original_duration`.
    /// Returns `(timer / original_duration).clamp(0, 1)`; 0.0 when inactive or
    /// `original_duration ≤ 0`.
    pub fn window_fraction(&self, original_duration: f32) -> f32 {
        if self.timer <= 0.0 || original_duration <= 0.0 {
            return 0.0;
        }
        (self.timer / original_duration).clamp(0.0, 1.0)
    }
}

impl Default for Reflex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let r = Reflex::new();
        assert!(!r.has_window());
        assert_eq!(r.timer, 0.0);
    }

    #[test]
    fn trigger_opens_window() {
        let mut r = Reflex::new();
        r.trigger(0.5);
        assert!(r.has_window());
        assert!(r.just_triggered);
    }

    #[test]
    fn trigger_extends_on_longer_duration() {
        let mut r = Reflex::new();
        r.trigger(0.3);
        r.tick(0.016);
        r.trigger(1.0);
        assert!((r.timer - 1.0).abs() < 1e-4);
    }

    #[test]
    fn trigger_no_extend_on_shorter_duration() {
        let mut r = Reflex::new();
        r.trigger(1.0);
        r.trigger(0.3);
        assert!((r.timer - 1.0).abs() < 1e-4);
    }

    #[test]
    fn just_triggered_not_set_on_extend() {
        let mut r = Reflex::new();
        r.trigger(0.3);
        r.tick(0.016);
        r.trigger(1.0);
        assert!(!r.just_triggered);
    }

    #[test]
    fn trigger_no_op_when_disabled() {
        let mut r = Reflex::new();
        r.enabled = false;
        r.trigger(1.0);
        assert!(!r.has_window());
    }

    #[test]
    fn trigger_no_op_at_zero_duration() {
        let mut r = Reflex::new();
        r.trigger(0.0);
        assert!(!r.has_window());
    }

    #[test]
    fn evade_returns_true_with_window() {
        let mut r = Reflex::new();
        r.trigger(1.0);
        assert!(r.evade());
        assert!(r.just_evaded);
    }

    #[test]
    fn evade_returns_false_without_window() {
        let mut r = Reflex::new();
        assert!(!r.evade());
        assert!(!r.just_evaded);
    }

    #[test]
    fn evade_returns_false_when_disabled() {
        let mut r = Reflex::new();
        r.timer = 1.0;
        r.enabled = false;
        assert!(!r.evade());
    }

    #[test]
    fn tick_expires_naturally() {
        let mut r = Reflex::new();
        r.trigger(0.5);
        r.tick(0.016); // clear flags
        r.tick(1.0); // expire — no evade this window
        assert!(!r.has_window());
        assert!(r.just_missed);
    }

    #[test]
    fn tick_no_just_missed_when_evaded() {
        let mut r = Reflex::new();
        r.trigger(0.5);
        r.evade();
        r.tick(1.0); // expires but evade happened
        assert!(!r.just_missed);
    }

    #[test]
    fn tick_clears_just_triggered() {
        let mut r = Reflex::new();
        r.trigger(1.0);
        r.tick(0.016);
        assert!(!r.just_triggered);
    }

    #[test]
    fn tick_clears_just_evaded() {
        let mut r = Reflex::new();
        r.trigger(1.0);
        r.evade();
        r.tick(0.016);
        assert!(!r.just_evaded);
    }

    #[test]
    fn tick_clears_just_missed() {
        let mut r = Reflex::new();
        r.trigger(0.5);
        r.tick(0.016); // clears flags
        r.tick(1.0); // fires just_missed
        r.tick(0.016); // clears just_missed
        assert!(!r.just_missed);
    }

    #[test]
    fn has_window_false_when_disabled() {
        let mut r = Reflex::new();
        r.timer = 1.0;
        r.enabled = false;
        assert!(!r.has_window());
    }

    #[test]
    fn window_fraction_at_half() {
        let mut r = Reflex::new();
        r.trigger(2.0);
        r.tick(0.016); // advance slightly
        r.trigger(1.0); // set to exactly half of 2.0
        assert!((r.window_fraction(2.0) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn window_fraction_zero_when_inactive() {
        let r = Reflex::new();
        assert_eq!(r.window_fraction(2.0), 0.0);
    }

    #[test]
    fn window_fraction_zero_at_zero_original() {
        let mut r = Reflex::new();
        r.trigger(1.0);
        assert_eq!(r.window_fraction(0.0), 0.0);
    }

    #[test]
    fn re_triggers_after_expiry() {
        let mut r = Reflex::new();
        r.trigger(0.5);
        r.tick(0.016);
        r.tick(1.0); // expires
        r.tick(0.016);
        r.trigger(1.0); // new window
        assert!(r.just_triggered);
    }

    #[test]
    fn just_missed_only_on_natural_expiry() {
        let mut r = Reflex::new();
        r.trigger(0.5);
        // let it expire without evading — should fire just_missed
        r.tick(0.016);
        r.tick(1.0);
        assert!(r.just_missed);
    }
}
