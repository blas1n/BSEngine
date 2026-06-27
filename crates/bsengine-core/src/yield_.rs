use bevy_ecs::prelude::Component;

/// Right-of-way timer. Entities call `yield_to()` to voluntarily defer for a
/// fixed duration, suppressing their priority output while yielding. Models
/// traffic priority, turn-order deferral, or cooldown-gated action gates.
///
/// `yield_to()` starts a yield period of `yield_duration` seconds. Fires
/// `just_started_yielding`. No-op when already yielding or disabled.
///
/// `tick(dt)` clears one-frame flags first, then if enabled and yielding:
/// drains `yield_remaining`. Fires `just_finished_yielding` when it reaches 0.
///
/// `has_right_of_way()` returns `!is_yielding && enabled`.
///
/// `yield_fraction()` returns `(yield_remaining / yield_duration).clamp(0.0, 1.0)`.
///
/// `effective_priority(base)` returns `base` when `has_right_of_way()`;
/// `0.0` when yielding or disabled.
///
/// Default: `new(0.5)` — half-second yield window.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yield {
    /// Seconds to yield for when `yield_to()` is called. Clamped >= 0.1.
    pub yield_duration: f32,
    /// Countdown remaining [0, yield_duration].
    pub yield_remaining: f32,
    /// `true` while actively yielding.
    pub is_yielding: bool,
    pub just_started_yielding: bool,
    pub just_finished_yielding: bool,
    pub enabled: bool,
}

impl Yield {
    pub fn new(yield_duration: f32) -> Self {
        Self {
            yield_duration: yield_duration.max(0.1),
            yield_remaining: 0.0,
            is_yielding: false,
            just_started_yielding: false,
            just_finished_yielding: false,
            enabled: true,
        }
    }

    /// Begin yielding. No-op when already yielding or disabled.
    pub fn yield_to(&mut self) {
        if !self.enabled || self.is_yielding {
            return;
        }
        self.is_yielding = true;
        self.yield_remaining = self.yield_duration;
        self.just_started_yielding = true;
    }

    /// Advance one frame: clear flags, then drain yield timer.
    pub fn tick(&mut self, dt: f32) {
        self.just_started_yielding = false;
        self.just_finished_yielding = false;
        if !self.enabled || !self.is_yielding {
            return;
        }
        self.yield_remaining = (self.yield_remaining - dt).max(0.0);
        if self.yield_remaining == 0.0 {
            self.is_yielding = false;
            self.just_finished_yielding = true;
        }
    }

    /// `true` when not yielding and component is enabled.
    pub fn has_right_of_way(&self) -> bool {
        !self.is_yielding && self.enabled
    }

    /// Yield progress remaining as a fraction [0.0, 1.0].
    pub fn yield_fraction(&self) -> f32 {
        if self.yield_duration <= 0.0 {
            return 0.0;
        }
        (self.yield_remaining / self.yield_duration).clamp(0.0, 1.0)
    }

    /// Returns `base` when `has_right_of_way()`; `0.0` when yielding or
    /// disabled.
    pub fn effective_priority(&self, base: f32) -> f32 {
        if self.has_right_of_way() {
            base
        } else {
            0.0
        }
    }
}

