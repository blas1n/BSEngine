use bevy_ecs::prelude::Component;

/// Burst-limited emitter. Allows up to `burst_limit` activations in rapid
/// succession, then enforces a `cooldown` before the burst counter resets.
/// Models rapid-fire that exhausts itself, limited-use burst abilities, or
/// short-interval click detection with saturation.
///
/// `yip()` fires `just_yipped` and increments `burst_count`. Fires
/// `just_burst_out` and starts `cooldown_remaining` when the burst is
/// exhausted. No-op when the burst is full or disabled.
///
/// `tick(dt)` clears one-frame flags first, then if enabled drains
/// `cooldown_remaining`. Resets `burst_count` to 0 when it reaches 0.
///
/// `is_burst_full()` returns `burst_count >= burst_limit && enabled`.
///
/// `is_cooling_down()` returns `cooldown_remaining > 0.0 && enabled`.
///
/// `burst_fraction()` returns `(burst_count as f32 / burst_limit as f32).clamp(0.0, 1.0)`.
///
/// `effective_burst(base)` returns `base * burst_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(3, 2.0)` — 3-shot burst with 2-second cooldown.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yip {
    /// Maximum activations per burst. Clamped >= 1.
    pub burst_limit: u32,
    /// Current activations in this burst [0, burst_limit].
    pub burst_count: u32,
    /// Seconds to wait after a full burst before resetting. Clamped >= 0.
    pub cooldown: f32,
    /// Countdown remaining until burst resets.
    pub cooldown_remaining: f32,
    pub just_yipped: bool,
    pub just_burst_out: bool,
    pub enabled: bool,
}

impl Yip {
    pub fn new(burst_limit: u32, cooldown: f32) -> Self {
        Self {
            burst_limit: burst_limit.max(1),
            burst_count: 0,
            cooldown: cooldown.max(0.0),
            cooldown_remaining: 0.0,
            just_yipped: false,
            just_burst_out: false,
            enabled: true,
        }
    }

    /// Emit one activation. Fires `just_burst_out` when exhausting the burst.
    /// No-op when burst is full or disabled.
    pub fn yip(&mut self) {
        if !self.enabled || self.burst_count >= self.burst_limit {
            return;
        }
        self.burst_count += 1;
        self.just_yipped = true;
        if self.burst_count >= self.burst_limit {
            self.just_burst_out = true;
            if self.cooldown > 0.0 {
                self.cooldown_remaining = self.cooldown;
            } else {
                self.burst_count = 0;
            }
        }
    }

    /// Advance one frame: clear flags, then drain cooldown and reset burst.
    pub fn tick(&mut self, dt: f32) {
        self.just_yipped = false;
        self.just_burst_out = false;
        if !self.enabled {
            return;
        }
        if self.cooldown_remaining > 0.0 {
            self.cooldown_remaining = (self.cooldown_remaining - dt).max(0.0);
            if self.cooldown_remaining == 0.0 {
                self.burst_count = 0;
            }
        }
    }

    /// `true` when burst is exhausted and enabled.
    pub fn is_burst_full(&self) -> bool {
        self.burst_count >= self.burst_limit && self.enabled
    }

    /// `true` when cooling down after an exhausted burst and enabled.
    pub fn is_cooling_down(&self) -> bool {
        self.cooldown_remaining > 0.0 && self.enabled
    }

    /// Burst fill as a fraction of the limit [0.0, 1.0].
    pub fn burst_fraction(&self) -> f32 {
        (self.burst_count as f32 / self.burst_limit as f32).clamp(0.0, 1.0)
    }

    /// Returns `base * burst_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_burst(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.burst_fraction()
    }
}

