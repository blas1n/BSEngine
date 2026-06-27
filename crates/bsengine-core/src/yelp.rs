use bevy_ecs::prelude::Component;

/// Integer hit-stack accumulator. Models the build-up of reactive distress
/// as discrete count events rather than a continuous float level.
///
/// Each call to `yelp()` increments `yelps` by 1 (capped at `max_yelps`) and
/// resets `decay_timer` to `decay_interval`. No-op when at cap or disabled.
/// Fires `just_yelped` on success; also fires `just_peaked` the first time
/// `yelps` reaches `max_yelps`.
///
/// `tick(dt)` clears one-frame flags first. If `yelps > 0`: decrements
/// `decay_timer` by `dt`; when it reaches 0 or below, decrements `yelps` by
/// 1, fires `just_calmed`, and resets the timer for the next stack. No-op
/// (beyond flag clear) when disabled.
///
/// `is_frantic()` returns `yelps >= max_yelps && enabled`.
///
/// `stack_fraction()` returns `(yelps as f32 / max_yelps as f32).clamp(0.0, 1.0)`.
///
/// `effective_scramble(base)` returns `base * (1.0 + stack_fraction())` when
/// enabled — scramble scales with how many stacks are active; returns `base`
/// unchanged when disabled.
///
/// Distinct from float accumulators (`Whelm`, `Wrest`, `Yowl`): stacks are
/// discrete and each call to `yelp()` represents a recognizable event (a hit,
/// a scare, a taunt). The timer resets on each `yelp()`, giving recently-hit
/// entities a refreshed decay window — the classic "stack-refresh" pattern.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yelp {
    /// Current stack count [0, max_yelps].
    pub yelps: u32,
    /// Maximum stacks. Clamped >= 1.
    pub max_yelps: u32,
    /// Time remaining until the next stack decays [0.0, decay_interval].
    pub decay_timer: f32,
    /// Duration each stack lasts after the last `yelp()`. Clamped >= 0.1.
    pub decay_interval: f32,
    pub just_yelped: bool,
    pub just_peaked: bool,
    pub just_calmed: bool,
    pub enabled: bool,
}

impl Yelp {
    pub fn new(max_yelps: u32, decay_interval: f32) -> Self {
        Self {
            yelps: 0,
            max_yelps: max_yelps.max(1),
            decay_timer: 0.0,
            decay_interval: decay_interval.max(0.1),
            just_yelped: false,
            just_peaked: false,
            just_calmed: false,
            enabled: true,
        }
    }

    /// Add one stack and refresh the decay timer. No-op when at cap or
    /// disabled. Fires `just_yelped`; fires `just_peaked` when hitting cap.
    pub fn yelp(&mut self) {
        if !self.enabled || self.yelps >= self.max_yelps {
            return;
        }
        self.yelps += 1;
        self.decay_timer = self.decay_interval;
        self.just_yelped = true;
        if self.yelps >= self.max_yelps {
            self.just_peaked = true;
        }
    }

    /// Advance one frame: clear flags, then tick down the decay timer.
    /// Loses one stack when the timer expires; resets the timer for the next.
    /// No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_yelped = false;
        self.just_peaked = false;
        self.just_calmed = false;

        if !self.enabled {
            return;
        }

        if self.yelps > 0 {
            self.decay_timer -= dt;
            if self.decay_timer <= 0.0 {
                self.yelps -= 1;
                self.just_calmed = true;
                if self.yelps > 0 {
                    self.decay_timer = self.decay_interval;
                }
            }
        }
    }

    /// `true` when stacks are at maximum and component is enabled.
    pub fn is_frantic(&self) -> bool {
        self.yelps >= self.max_yelps && self.enabled
    }

    /// Stack count as a fraction of maximum [0.0, 1.0].
    pub fn stack_fraction(&self) -> f32 {
        (self.yelps as f32 / self.max_yelps as f32).clamp(0.0, 1.0)
    }

    /// Scale `base` by active stack fraction. Returns
    /// `base * (1.0 + stack_fraction())` when enabled; `base` otherwise.
    pub fn effective_scramble(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.stack_fraction())
    }
}

