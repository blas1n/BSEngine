use bevy_ecs::prelude::Component;

/// Affirmation counter with threshold. Accumulates `vote()` calls and fires
/// `just_reached` once when the count meets `yes_threshold`. Models crowd
/// approval, multi-press confirm gates, voting systems, or cooperative
/// trigger requirements.
///
/// `vote()` increments `yes_count`. Fires `just_reached` the first time the
/// count reaches `yes_threshold`. No-op when disabled.
///
/// `reset()` clears `yes_count` and fires `just_reset` if any votes existed.
/// No-op when disabled.
///
/// `reset_all()` clears `yes_count` and fires `just_reset` if any votes
/// existed regardless of `enabled` state.
///
/// `tick(_dt)` clears one-frame flags only.
///
/// `is_confirmed()` returns `yes_count >= yes_threshold && enabled`.
///
/// `vote_fraction()` returns `(yes_count as f32 / yes_threshold as f32).clamp(0.0, 1.0)`.
///
/// `effective_approval(base)` returns `base * vote_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(3)` — three votes required.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yes {
    /// Current vote count.
    pub yes_count: u32,
    /// Votes required to confirm. Clamped >= 1.
    pub yes_threshold: u32,
    pub just_reached: bool,
    pub just_reset: bool,
    pub enabled: bool,
}

impl Yes {
    pub fn new(yes_threshold: u32) -> Self {
        Self {
            yes_count: 0,
            yes_threshold: yes_threshold.max(1),
            just_reached: false,
            just_reset: false,
            enabled: true,
        }
    }

    /// Cast one vote. Fires `just_reached` on first crossing `yes_threshold`.
    /// No-op when disabled.
    pub fn vote(&mut self) {
        if !self.enabled {
            return;
        }
        let was_below = self.yes_count < self.yes_threshold;
        self.yes_count += 1;
        if was_below && self.yes_count >= self.yes_threshold {
            self.just_reached = true;
        }
    }

    /// Clear all votes. Fires `just_reset` if any existed. No-op when
    /// disabled.
    pub fn reset(&mut self) {
        if !self.enabled {
            return;
        }
        if self.yes_count > 0 {
            self.just_reset = true;
        }
        self.yes_count = 0;
    }

    /// Clear all votes regardless of enabled state. Fires `just_reset` if
    /// any existed.
    pub fn reset_all(&mut self) {
        if self.yes_count > 0 {
            self.just_reset = true;
        }
        self.yes_count = 0;
    }

    /// Advance one frame: clear one-frame flags only.
    pub fn tick(&mut self, _dt: f32) {
        self.just_reached = false;
        self.just_reset = false;
    }

    /// `true` when vote count has reached or exceeded threshold and enabled.
    pub fn is_confirmed(&self) -> bool {
        self.yes_count >= self.yes_threshold && self.enabled
    }

    /// Vote progress as a fraction of threshold [0.0, 1.0].
    pub fn vote_fraction(&self) -> f32 {
        (self.yes_count as f32 / self.yes_threshold as f32).clamp(0.0, 1.0)
    }

    /// Returns `base * vote_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_approval(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.vote_fraction()
    }
}

impl Default for Yes {
    fn default() -> Self {
        Self::new(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yes {
        Yes::new(3)
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let y = y();
        assert_eq!(y.yes_count, 0);
        assert!(!y.just_reached);
        assert!(!y.just_reset);
        assert!(!y.is_confirmed());
    }

    #[test]
    fn threshold_clamped_to_one() {
        let y = Yes::new(0);
        assert_eq!(y.yes_threshold, 1);
    }

    // --- vote ---

    #[test]
    fn vote_increments_count() {
        let mut y = y();
        y.vote();
        assert_eq!(y.yes_count, 1);
    }

    #[test]
    fn vote_fires_just_reached_at_threshold() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        assert!(y.just_reached);
        assert!(y.is_confirmed());
    }

    #[test]
    fn vote_does_not_refire_just_reached() {
        let mut y = y();
        for _ in 0..3 {
            y.vote();
        }
        y.tick(0.016);
        y.vote();
        assert!(!y.just_reached);
    }

    #[test]
    fn vote_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.vote();
        assert_eq!(y.yes_count, 0);
        assert!(!y.just_reached);
    }