impl Default for Yip {
    fn default() -> Self {
        Self::new(3, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yip {
        Yip::new(3, 2.0) // 3-shot burst, 2s cooldown
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let y = y();
        assert_eq!(y.burst_count, 0);
        assert_eq!(y.cooldown_remaining, 0.0);
        assert!(!y.just_yipped);
        assert!(!y.just_burst_out);
        assert!(!y.is_burst_full());
        assert!(!y.is_cooling_down());
    }

    #[test]
    fn burst_limit_clamped_to_one() {
        let y = Yip::new(0, 1.0);
        assert_eq!(y.burst_limit, 1);
    }

    #[test]
    fn cooldown_clamped_to_zero() {
        let y = Yip::new(3, -1.0);
        assert_eq!(y.cooldown, 0.0);
    }

    // --- yip ---

    #[test]
    fn yip_increments_burst_count() {
        let mut y = y();
        y.yip();
        assert_eq!(y.burst_count, 1);
    }

    #[test]
    fn yip_fires_just_yipped() {
        let mut y = y();
        y.yip();
        assert!(y.just_yipped);
    }

    #[test]
    fn yip_fires_just_burst_out_at_limit() {
        let mut y = y();
        y.yip();
        y.yip();
        y.yip(); // 3rd = limit
        assert!(y.just_burst_out);
        assert!(y.is_burst_full());
    }

    #[test]
    fn yip_starts_cooldown_at_limit() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        assert!((y.cooldown_remaining - 2.0).abs() < 1e-5);
    }

    #[test]
    fn yip_no_op_when_burst_full() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        y.tick(0.016); // clear flags
        y.yip();
        assert!(!y.just_yipped);
        assert_eq!(y.burst_count, 3);
    }

    #[test]
    fn yip_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.yip();
        assert_eq!(y.burst_count, 0);
        assert!(!y.just_yipped);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_yipped() {
        let mut y = y();
        y.yip();
        y.tick(0.016);
        assert!(!y.just_yipped);
    }

    #[test]
    fn tick_clears_just_burst_out() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        y.tick(0.016);
        assert!(!y.just_burst_out);
    }

    #[test]
    fn tick_drains_cooldown() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        y.tick(1.0);
        assert!((y.cooldown_remaining - 1.0).abs() < 1e-4);
    }

    #[test]
    fn tick_resets_burst_count_after_cooldown() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        y.tick(2.0);
        assert_eq!(y.burst_count, 0);
        assert_eq!(y.cooldown_remaining, 0.0);
    }

    #[test]
    fn tick_no_op_on_cooldown_when_disabled() {
        let mut y = y();
        y.cooldown_remaining = 1.0;
        y.enabled = false;
        y.tick(0.5);
        assert!((y.cooldown_remaining - 1.0).abs() < 1e-5);
    }

    // --- is_burst_full / is_cooling_down ---

    #[test]
    fn is_burst_full_false_below_limit() {
        let mut y = y();
        y.yip();
        y.yip();
        assert!(!y.is_burst_full());
    }

    #[test]
    fn is_burst_full_true_at_limit() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        assert!(y.is_burst_full());
    }

    #[test]
    fn is_burst_full_false_when_disabled() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        y.enabled = false;
        assert!(!y.is_burst_full());
    }

    #[test]
    fn is_cooling_down_true_after_burst() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        assert!(y.is_cooling_down());
    }

    #[test]
    fn is_cooling_down_false_after_cooldown_expires() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        y.tick(2.0);
        assert!(!y.is_cooling_down());
    }

    // --- burst_fraction ---

    #[test]
    fn burst_fraction_zero_when_empty() {
        assert_eq!(y().burst_fraction(), 0.0);
    }

    #[test]
    fn burst_fraction_at_one_third() {
        let mut y = y(); // limit=3
        y.yip();
        assert!((y.burst_fraction() - 1.0 / 3.0).abs() < 1e-4);
    }

    #[test]
    fn burst_fraction_one_at_limit() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        assert!((y.burst_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_burst ---

    #[test]
    fn effective_burst_zero_when_empty() {
        assert_eq!(y().effective_burst(100.0), 0.0);
    }

    #[test]
    fn effective_burst_scales_with_count() {
        let mut y = y();
        y.yip(); // 1/3
        assert!((y.effective_burst(90.0) - 30.0).abs() < 1e-3);
    }

    #[test]
    fn effective_burst_full_at_limit() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        assert!((y.effective_burst(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_burst_zero_when_disabled() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        y.enabled = false;
        assert_eq!(y.effective_burst(100.0), 0.0);
    }

    // --- re-burst cycle ---

    #[test]
    fn can_burst_again_after_cooldown() {
        let mut y = y();
        for _ in 0..3 {
            y.yip();
        }
        y.tick(2.0); // cooldown expires, burst_count=0
        y.yip();
        assert!(y.just_yipped);
        assert_eq!(y.burst_count, 1);
    }

    #[test]
    fn zero_cooldown_resets_immediately() {
        let mut y = Yip::new(2, 0.0);
        y.yip();
        y.yip(); // bursts out, cooldown=0
        y.tick(0.0); // reset immediately
        assert_eq!(y.burst_count, 0);
    }
}
