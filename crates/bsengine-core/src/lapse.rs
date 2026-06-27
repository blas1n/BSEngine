use bevy_ecs::prelude::Component;

/// Involuntary periodic concentration failure. The entity cycles through a
/// focused phase (counting down `interval_timer`) and a lapsing phase
/// (counting down `duration_timer`). While `is_lapsing()`, systems can use
/// this signal to suppress abilities or apply accuracy penalties.
///
/// `tick(dt)` clears one-frame flags at the start. If lapsing, it counts down
/// `duration_timer`; when it expires the entity returns to the focused phase
/// (`interval_timer` resets to `interval`) and fires `just_focused`. If not
/// lapsing, it counts down `interval_timer`; when it expires the entity enters
/// the lapse phase (`duration_timer` resets to `lapse_duration`) and fires
/// `just_lapsed`. No-op when disabled.
///
/// `reset()` immediately ends any active lapse and resets both timers, returning
/// the entity to the start of the focused phase.
///
/// `is_lapsing()` returns `lapsing && enabled`.
///
/// `focus_fraction()` returns the fraction of the focused phase completed:
/// `1.0 - (interval_timer / interval).clamp(0, 1)`. Returns 0.0 while lapsing.
///
/// Distinct from `Cooldown` (single ability cooldown), `Interrupt` (reaction
/// to being interrupted by an external force), `Exhaustion` (cumulative
/// fatigue from exertion), and `Stun` (external immobilisation event): Lapse
/// is an **involuntary periodic concentration failure** — purely timer-driven,
/// independent of actions, representing a natural mental rhythm that cycles
/// regardless of what the entity is doing.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Lapse {
    pub lapsing: bool,
    /// Countdown to the next lapse when focused. Resets to `interval` at
    /// the end of each lapse.
    pub interval_timer: f32,
    /// Countdown during the active lapse. Resets to `lapse_duration` at
    /// the start of each lapse.
    pub duration_timer: f32,
    /// Seconds between lapses. Clamped ≥ 1.0.
    pub interval: f32,
    /// Seconds each lapse lasts. Clamped ≥ 0.0.
    pub lapse_duration: f32,
    pub just_lapsed: bool,
    pub just_focused: bool,
    pub enabled: bool,
}

impl Lapse {
    pub fn new(interval: f32, lapse_duration: f32) -> Self {
        let clamped_interval = interval.max(1.0);
        Self {
            lapsing: false,
            interval_timer: clamped_interval,
            duration_timer: 0.0,
            interval: clamped_interval,
            lapse_duration: lapse_duration.max(0.0),
            just_lapsed: false,
            just_focused: false,
            enabled: true,
        }
    }

    /// Advance timers. Clears one-frame flags at start. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_lapsed = false;
        self.just_focused = false;

        if !self.enabled {
            return;
        }

        if self.lapsing {
            self.duration_timer -= dt;
            if self.duration_timer <= 0.0 {
                self.duration_timer = 0.0;
                self.lapsing = false;
                self.interval_timer = self.interval;
                self.just_focused = true;
            }
        } else {
            self.interval_timer -= dt;
            if self.interval_timer <= 0.0 {
                self.interval_timer = 0.0;
                self.lapsing = true;
                self.duration_timer = self.lapse_duration;
                self.just_lapsed = true;
            }
        }
    }

    /// Immediately end any active lapse and restart the focused phase from
    /// the beginning.
    pub fn reset(&mut self) {
        self.lapsing = false;
        self.interval_timer = self.interval;
        self.duration_timer = 0.0;
        self.just_lapsed = false;
        self.just_focused = false;
    }

    /// `true` when the entity is in the lapsing phase and the component is
    /// enabled.
    pub fn is_lapsing(&self) -> bool {
        self.lapsing && self.enabled
    }

    /// Fraction of the focused phase that has elapsed since the last lapse
    /// ended [0.0 = just refocused, 1.0 = about to lapse again].
    /// Returns 0.0 while lapsing.
    pub fn focus_fraction(&self) -> f32 {
        if self.lapsing || self.interval <= 0.0 {
            return 0.0;
        }
        (1.0 - self.interval_timer / self.interval).clamp(0.0, 1.0)
    }
}

