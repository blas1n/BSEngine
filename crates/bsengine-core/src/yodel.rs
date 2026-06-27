use bevy_ecs::prelude::Component;

/// Echo-delay tracker. A `yodel()` call emits a signal that returns as an
/// echo after `echo_delay` seconds. While waiting, `is_listening()` is true.
/// When the echo arrives, `just_echoed` fires for one frame. Models sonar,
/// echolocation, time-delayed responses, signal round-trips, or any mechanic
/// where an emitted action has a deferred return effect.
///
/// `yodel()` begins a new countdown (`echo_remaining = echo_delay`,
/// `awaiting_echo = true`, `just_yodeled = true`). Interrupts any in-progress
/// countdown. No-op when disabled.
///
/// `tick(dt)` clears `just_yodeled` and `just_echoed`, then (when
/// `awaiting_echo` and enabled) drains `echo_remaining`. When
/// `echo_remaining` reaches 0, fires `just_echoed` and clears
/// `awaiting_echo`.
///
/// `is_listening()` returns `awaiting_echo && enabled`.
///
/// `echo_progress()` returns the fraction of the delay elapsed [0.0, 1.0];
/// `0.0` when `echo_delay` is 0 or no yodel is in flight.
///
/// `effective_signal(base)` returns `base` when `just_echoed`; `0.0`
/// otherwise.
///
/// Default: `new(1.0)`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yodel {
    pub echo_delay: f32,
    pub echo_remaining: f32,
    pub awaiting_echo: bool,
    pub just_yodeled: bool,
    pub just_echoed: bool,
    pub enabled: bool,
}

impl Yodel {
    pub fn new(echo_delay: f32) -> Self {
        Self {
            echo_delay: echo_delay.max(0.0),
            echo_remaining: 0.0,
            awaiting_echo: false,
            just_yodeled: false,
            just_echoed: false,
            enabled: true,
        }
    }

    /// Begin countdown. Interrupts any in-progress echo. No-op when disabled.
    pub fn yodel(&mut self) {
        if !self.enabled {
            return;
        }
        self.echo_remaining = self.echo_delay;
        self.awaiting_echo = true;
        self.just_yodeled = true;
    }

    /// Advance one frame: clear flags, drain echo countdown when active.
    /// Fires `just_echoed` when countdown reaches 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_yodeled = false;
        self.just_echoed = false;
        if self.enabled && self.awaiting_echo {
            if self.echo_remaining > 0.0 {
                self.echo_remaining = (self.echo_remaining - dt).max(0.0);
            }
            if self.echo_remaining == 0.0 {
                self.awaiting_echo = false;
                self.just_echoed = true;
            }
        }
    }

    /// `true` while waiting for the echo and component is enabled.
    pub fn is_listening(&self) -> bool {
        self.awaiting_echo && self.enabled
    }

    /// Fraction of `echo_delay` elapsed [0.0, 1.0]. `0.0` when idle or
    /// `echo_delay` is 0.
    pub fn echo_progress(&self) -> f32 {
        if !self.awaiting_echo || self.echo_delay <= 0.0 {
            return 0.0;
        }
        let elapsed = self.echo_delay - self.echo_remaining;
        (elapsed / self.echo_delay).clamp(0.0, 1.0)
    }

    /// Returns `base` when the echo just arrived this frame; `0.0` otherwise.
    pub fn effective_signal(&self, base: f32) -> f32 {
        if self.just_echoed {
            base
        } else {
            0.0
        }
    }
}

impl Default for Yodel {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yodel {
        Yodel::new(1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_idle() {
        let y = y();
        assert_eq!(y.echo_delay, 1.0);
        assert_eq!(y.echo_remaining, 0.0);
        assert!(!y.awaiting_echo);
        assert!(!y.is_listening());
    }

    #[test]
    fn new_clamps_negative_delay() {
        let y = Yodel::new(-1.0);
        assert_eq!(y.echo_delay, 0.0);
    }

    #[test]
    fn default_delay_is_one() {
        assert!((Yodel::default().echo_delay - 1.0).abs() < 1e-5);
    }

    // --- yodel ---

    #[test]
    fn yodel_sets_awaiting_echo() {
        let mut y = y();
        y.yodel();
        assert!(y.awaiting_echo);
        assert!(y.is_listening());
    }

    #[test]
    fn yodel_sets_just_yodeled() {
        let mut y = y();
        y.yodel();
        assert!(y.just_yodeled);
    }

    #[test]
    fn yodel_sets_echo_remaining_to_delay() {
        let mut y = y();
        y.yodel();
        assert!((y.echo_remaining - 1.0).abs() < 1e-5);
    }

    #[test]
    fn yodel_interrupts_in_progress_countdown() {
        let mut y = y();
        y.yodel();
        y.tick(0.5); // half elapsed
        y.yodel(); // restart
        assert!((y.echo_remaining - 1.0).abs() < 1e-5);
    }

    #[test]
    fn yodel_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.yodel();
        assert!(!y.awaiting_echo);
        assert!(!y.just_yodeled);
    }