impl Default for Yelp {
    fn default() -> Self {
        Self::new(5, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yelp {
        Yelp::new(5, 3.0)
    }

    #[test]
    fn new_starts_empty() {
        let y = y();
        assert_eq!(y.yelps, 0);
        assert!(!y.just_yelped);
        assert!(!y.just_peaked);
        assert!(!y.just_calmed);
        assert!(!y.is_frantic());
    }

    #[test]
    fn yelp_increments_stack() {
        let mut y = y();
        y.yelp();
        assert_eq!(y.yelps, 1);
    }

    #[test]
    fn yelp_fires_just_yelped() {
        let mut y = y();
        y.yelp();
        assert!(y.just_yelped);
    }

    #[test]
    fn yelp_resets_decay_timer() {
        let mut y = y(); // decay_interval=3.0
        y.yelp();
        assert!((y.decay_timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn yelp_fires_just_peaked_at_cap() {
        let mut y = y(); // max_yelps=5
        for _ in 0..5 {
            y.yelp();
        }
        assert!(y.just_peaked);
        assert_eq!(y.yelps, 5);
    }

    #[test]
    fn yelp_no_op_at_cap() {
        let mut y = y();
        for _ in 0..5 {
            y.yelp();
        }
        y.just_peaked = false;
        y.just_yelped = false;
        y.yelp(); // should no-op
        assert!(!y.just_yelped);
        assert_eq!(y.yelps, 5);
    }

    #[test]
    fn yelp_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.yelp();
        assert_eq!(y.yelps, 0);
        assert!(!y.just_yelped);
    }

    #[test]
    fn yelp_not_peaked_below_cap() {
        let mut y = y(); // max_yelps=5
        y.yelp(); // 1
        y.yelp(); // 2
        assert!(!y.just_peaked);
    }

    #[test]
    fn yelp_refreshes_timer_mid_count() {
        let mut y = y(); // decay_interval=3.0
        y.yelp(); // 1
        y.tick(2.0); // timer: 3.0-2.0=1.0
        y.yelp(); // refresh → 3.0 again
        assert!((y.decay_timer - 3.0).abs() < 1e-5);
        assert_eq!(y.yelps, 2);
    }

    #[test]
    fn tick_clears_flags() {
        let mut y = y();
        y.yelp();
        y.tick(0.016);
        assert!(!y.just_yelped);
        assert!(!y.just_peaked);
    }

    #[test]
    fn tick_decrements_timer() {
        let mut y = y(); // decay_interval=3.0
        y.yelp();
        y.tick(1.0); // 2.0
        assert!((y.decay_timer - 2.0).abs() < 1e-4);
        assert_eq!(y.yelps, 1); // not decayed yet
    }

    #[test]
    fn tick_decays_stack_when_timer_expires() {
        let mut y = y(); // decay_interval=3.0
        y.yelp(); // 1
        y.tick(3.0); // expires exactly
        assert_eq!(y.yelps, 0);
        assert!(y.just_calmed);
    }

    #[test]
    fn tick_decays_stack_past_zero() {
        let mut y = y();
        y.yelp(); // 1
        y.tick(100.0); // way past
        assert_eq!(y.yelps, 0);
        assert!(y.just_calmed);
    }

    #[test]
    fn tick_fires_just_calmed_on_decay() {
        let mut y = y();
        y.yelp();
        y.tick(3.0);
        assert!(y.just_calmed);
    }

    #[test]
    fn tick_clears_just_calmed_next_frame() {
        let mut y = y();
        y.yelp();
        y.tick(3.0); // calmed
        y.tick(0.016);
        assert!(!y.just_calmed);
    }

    #[test]
    fn tick_resets_timer_for_next_stack_after_decay() {
        let mut y = y(); // decay_interval=3.0
        y.yelp(); // 1
        y.yelp(); // 2
        y.tick(3.0); // first stack decays → yelps=1, timer resets to 3.0
        assert_eq!(y.yelps, 1);
        assert!((y.decay_timer - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_op_when_yelps_zero() {
        let mut y = y();
        y.tick(10.0); // nothing to do
        assert_eq!(y.yelps, 0);
        assert!(!y.just_calmed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut y = y();
        y.yelp();
        y.enabled = false;
        y.tick(100.0); // should not decay
        assert_eq!(y.yelps, 1);
        assert!(!y.just_calmed);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut y = y();
        y.just_yelped = true;
        y.just_peaked = true;
        y.just_calmed = true;
        y.enabled = false;
        y.tick(0.016);
        assert!(!y.just_yelped);
        assert!(!y.just_peaked);
        assert!(!y.just_calmed);
    }

    #[test]
    fn is_frantic_true_at_cap() {
        let mut y = y();
        for _ in 0..5 {
            y.yelp();
        }
        assert!(y.is_frantic());
    }

    #[test]
    fn is_frantic_false_below_cap() {
        let mut y = y();
        y.yelp();
        y.yelp();
        assert!(!y.is_frantic());
    }

    #[test]
    fn is_frantic_false_when_disabled() {
        let mut y = y();
        for _ in 0..5 {
            y.yelp();
        }
        y.enabled = false;
        assert!(!y.is_frantic());
    }

    #[test]
    fn stack_fraction_zero_when_empty() {
        let y = y();
        assert_eq!(y.stack_fraction(), 0.0);
    }

    #[test]
    fn stack_fraction_half_at_midpoint() {
        let mut y = Yelp::new(4, 3.0); // max=4
        y.yelp();
        y.yelp(); // 2/4 = 0.5
        assert!((y.stack_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn stack_fraction_one_at_cap() {
        let mut y = y();
        for _ in 0..5 {
            y.yelp();
        }
        assert!((y.stack_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_scramble_base_when_empty() {
        let y = y();
        assert!((y.effective_scramble(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_scramble_scaled_at_half_stacks() {
        let mut y = Yelp::new(4, 3.0); // max=4
        y.yelp();
        y.yelp(); // 2/4 = 0.5
                  // 100 * (1 + 0.5) = 150
        assert!((y.effective_scramble(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_scramble_doubled_at_cap() {
        let mut y = y();
        for _ in 0..5 {
            y.yelp();
        }
        // 100 * (1 + 1.0) = 200
        assert!((y.effective_scramble(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_scramble_passthrough_when_disabled() {
        let mut y = y();
        for _ in 0..5 {
            y.yelp();
        }
        y.enabled = false;
        assert!((y.effective_scramble(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_yelps_clamped_to_one() {
        let y = Yelp::new(0, 3.0);
        assert_eq!(y.max_yelps, 1);
    }

    #[test]
    fn decay_interval_clamped_to_point_one() {
        let y = Yelp::new(5, 0.0);
        assert!((y.decay_interval - 0.1).abs() < 1e-5);
    }

    #[test]
    fn full_decay_sequence_two_stacks() {
        let mut y = Yelp::new(5, 2.0); // decay_interval=2.0
        y.yelp(); // 1
        y.yelp(); // 2, timer reset to 2.0
        y.tick(2.0); // one stack decays → 1, timer reset to 2.0
        assert_eq!(y.yelps, 1);
        y.tick(2.0); // second stack decays → 0
        assert_eq!(y.yelps, 0);
    }
}
