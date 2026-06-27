use bevy_ecs::prelude::Component;

/// General-purpose counter component for tracking cumulative occurrences toward
/// a goal (kills, hits landed, objectives completed, items collected, etc.).
///
/// Call `increment(amount)` each time the tracked event occurs; `count` grows
/// up to `goal`. `just_completed` fires on the frame `count` first reaches
/// `goal`; subsequent increments while already complete are no-ops.
///
/// `reset()` returns the tally to zero (sets `just_reset`). `tick()` clears
/// one-frame flags.
///
/// Distinct from `Combo` (time-sensitive streak with decay), `Experience` (XP
/// accumulation with level thresholds), and `Stat` (raw attribute value):
/// Tally is a simple, stateless event counter — no decay, no time pressure,
/// just count toward a fixed goal.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Tally {
    pub count: u32,
    pub goal: u32,
    pub just_incremented: bool,
    pub just_completed: bool,
    pub just_reset: bool,
    pub enabled: bool,
}

impl Tally {
    pub fn new(goal: u32) -> Self {
        Self {
            count: 0,
            goal: goal.max(1),
            just_incremented: false,
            just_completed: false,
            just_reset: false,
            enabled: true,
        }
    }

    /// Add `amount` to the count, clamping at `goal`. Sets `just_incremented`
    /// and `just_completed` (on first completion). No-op when already complete
    /// or disabled.
    pub fn increment(&mut self, amount: u32) {
        if !self.enabled || self.is_complete() {
            return;
        }
        let was_complete = self.is_complete();
        self.count = (self.count + amount).min(self.goal);
        self.just_incremented = true;
        if !was_complete && self.is_complete() {
            self.just_completed = true;
        }
    }

    /// Reset the count to zero.
    pub fn reset(&mut self) {
        self.count = 0;
        self.just_reset = true;
        self.just_completed = false;
    }

    /// Clear one-frame flags. Call once per frame after systems have read them.
    pub fn tick(&mut self) {
        self.just_incremented = false;
        self.just_completed = false;
        self.just_reset = false;
    }

    pub fn is_complete(&self) -> bool {
        self.count >= self.goal
    }

    /// Fraction of the goal achieved [0.0, 1.0].
    pub fn progress_fraction(&self) -> f32 {
        if self.goal == 0 {
            return 1.0;
        }
        (self.count as f32 / self.goal as f32).clamp(0.0, 1.0)
    }

    /// Remaining events needed to reach the goal.
    pub fn remaining(&self) -> u32 {
        self.goal.saturating_sub(self.count)
    }
}

impl Default for Tally {
    fn default() -> Self {
        Self::new(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn increment_increases_count() {
        let mut t = Tally::new(5);
        t.increment(1);
        assert_eq!(t.count, 1);
        assert!(t.just_incremented);
    }

    #[test]
    fn increment_clamps_at_goal() {
        let mut t = Tally::new(3);
        t.increment(10);
        assert_eq!(t.count, 3);
    }

    #[test]
    fn just_completed_fires_on_first_completion() {
        let mut t = Tally::new(3);
        t.increment(2);
        t.tick();
        t.increment(1);
        assert!(t.just_completed);
        assert!(t.is_complete());
    }

    #[test]
    fn increment_no_op_when_already_complete() {
        let mut t = Tally::new(3);
        t.increment(3);
        t.tick();
        t.increment(1); // already done
        assert!(!t.just_incremented);
        assert_eq!(t.count, 3);
    }

    #[test]
    fn reset_zeros_count() {
        let mut t = Tally::new(5);
        t.increment(3);
        t.reset();
        assert_eq!(t.count, 0);
        assert!(t.just_reset);
        assert!(!t.is_complete());
    }

    #[test]
    fn reset_clears_just_completed() {
        let mut t = Tally::new(2);
        t.increment(2);
        assert!(t.just_completed);
        t.reset();
        assert!(!t.just_completed);
    }

    #[test]
    fn tick_clears_flags() {
        let mut t = Tally::new(5);
        t.increment(1);
        t.tick();
        assert!(!t.just_incremented);
        assert!(!t.just_completed);
        assert!(!t.just_reset);
    }

    #[test]
    fn progress_fraction_mid() {
        let mut t = Tally::new(4);
        t.increment(2);
        assert!((t.progress_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn progress_fraction_complete() {
        let mut t = Tally::new(4);
        t.increment(4);
        assert!((t.progress_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_correct() {
        let mut t = Tally::new(5);
        t.increment(2);
        assert_eq!(t.remaining(), 3);
    }

    #[test]
    fn remaining_zero_when_complete() {
        let mut t = Tally::new(3);
        t.increment(3);
        assert_eq!(t.remaining(), 0);
    }

    #[test]
    fn disabled_increment_no_op() {
        let mut t = Tally::new(5);
        t.enabled = false;
        t.increment(1);
        assert_eq!(t.count, 0);
    }

    #[test]
    fn can_complete_again_after_reset() {
        let mut t = Tally::new(2);
        t.increment(2);
        t.tick();
        t.reset();
        t.increment(2);
        assert!(t.just_completed);
    }

    #[test]
    fn goal_clamped_to_one() {
        let t = Tally::new(0);
        assert_eq!(t.goal, 1);
    }
}
