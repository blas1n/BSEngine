use bevy_ecs::prelude::Component;

/// Duration-based viscous debuff that reduces movement speed and notifies
/// on cleanse. An external source applies the slime via `apply_slime(duration)`;
/// while active, `effective_move_speed()` returns a reduced speed. The slime
/// wears off naturally as `slime_timer` counts down, firing `just_cleansed`
/// on expiry.
///
/// `apply_slime(duration)` applies or extends the debuff. Uses a
/// high-watermark rule: `slime_timer` is only replaced when the new duration
/// exceeds the current remaining time. Fires `just_slimed` on the first
/// application (i.e., when transitioning from not-slimed to slimed). No-op
/// when disabled or `duration ≤ 0`.
///
/// `tick(dt)` clears one-frame flags first; counts down `slime_timer`; fires
/// `just_cleansed` on the tick the timer reaches zero.
///
/// `cleanse()` removes the slime immediately and fires `just_cleansed` if
/// the entity was slimed. No-op when not slimed or disabled.
///
/// `is_slimed()` returns `slimed && enabled`.
///
/// `effective_move_speed(base)` returns `base * slow_factor` when slimed and
/// enabled; returns `base` otherwise.
///
/// Distinct from `Chill` (ice-based slow that scales with chill stacks),
/// `Slow` (flat persistent movement reduction, no duration), `Hobble`
/// (structural movement penalty from injury), and `Drench` (water-saturation
/// stat debuff): Slime is a **timed viscous grapple-slow** — a one-shot
/// duration debuff applied externally, with high-watermark re-application
/// and a cleanse event that external systems can react to.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Slime {
    /// Remaining slime duration in seconds. 0.0 when not slimed.
    pub slime_timer: f32,
    /// Movement speed multiplier while slimed. Clamped [0.0, 1.0].
    /// 1.0 = no slow; 0.0 = completely immobilised.
    pub slow_factor: f32,
    pub slimed: bool,
    pub just_slimed: bool,
    pub just_cleansed: bool,
    pub enabled: bool,
}

impl Slime {
    pub fn new(slow_factor: f32) -> Self {
        Self {
            slime_timer: 0.0,
            slow_factor: slow_factor.clamp(0.0, 1.0),
            slimed: false,
            just_slimed: false,
            just_cleansed: false,
            enabled: true,
        }
    }

    /// Apply a slime debuff for `duration` seconds (high-watermark: only
    /// extends if `duration > slime_timer`). Fires `just_slimed` on the
    /// first application. No-op when disabled or `duration ≤ 0`.
    pub fn apply_slime(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        let was_not_slimed = !self.slimed;
        if duration > self.slime_timer {
            self.slime_timer = duration;
        }
        if !self.slimed {
            self.slimed = true;
        }
        if was_not_slimed {
            self.just_slimed = true;
        }
    }

    /// Advance the slime timer. Clears `just_slimed` and `just_cleansed`
    /// first; counts down `slime_timer`; fires `just_cleansed` on the tick
    /// that the timer reaches zero.
    pub fn tick(&mut self, dt: f32) {
        self.just_slimed = false;
        self.just_cleansed = false;

        if self.slimed {
            self.slime_timer = (self.slime_timer - dt).max(0.0);
            if self.slime_timer == 0.0 {
                self.slimed = false;
                self.just_cleansed = true;
            }
        }
    }

    /// Remove the slime debuff immediately. Fires `just_cleansed` if the
    /// entity was slimed. No-op when not slimed or disabled.
    pub fn cleanse(&mut self) {
        if !self.enabled || !self.slimed {
            return;
        }
        self.slimed = false;
        self.slime_timer = 0.0;
        self.just_cleansed = true;
    }

    /// `true` when the entity is slimed and the component is enabled.
    pub fn is_slimed(&self) -> bool {
        self.slimed && self.enabled
    }

    /// Effective movement speed reduced by slime. Returns `base * slow_factor`
    /// when slimed and enabled; returns `base` otherwise.
    pub fn effective_move_speed(&self, base: f32) -> f32 {
        if self.is_slimed() {
            base * self.slow_factor
        } else {
            base
        }
    }
}

