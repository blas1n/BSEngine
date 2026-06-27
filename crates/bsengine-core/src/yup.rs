use bevy_ecs::prelude::Component;

/// Consecutive-action streak tracker. Each call to `yup()` extends the
/// streak; a `tick()` with no preceding `yup()` resets the streak to zero
/// and fires `just_broke`. When the streak reaches `max_streak`, fires
/// `just_peaked`.
///
/// Models combo counters, approval chains, rhythm-game streaks, or any
/// mechanic where consecutive same-frame actions must continue or be lost.
///
/// `yup()` increments `streak` (capped at `max_streak`) and marks
/// `responded` for this frame. Fires `just_peaked` the first time streak
/// reaches `max_streak`. No-op when disabled.
///
/// `tick(_dt)` clears `just_peaked` and `just_broke`, then (when no `yup()`
/// was called this frame) resets `streak` to 0 and fires `just_broke`.
/// Finally resets `responded`.
///
/// `is_peaked()` returns `streak >= max_streak && enabled`.
///
/// `streak_fraction()` returns `(streak / max_streak).clamp(0, 1)`.
///
/// `effective_bonus(base)` returns `base * streak_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(5)`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yup {
    pub streak: u32,
    pub max_streak: u32,
    /// Set by `yup()` for this frame; cleared by `tick()`.
    pub responded: bool,
    pub just_peaked: bool,
    pub just_broke: bool,
    pub enabled: bool,
}

impl Yup {
    pub fn new(max_streak: u32) -> Self {
        Self {
            streak: 0,
            max_streak: max_streak.max(1),
            responded: false,
            just_peaked: false,
            just_broke: false,
            enabled: true,
        }
    }

    /// Extend streak for this frame. Fires `just_peaked` on first reaching
    /// `max_streak`. No-op when disabled.
    pub fn yup(&mut self) {
        if !self.enabled {
            return;
        }
        self.responded = true;
        if self.streak < self.max_streak {
            self.streak += 1;
            if self.streak >= self.max_streak {
                self.just_peaked = true;
            }
        }
    }

    /// Advance one frame: clear flags, then break streak if no `yup()` was
    /// called this frame.
    pub fn tick(&mut self, _dt: f32) {
        self.just_peaked = false;
        self.just_broke = false;
        if !self.responded && self.streak > 0 {
            self.streak = 0;
            self.just_broke = true;
        }
        self.responded = false;
    }

    /// `true` when streak is at max and component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.streak >= self.max_streak && self.enabled
    }

    /// Fraction of max streak [0.0, 1.0].
    pub fn streak_fraction(&self) -> f32 {
        (self.streak as f32 / self.max_streak as f32).clamp(0.0, 1.0)
    }

    /// Returns `base * streak_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_bonus(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.streak_fraction()
    }
}