impl Default for Lapse {
    fn default() -> Self {
        Self::new(10.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_focused() {
        let l = Lapse::new(10.0, 2.0);
        assert!(!l.lapsing);
        assert!(!l.is_lapsing());
    }

    #[test]
    fn new_interval_timer_equals_interval() {
        let l = Lapse::new(10.0, 2.0);
        assert!((l.interval_timer - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_counts_down_interval_timer() {
        let mut l = Lapse::new(10.0, 2.0);
        l.tick(3.0);
        assert!((l.interval_timer - 7.0).abs() < 1e-5);
    }

    #[test]
    fn tick_triggers_lapse_when_interval_expires() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0);
        assert!(l.lapsing);
        assert!(l.just_lapsed);
        assert!(l.is_lapsing());
    }

    #[test]
    fn tick_resets_duration_timer_on_lapse_start() {
        let mut l = Lapse::new(5.0, 3.0);
        l.tick(5.0);
        assert!((l.duration_timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_counts_down_duration_timer_while_lapsing() {
        let mut l = Lapse::new(5.0, 4.0);
        l.tick(5.0); // enter lapse, duration_timer = 4.0
        l.tick(1.5);
        assert!((l.duration_timer - 2.5).abs() < 1e-5);
    }

    #[test]
    fn tick_exits_lapse_when_duration_expires() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0); // enter lapse
        l.tick(2.0); // lapse expires
        assert!(!l.lapsing);
        assert!(l.just_focused);
        assert!(!l.is_lapsing());
    }

    #[test]
    fn tick_resets_interval_timer_after_lapse() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0);
        l.tick(2.0);
        assert!((l.interval_timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_lapsed() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0); // triggers
        l.tick(0.016);
        assert!(!l.just_lapsed);
    }

    #[test]
    fn tick_clears_just_focused() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0); // lapse
        l.tick(2.0); // refocus
        l.tick(0.016);
        assert!(!l.just_focused);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut l = Lapse::new(5.0, 2.0);
        l.enabled = false;
        l.tick(100.0);
        assert!(!l.lapsing);
        assert!((l.interval_timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn is_lapsing_false_when_disabled() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0); // lapse
        l.enabled = false;
        assert!(!l.is_lapsing());
    }

    #[test]
    fn reset_ends_lapse() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0); // lapse
        l.reset();
        assert!(!l.lapsing);
        assert!((l.interval_timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn reset_while_focused_restarts_countdown() {
        let mut l = Lapse::new(10.0, 2.0);
        l.tick(4.0); // partially through interval
        l.reset();
        assert!((l.interval_timer - 10.0).abs() < 1e-5);
    }

    #[test]
    fn reset_clears_flags() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0); // just_lapsed = true
        l.reset();
        assert!(!l.just_lapsed);
        assert!(!l.just_focused);
    }

    #[test]
    fn focus_fraction_at_start() {
        let l = Lapse::new(10.0, 2.0);
        assert_eq!(l.focus_fraction(), 0.0);
    }

    #[test]
    fn focus_fraction_at_half_interval() {
        let mut l = Lapse::new(10.0, 2.0);
        l.tick(5.0);
        assert!((l.focus_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn focus_fraction_near_one_before_lapse() {
        let mut l = Lapse::new(10.0, 2.0);
        l.tick(9.5);
        assert!((l.focus_fraction() - 0.95).abs() < 1e-3);
    }

    #[test]
    fn focus_fraction_zero_while_lapsing() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0); // lapse
        assert_eq!(l.focus_fraction(), 0.0);
    }

    #[test]
    fn focus_fraction_resets_after_refocus() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0); // lapse
        l.tick(2.0); // refocus
        assert_eq!(l.focus_fraction(), 0.0);
    }

    #[test]
    fn cyclic_lapse_and_refocus() {
        let mut l = Lapse::new(5.0, 2.0);
        l.tick(5.0); // → lapse
        assert!(l.just_lapsed);
        l.tick(2.0); // → focused
        assert!(l.just_focused);
        l.tick(5.0); // → lapse again
        assert!(l.just_lapsed);
    }

    #[test]
    fn interval_clamped_to_one() {
        let l = Lapse::new(0.5, 2.0);
        assert!((l.interval - 1.0).abs() < 1e-5);
    }

    #[test]
    fn lapse_duration_clamped_to_zero() {
        let l = Lapse::new(10.0, -1.0);
        assert_eq!(l.lapse_duration, 0.0);
    }

    #[test]
    fn zero_lapse_duration_exits_immediately() {
        let mut l = Lapse::new(5.0, 0.0);
        l.tick(5.0); // lapse with 0 duration
                     // duration_timer starts at 0, immediately expires — need another tick
        l.tick(0.016); // should refocus
                       // After setting duration_timer=0, the next tick should see it <= 0 and exit
                       // Actually: entering lapse sets duration_timer = 0.0 and just_lapsed = true.
                       // On the same tick, we set lapsing=true and duration_timer=0.
                       // The current tick already moved past the lapse entry — we need one more tick
                       // to detect expiry.
        assert!(!l.lapsing);
    }
}