    // --- tick countdown ---

    #[test]
    fn tick_drains_echo_remaining() {
        let mut y = y();
        y.yodel();
        y.tick(0.4);
        assert!((y.echo_remaining - 0.6).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_echoed_at_zero() {
        let mut y = y();
        y.yodel();
        y.tick(1.0);
        assert!(y.just_echoed);
        assert!(!y.awaiting_echo);
    }

    #[test]
    fn tick_fires_just_echoed_over_delay() {
        let mut y = y();
        y.yodel();
        y.tick(2.0); // overshoots
        assert!(y.just_echoed);
        assert_eq!(y.echo_remaining, 0.0);
    }

    #[test]
    fn tick_clears_just_yodeled() {
        let mut y = y();
        y.yodel();
        y.tick(0.1);
        assert!(!y.just_yodeled);
    }

    #[test]
    fn tick_clears_just_echoed_next_frame() {
        let mut y = y();
        y.yodel();
        y.tick(1.0);
        assert!(y.just_echoed);
        y.tick(0.016);
        assert!(!y.just_echoed);
    }

    #[test]
    fn tick_no_effect_when_idle() {
        let mut y = y();
        y.tick(10.0);
        assert!(!y.just_echoed);
        assert!(!y.awaiting_echo);
    }

    #[test]
    fn tick_does_not_drain_when_disabled() {
        let mut y = y();
        y.yodel();
        y.enabled = false;
        y.tick(1.0);
        assert!(!y.just_echoed);
        assert!((y.echo_remaining - 1.0).abs() < 1e-5);
    }

    // --- zero delay ---

    #[test]
    fn zero_delay_echoes_on_first_tick() {
        let mut y = Yodel::new(0.0);
        y.yodel();
        assert!(y.awaiting_echo); // set by yodel
        y.tick(0.016);
        assert!(y.just_echoed); // immediately on first tick
    }

    // --- is_listening ---

    #[test]
    fn is_listening_true_while_counting_down() {
        let mut y = y();
        y.yodel();
        y.tick(0.3);
        assert!(y.is_listening());
    }

    #[test]
    fn is_listening_false_after_echo() {
        let mut y = y();
        y.yodel();
        y.tick(1.0);
        assert!(!y.is_listening());
    }

    #[test]
    fn is_listening_false_when_disabled() {
        let mut y = y();
        y.yodel();
        y.enabled = false;
        assert!(!y.is_listening());
    }

    // --- echo_progress ---

    #[test]
    fn echo_progress_zero_when_idle() {
        assert_eq!(y().echo_progress(), 0.0);
    }

    #[test]
    fn echo_progress_zero_at_start_of_countdown() {
        let mut y = y();
        y.yodel();
        assert!((y.echo_progress() - 0.0).abs() < 1e-4);
    }

    #[test]
    fn echo_progress_half_at_midpoint() {
        let mut y = y();
        y.yodel();
        y.tick(0.5);
        assert!((y.echo_progress() - 0.5).abs() < 1e-4);
    }

    // --- effective_signal ---

    #[test]
    fn effective_signal_zero_when_idle() {
        assert_eq!(y().effective_signal(100.0), 0.0);
    }

    #[test]
    fn effective_signal_base_when_just_echoed() {
        let mut y = y();
        y.yodel();
        y.tick(1.0);
        assert!((y.effective_signal(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_signal_zero_before_echo() {
        let mut y = y();
        y.yodel();
        y.tick(0.5);
        assert_eq!(y.effective_signal(100.0), 0.0);
    }
}
