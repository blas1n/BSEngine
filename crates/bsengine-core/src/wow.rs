use bevy_ecs::prelude::Component;

/// Spectacle-gate cooldown. Fires a one-frame `just_wowed` flag and holds
/// `wow_timer` for `wow_duration` seconds; cannot re-trigger until the timer
/// expires. Models rare spectacular events (big kills, jackpots, story beats)
/// that must not spam — once triggered, the "wow window" provides a freshness
/// bonus that decays to neutral at expiry.
///
/// Unlike `Cooldown` (which gates a recurring action), Wow gates **how often
/// you can be amazed**: the timer is the wow itself, not a delay before
/// enabling something else. Unlike `Zest` or `Woo` (replenishable
/// accumulators), Wow has no build-up — it's binary: idle or active.
///
/// `trigger()` fires `just_wowed` and resets `wow_timer` to `wow_duration`.
/// No-op when already wowing or disabled.
///
/// `tick(dt)` clears one-frame flags first, then decrements `wow_timer` by
/// `dt` (floors at 0) when enabled. No-op (beyond flag clear) when disabled.
///
/// `is_wowing()` returns `wow_timer > 0.0 && enabled`.
///
/// `wow_fraction()` returns `(wow_timer / wow_duration).clamp(0.0, 1.0)` —
/// 1.0 immediately after trigger, fading to 0.0 at expiry.
///
/// `effective_wow(base)` returns `base * (1.0 + wow_fraction())` when enabled
/// — 2× immediately, decaying to 1× as the wow fades; `base` when disabled.
///
/// Default: `new(2.0)` — wow window lasts 2 seconds.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wow {
    /// Remaining wow time. Counts down to 0; 0 means idle.
    pub wow_timer: f32,
    /// Duration of each wow window in seconds. Clamped >= 0.1.
    pub wow_duration: f32,
    pub just_wowed: bool,
    pub enabled: bool,
}

impl Wow {
    pub fn new(wow_duration: f32) -> Self {
        Self {
            wow_timer: 0.0,
            wow_duration: wow_duration.max(0.1),
            just_wowed: false,
            enabled: true,
        }
    }

    /// Start a wow window. Fires `just_wowed` and sets timer to full
    /// duration. No-op when already wowing or disabled.
    pub fn trigger(&mut self) {
        if !self.enabled || self.wow_timer > 0.0 {
            return;
        }
        self.wow_timer = self.wow_duration;
        self.just_wowed = true;
    }

    /// Advance one frame: clear flags, then tick the timer down. No-op
    /// (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_wowed = false;

        if !self.enabled || self.wow_timer == 0.0 {
            return;
        }

        self.wow_timer = (self.wow_timer - dt).max(0.0);
    }

    /// `true` while the wow window is active and component is enabled.
    pub fn is_wowing(&self) -> bool {
        self.wow_timer > 0.0 && self.enabled
    }

    /// Wow freshness [0.0, 1.0]: 1.0 right after trigger, 0.0 at expiry.
    pub fn wow_fraction(&self) -> f32 {
        (self.wow_timer / self.wow_duration).clamp(0.0, 1.0)
    }

    /// Scale `base` by wow freshness. Returns `base * (1.0 + wow_fraction())`
    /// when enabled — 2× at trigger, 1× at expiry; `base` when disabled.
    pub fn effective_wow(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.wow_fraction())
    }
}