    // --- reset ---

    #[test]
    fn reset_clears_count() {
        let mut y = y();
        y.vote();
        y.vote();
        y.reset();
        assert_eq!(y.yes_count, 0);
    }

    #[test]
    fn reset_fires_just_reset_when_had_votes() {
        let mut y = y();
        y.vote();
        y.reset();
        assert!(y.just_reset);
    }

    #[test]
    fn reset_no_just_reset_when_empty() {
        let mut y = y();
        y.reset();
        assert!(!y.just_reset);
    }

    #[test]
    fn reset_no_op_when_disabled() {
        let mut y = y();
        y.vote();
        y.enabled = false;
        y.reset();
        assert_eq!(y.yes_count, 1);
        assert!(!y.just_reset);
    }

    // --- reset_all ---

    #[test]
    fn reset_all_clears_when_disabled() {
        let mut y = y();
        y.vote();
        y.enabled = false;
        y.reset_all();
        assert_eq!(y.yes_count, 0);
        assert!(y.just_reset);
    }

    #[test]
    fn reset_all_no_just_reset_when_empty() {
        let mut y = y();
        y.reset_all();
        assert!(!y.just_reset);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_reached() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        y.tick(0.016);
        assert!(!y.just_reached);
    }

    #[test]
    fn tick_clears_just_reset() {
        let mut y = y();
        y.vote();
        y.reset();
        y.tick(0.016);
        assert!(!y.just_reset);
    }

    #[test]
    fn tick_does_not_change_count() {
        let mut y = y();
        y.vote();
        y.vote();
        y.tick(1000.0);
        assert_eq!(y.yes_count, 2);
    }

    // --- is_confirmed ---

    #[test]
    fn is_confirmed_false_below_threshold() {
        let mut y = y();
        y.vote();
        y.vote();
        assert!(!y.is_confirmed());
    }

    #[test]
    fn is_confirmed_true_at_threshold() {
        let mut y = y();
        for _ in 0..3 {
            y.vote();
        }
        assert!(y.is_confirmed());
    }

    #[test]
    fn is_confirmed_false_when_disabled() {
        let mut y = y();
        for _ in 0..3 {
            y.vote();
        }
        y.enabled = false;
        assert!(!y.is_confirmed());
    }

    // --- vote_fraction ---

    #[test]
    fn vote_fraction_zero_when_empty() {
        assert_eq!(y().vote_fraction(), 0.0);
    }

    #[test]
    fn vote_fraction_at_partial() {
        let mut y = y(); // threshold=3
        y.vote(); // 1/3
        assert!((y.vote_fraction() - 1.0 / 3.0).abs() < 1e-4);
    }

    #[test]
    fn vote_fraction_one_at_threshold() {
        let mut y = y();
        for _ in 0..3 {
            y.vote();
        }
        assert!((y.vote_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_approval ---

    #[test]
    fn effective_approval_zero_when_empty() {
        assert_eq!(y().effective_approval(100.0), 0.0);
    }

    #[test]
    fn effective_approval_scales_with_votes() {
        let mut y = y(); // threshold=3
        y.vote(); // 1/3
        assert!((y.effective_approval(90.0) - 30.0).abs() < 1e-3);
    }

    #[test]
    fn effective_approval_full_at_threshold() {
        let mut y = y();
        for _ in 0..3 {
            y.vote();
        }
        assert!((y.effective_approval(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_approval_zero_when_disabled() {
        let mut y = y();
        for _ in 0..3 {
            y.vote();
        }
        y.enabled = false;
        assert_eq!(y.effective_approval(100.0), 0.0);
    }

    // --- vote-reset cycle ---

    #[test]
    fn reset_and_revote_allows_new_confirmation() {
        let mut y = y();
        for _ in 0..3 {
            y.vote();
        }
        y.tick(0.016);
        y.reset();
        y.tick(0.016);
        for _ in 0..3 {
            y.vote();
        }
        assert!(y.just_reached);
    }
}