impl Default for Yield {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yield {
        Yield::new(1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_with_right_of_way() {
        let y = y();
        assert!(!y.is_yielding);
        assert_eq!(y.yield_remaining, 0.0);
        assert!(!y.just_started_yielding);
        assert!(!y.just_finished_yielding);
        assert!(y.has_right_of_way());
    }

    #[test]
    fn yield_duration_clamped_to_tenth() {
        let y = Yield::new(0.0);
        assert!((y.yield_duration - 0.1).abs() < 1e-5);
    }

    // --- yield_to ---

    #[test]
    fn yield_to_starts_yielding() {
        let mut y = y();
        y.yield_to();
        assert!(y.is_yielding);
        assert!(!y.has_right_of_way());
    }

    #[test]
    fn yield_to_fires_just_started_yielding() {
        let mut y = y();
        y.yield_to();
        assert!(y.just_started_yielding);
    }

    #[test]
    fn yield_to_sets_remaining_to_duration() {
        let mut y = y();
        y.yield_to();
        assert!((y.yield_remaining - 1.0).abs() < 1e-5);
    }

    #[test]
    fn yield_to_no_op_when_already_yielding() {
        let mut y = y();
        y.yield_to();
        y.tick(0.016); // clears just_started_yielding
        y.yield_to(); // already yielding
        assert!(!y.just_started_yielding);
    }

    #[test]
    fn yield_to_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.yield_to();
        assert!(!y.is_yielding);
        assert!(!y.just_started_yielding);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_started_yielding() {
        let mut y = y();
        y.yield_to();
        y.tick(0.016);
        assert!(!y.just_started_yielding);
    }

    #[test]
    fn tick_drains_yield_remaining() {
        let mut y = y();
        y.yield_to();
        y.tick(0.5);
        assert!((y.yield_remaining - 0.5).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_finished_yielding_at_zero() {
        let mut y = y();
        y.yield_to();
        y.tick(1.0);
        assert!(y.just_finished_yielding);
        assert!(!y.is_yielding);
    }

    #[test]
    fn tick_fires_just_finished_yielding_crossing_zero() {
        let mut y = y();
        y.yield_to();
        y.tick(0.5);
        y.tick(0.6);
        assert!(y.just_finished_yielding);
        assert!(!y.is_yielding);
    }

    #[test]
    fn tick_restores_right_of_way_after_yield() {
        let mut y = y();
        y.yield_to();
        y.tick(1.0);
        assert!(y.has_right_of_way());
    }

    #[test]
    fn tick_clears_just_finished_yielding_next_frame() {
        let mut y = y();
        y.yield_to();
        y.tick(1.0); // just_finished=true
        y.tick(0.016);
        assert!(!y.just_finished_yielding);
    }

    #[test]
    fn tick_no_op_when_not_yielding() {
        let mut y = y();
        y.tick(1.0);
        assert!(!y.just_finished_yielding);
        assert_eq!(y.yield_remaining, 0.0);
    }

    #[test]
    fn tick_no_op_on_timer_when_disabled() {
        let mut y = y();
        y.yield_remaining = 1.0;
        y.is_yielding = true;
        y.enabled = false;
        y.tick(0.5);
        assert!((y.yield_remaining - 1.0).abs() < 1e-5);
    }

    // --- has_right_of_way ---

    #[test]
    fn has_right_of_way_true_initially() {
        assert!(y().has_right_of_way());
    }

    #[test]
    fn has_right_of_way_false_while_yielding() {
        let mut y = y();
        y.yield_to();
        assert!(!y.has_right_of_way());
    }

    #[test]
    fn has_right_of_way_false_when_disabled() {
        let y_disabled = {
            let mut y = y();
            y.enabled = false;
            y
        };
        assert!(!y_disabled.has_right_of_way());
    }

    // --- yield_fraction ---

    #[test]
    fn yield_fraction_zero_when_not_yielding() {
        assert_eq!(y().yield_fraction(), 0.0);
    }

    #[test]
    fn yield_fraction_one_when_just_started() {
        let mut y = y();
        y.yield_to();
        assert!((y.yield_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn yield_fraction_half_at_midpoint() {
        let mut y = y();
        y.yield_to();
        y.tick(0.5);
        assert!((y.yield_fraction() - 0.5).abs() < 1e-4);
    }

    // --- effective_priority ---

    #[test]
    fn effective_priority_base_when_right_of_way() {
        let y = y();
        assert!((y.effective_priority(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_priority_zero_while_yielding() {
        let mut y = y();
        y.yield_to();
        assert_eq!(y.effective_priority(100.0), 0.0);
    }

    #[test]
    fn effective_priority_zero_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert_eq!(y.effective_priority(100.0), 0.0);
    }

    #[test]
    fn effective_priority_restored_after_yield_expires() {
        let mut y = y();
        y.yield_to();
        y.tick(1.0);
        assert!((y.effective_priority(100.0) - 100.0).abs() < 1e-4);
    }

    // --- re-yield cycle ---

    #[test]
    fn can_yield_again_after_expiry() {
        let mut y = y();
        y.yield_to();
        y.tick(1.0); // expires
        y.tick(0.016);
        y.yield_to();
        assert!(y.is_yielding);
        assert!(y.just_started_yielding);
    }
}