impl Default for Wow {
    fn default() -> Self {
        Self::new(2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wow {
        Wow::new(10.0) // 10s wow window
    }

    // --- construction ---

    #[test]
    fn new_starts_idle() {
        let w = w();
        assert_eq!(w.wow_timer, 0.0);
        assert!(!w.just_wowed);
        assert!(!w.is_wowing());
    }

    #[test]
    fn wow_duration_clamped_to_point_one() {
        let w = Wow::new(0.0);
        assert!((w.wow_duration - 0.1).abs() < 1e-5);
    }

    // --- trigger ---

    #[test]
    fn trigger_sets_timer_to_duration() {
        let mut w = w();
        w.trigger();
        assert!((w.wow_timer - 10.0).abs() < 1e-5);
    }

    #[test]
    fn trigger_fires_just_wowed() {
        let mut w = w();
        w.trigger();
        assert!(w.just_wowed);
    }

    #[test]
    fn trigger_activates_is_wowing() {
        let mut w = w();
        w.trigger();
        assert!(w.is_wowing());
    }

    #[test]
    fn trigger_no_op_while_already_wowing() {
        let mut w = w();
        w.trigger();
        w.tick(3.0); // timer=7, flags cleared
        w.trigger(); // no-op: still wowing
        assert!(!w.just_wowed);
        assert!((w.wow_timer - 7.0).abs() < 1e-4);
    }

    #[test]
    fn trigger_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.trigger();
        assert_eq!(w.wow_timer, 0.0);
        assert!(!w.just_wowed);
    }

    #[test]
    fn trigger_allowed_after_expiry() {
        let mut w = w();
        w.trigger();
        w.tick(10.0); // expires
        w.trigger(); // new window
        assert!(w.just_wowed);
        assert!((w.wow_timer - 10.0).abs() < 1e-5);
    }

    // --- tick ---

    #[test]
    fn tick_counts_down_timer() {
        let mut w = w();
        w.trigger(); // 10.0
        w.tick(3.0); // 7.0
        assert!((w.wow_timer - 7.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_timer_at_zero() {
        let mut w = w();
        w.trigger();
        w.tick(15.0); // over by 5
        assert_eq!(w.wow_timer, 0.0);
    }

    #[test]
    fn tick_clears_just_wowed_next_frame() {
        let mut w = w();
        w.trigger(); // just_wowed=true
        w.tick(0.016);
        assert!(!w.just_wowed);
    }

    #[test]
    fn tick_no_op_when_idle() {
        let mut w = w(); // timer=0
        w.tick(1.0);
        assert_eq!(w.wow_timer, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.trigger();
        w.enabled = false;
        let timer_before = w.wow_timer;
        w.tick(5.0);
        assert!((w.wow_timer - timer_before).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_wowed = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_wowed);
    }

    // --- is_wowing ---

    #[test]
    fn is_wowing_false_when_idle() {
        let w = w();
        assert!(!w.is_wowing());
    }

    #[test]
    fn is_wowing_true_while_active() {
        let mut w = w();
        w.trigger();
        w.tick(5.0); // 5.0 remaining
        assert!(w.is_wowing());
    }

    #[test]
    fn is_wowing_false_when_expired() {
        let mut w = w();
        w.trigger();
        w.tick(10.0);
        assert!(!w.is_wowing());
    }

    #[test]
    fn is_wowing_false_when_disabled() {
        let mut w = w();
        w.trigger();
        w.enabled = false;
        assert!(!w.is_wowing());
    }

    // --- wow_fraction ---

    #[test]
    fn wow_fraction_zero_when_idle() {
        let w = w();
        assert_eq!(w.wow_fraction(), 0.0);
    }

    #[test]
    fn wow_fraction_one_immediately_after_trigger() {
        let mut w = w();
        w.trigger(); // timer=10, duration=10 → 1.0
        assert!((w.wow_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn wow_fraction_half_at_midpoint() {
        let mut w = w(); // duration=10
        w.trigger();
        w.tick(5.0); // timer=5 → 5/10=0.5
        assert!((w.wow_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn wow_fraction_zero_when_expired() {
        let mut w = w();
        w.trigger();
        w.tick(10.0); // timer=0 → 0.0
        assert_eq!(w.wow_fraction(), 0.0);
    }

    // --- effective_wow ---

    #[test]
    fn effective_wow_doubled_immediately_after_trigger() {
        let mut w = w();
        w.trigger(); // fraction=1.0 → 100*(1+1)=200
        assert!((w.effective_wow(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_wow_at_half_fraction() {
        let mut w = w();
        w.trigger();
        w.tick(5.0); // fraction=0.5 → 100*(1+0.5)=150
        assert!((w.effective_wow(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_wow_passthrough_when_expired() {
        let mut w = w();
        w.trigger();
        w.tick(10.0); // fraction=0 → 100*(1+0)=100
        assert!((w.effective_wow(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_wow_passthrough_when_idle() {
        let w = w();
        assert!((w.effective_wow(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_wow_passthrough_when_disabled() {
        let mut w = w();
        w.trigger();
        w.enabled = false;
        assert!((w.effective_wow(100.0) - 100.0).abs() < 1e-4);
    }

    // --- re-trigger cycle ---

    #[test]
    fn can_trigger_again_after_expiry() {
        let mut w = w();
        w.trigger();
        w.tick(10.0); // expire
        w.tick(0.016); // clear flags
        w.trigger(); // new wow
        assert!(w.just_wowed);
        assert!((w.wow_timer - 10.0).abs() < 1e-5);
    }

    #[test]
    fn multiple_trigger_cycles_independent() {
        let mut w = w();
        for _ in 0..3 {
            w.trigger();
            w.tick(10.0); // expire
            w.tick(0.0); // clear flags
        }
        assert!(!w.is_wowing());
    }
}