impl Default for Yup {
    fn default() -> Self {
        Self::new(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yup {
        Yup::new(5)
    }

    // --- construction ---

    #[test]
    fn new_starts_at_zero_streak() {
        let y = y();
        assert_eq!(y.streak, 0);
        assert_eq!(y.max_streak, 5);
        assert!(!y.responded);
        assert!(!y.just_peaked);
        assert!(!y.just_broke);
    }

    #[test]
    fn new_clamps_max_streak_to_one() {
        let y = Yup::new(0);
        assert_eq!(y.max_streak, 1);
    }

    #[test]
    fn default_max_streak_is_five() {
        assert_eq!(Yup::default().max_streak, 5);
    }

    // --- yup ---

    #[test]
    fn yup_increments_streak() {
        let mut y = y();
        y.yup();
        assert_eq!(y.streak, 1);
    }

    #[test]
    fn yup_sets_responded() {
        let mut y = y();
        y.yup();
        assert!(y.responded);
    }

    #[test]
    fn yup_fires_just_peaked_at_max() {
        let mut y = y();
        for _ in 0..5 {
            y.yup();
        }
        assert!(y.just_peaked);
        assert!(y.is_peaked());
    }

    #[test]
    fn yup_caps_at_max_streak() {
        let mut y = y();
        for _ in 0..10 {
            y.yup();
        }
        assert_eq!(y.streak, 5);
    }

    #[test]
    fn yup_no_second_peaked_when_already_at_max() {
        let mut y = y();
        for _ in 0..5 {
            y.yup();
        }
        y.tick(0.016); // clear just_peaked; no break — tick has a yup pending? No.
                       // Actually we need to call yup again after tick
        y.yup();
        assert!(!y.just_peaked); // already at max, no re-fire
    }

    #[test]
    fn yup_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.yup();
        assert_eq!(y.streak, 0);
        assert!(!y.responded);
    }

    // --- tick without yup (streak break) ---

    #[test]
    fn tick_breaks_streak_when_no_yup() {
        let mut y = y();
        y.yup();
        y.yup();
        y.tick(0.016); // responded=true, no break
        y.tick(0.016); // responded=false, break!
        assert_eq!(y.streak, 0);
        assert!(y.just_broke);
    }

    #[test]
    fn tick_does_not_break_when_yup_called() {
        let mut y = y();
        y.yup();
        y.tick(0.016);
        assert_eq!(y.streak, 1);
        assert!(!y.just_broke);
    }

    #[test]
    fn tick_no_break_when_streak_already_zero() {
        let mut y = y();
        y.tick(0.016); // streak=0, no break event
        assert!(!y.just_broke);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut y = y();
        for _ in 0..5 {
            y.yup();
        }
        y.yup(); // stay at max, keep responded true
        y.tick(0.016);
        assert!(!y.just_peaked);
    }

    #[test]
    fn tick_clears_just_broke_next_frame() {
        let mut y = y();
        y.yup();
        y.tick(0.016); // no break
        y.tick(0.016); // break fires
        assert!(y.just_broke);
        y.tick(0.016); // just_broke cleared
        assert!(!y.just_broke);
    }

    #[test]
    fn tick_resets_responded() {
        let mut y = y();
        y.yup();
        y.tick(0.016);
        assert!(!y.responded);
    }

    // --- continuous streak ---

    #[test]
    fn continuous_yup_each_frame_maintains_streak() {
        let mut y = y();
        for _ in 0..10 {
            y.yup();
            y.tick(0.016);
        }
        assert_eq!(y.streak, 5); // capped at max
        assert!(!y.just_broke);
    }

    #[test]
    fn streak_resets_and_rebuilds() {
        let mut y = y();
        y.yup();
        y.yup();
        y.tick(0.016); // maintain streak=2
        y.tick(0.016); // break → streak=0
        assert_eq!(y.streak, 0);
        y.yup();
        y.tick(0.016);
        assert_eq!(y.streak, 1);
    }

    // --- is_peaked ---

    #[test]
    fn is_peaked_false_when_below_max() {
        let mut y = y();
        y.yup();
        assert!(!y.is_peaked());
    }

    #[test]
    fn is_peaked_true_at_max() {
        let mut y = y();
        for _ in 0..5 {
            y.yup();
        }
        assert!(y.is_peaked());
    }

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut y = y();
        for _ in 0..5 {
            y.yup();
        }
        y.enabled = false;
        assert!(!y.is_peaked());
    }

    // --- fractions / effective ---

    #[test]
    fn streak_fraction_zero_at_start() {
        assert_eq!(y().streak_fraction(), 0.0);
    }

    #[test]
    fn streak_fraction_half_at_midpoint() {
        let mut y = Yup::new(4);
        y.yup();
        y.yup();
        assert!((y.streak_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn streak_fraction_one_at_max() {
        let mut y = y();
        for _ in 0..5 {
            y.yup();
        }
        assert!((y.streak_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_bonus_zero_when_no_streak() {
        assert_eq!(y().effective_bonus(100.0), 0.0);
    }

    #[test]
    fn effective_bonus_scales_with_fraction() {
        let mut y = Yup::new(4);
        y.yup();
        y.yup();
        assert!((y.effective_bonus(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_bonus_zero_when_disabled() {
        let mut y = y();
        for _ in 0..5 {
            y.yup();
        }
        y.enabled = false;
        assert_eq!(y.effective_bonus(100.0), 0.0);
    }
}