impl Default for Slime {
    fn default() -> Self {
        Self::new(0.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_not_slimed() {
        let s = Slime::new(0.4);
        assert!(!s.slimed);
        assert!(!s.is_slimed());
        assert_eq!(s.slime_timer, 0.0);
    }

    #[test]
    fn apply_slime_sets_slimed() {
        let mut s = Slime::new(0.4);
        s.apply_slime(3.0);
        assert!(s.slimed);
        assert!(s.is_slimed());
        assert!((s.slime_timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn apply_slime_fires_just_slimed_on_first_application() {
        let mut s = Slime::new(0.4);
        s.apply_slime(3.0);
        assert!(s.just_slimed);
    }

    #[test]
    fn apply_slime_no_just_slimed_on_reapplication() {
        let mut s = Slime::new(0.4);
        s.apply_slime(3.0);
        s.tick(0.0); // clear flags
        s.apply_slime(5.0); // re-apply while still slimed
        assert!(!s.just_slimed);
    }

    #[test]
    fn apply_slime_high_watermark_extends_timer() {
        let mut s = Slime::new(0.4);
        s.apply_slime(3.0);
        s.apply_slime(5.0); // longer — should replace
        assert!((s.slime_timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn apply_slime_high_watermark_no_replace_shorter() {
        let mut s = Slime::new(0.4);
        s.apply_slime(5.0);
        s.apply_slime(2.0); // shorter — should not replace
        assert!((s.slime_timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn apply_slime_no_op_when_disabled() {
        let mut s = Slime::new(0.4);
        s.enabled = false;
        s.apply_slime(3.0);
        assert!(!s.slimed);
    }

    #[test]
    fn apply_slime_no_op_when_duration_zero() {
        let mut s = Slime::new(0.4);
        s.apply_slime(0.0);
        assert!(!s.slimed);
    }

    #[test]
    fn apply_slime_no_op_when_duration_negative() {
        let mut s = Slime::new(0.4);
        s.apply_slime(-1.0);
        assert!(!s.slimed);
    }

    #[test]
    fn tick_counts_down_timer() {
        let mut s = Slime::new(0.4);
        s.apply_slime(4.0);
        s.tick(1.0);
        assert!((s.slime_timer - 3.0).abs() < 1e-5);
        assert!(s.slimed);
    }

    #[test]
    fn tick_clears_just_slimed() {
        let mut s = Slime::new(0.4);
        s.apply_slime(2.0);
        s.tick(0.016);
        assert!(!s.just_slimed);
    }

    #[test]
    fn tick_fires_just_cleansed_on_expiry() {
        let mut s = Slime::new(0.4);
        s.apply_slime(1.0);
        s.tick(1.0);
        assert!(s.just_cleansed);
        assert!(!s.slimed);
    }

    #[test]
    fn tick_clears_slimed_on_expiry() {
        let mut s = Slime::new(0.4);
        s.apply_slime(0.5);
        s.tick(0.5);
        assert!(!s.slimed);
        assert!(!s.is_slimed());
    }

    #[test]
    fn tick_no_just_cleansed_while_still_active() {
        let mut s = Slime::new(0.4);
        s.apply_slime(3.0);
        s.tick(1.0);
        assert!(!s.just_cleansed);
    }

    #[test]
    fn tick_clears_just_cleansed_next_frame() {
        let mut s = Slime::new(0.4);
        s.apply_slime(0.5);
        s.tick(0.5); // just_cleansed = true
        s.tick(0.016); // cleared
        assert!(!s.just_cleansed);
    }

    #[test]
    fn cleanse_removes_slime_immediately() {
        let mut s = Slime::new(0.4);
        s.apply_slime(5.0);
        s.cleanse();
        assert!(!s.slimed);
        assert_eq!(s.slime_timer, 0.0);
    }

    #[test]
    fn cleanse_fires_just_cleansed() {
        let mut s = Slime::new(0.4);
        s.apply_slime(5.0);
        s.cleanse();
        assert!(s.just_cleansed);
    }

    #[test]
    fn cleanse_no_op_when_not_slimed() {
        let mut s = Slime::new(0.4);
        s.cleanse();
        assert!(!s.just_cleansed);
    }

    #[test]
    fn cleanse_no_op_when_disabled() {
        let mut s = Slime::new(0.4);
        s.apply_slime(5.0);
        s.enabled = false;
        s.cleanse();
        // still slimed because cleanse was no-op
        assert!(s.slimed);
    }

    #[test]
    fn is_slimed_false_when_disabled() {
        let mut s = Slime::new(0.4);
        s.slimed = true;
        s.enabled = false;
        assert!(!s.is_slimed());
    }

    #[test]
    fn effective_move_speed_reduced_while_slimed() {
        let mut s = Slime::new(0.5);
        s.apply_slime(2.0);
        // 100 * 0.5 = 50
        assert!((s.effective_move_speed(100.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_base_when_not_slimed() {
        let s = Slime::new(0.5);
        assert!((s.effective_move_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_base_when_disabled() {
        let mut s = Slime::new(0.5);
        s.slimed = true;
        s.enabled = false;
        assert!((s.effective_move_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_base_after_expiry() {
        let mut s = Slime::new(0.5);
        s.apply_slime(0.5);
        s.tick(0.5);
        assert!((s.effective_move_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn slow_factor_clamped_to_one() {
        let s = Slime::new(2.0);
        assert!((s.slow_factor - 1.0).abs() < 1e-5);
    }

    #[test]
    fn slow_factor_clamped_to_zero() {
        let s = Slime::new(-0.5);
        assert_eq!(s.slow_factor, 0.0);
    }

    #[test]
    fn zero_slow_factor_fully_immobilises() {
        let mut s = Slime::new(0.0);
        s.apply_slime(3.0);
        assert!((s.effective_move_speed(100.0)).abs() < 1e-5);
    }

    #[test]
    fn reapplication_after_cleanse_fires_just_slimed_again() {
        let mut s = Slime::new(0.4);
        s.apply_slime(1.0);
        s.tick(0.0); // clear flags
        s.tick(1.0); // expires
        s.apply_slime(2.0); // reapply
        assert!(s.just_slimed);
    }
}
