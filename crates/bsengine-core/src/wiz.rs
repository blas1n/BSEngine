use bevy_ecs::prelude::Component;

/// Monotonically-growing mastery accumulator. Expertise grows through
/// `practice()` events and never decays on its own — only an explicit
/// `reset()` returns it to 0. Fires `just_practiced` on any increase
/// and `just_mastered` on first reaching the cap.
///
/// Unlike `Woo` (affection that decays passively), `Wist` (longing that
/// can be released), and `Whelm` (overflow threshold), Wiz is a **ratchet**:
/// progress can only go forward or be fully wiped. This models skills,
/// reputation, or proficiency that accumulate through repetition and
/// cannot be lost to inaction.
///
/// `practice()` adds `grow_rate` to `wiz_level`. If enabled and
/// `wiz_level < max_wiz`: clamps to `max_wiz`. Fires `just_practiced`.
/// Fires `just_mastered` the first time `wiz_level` reaches `max_wiz`.
/// No-op when already at cap or disabled.
///
/// `reset()` sets `wiz_level` to 0. No-op when already at 0.
///
/// `tick(dt)` clears one-frame flags only. No time-based changes.
///
/// `is_novice()` returns `wiz_level == 0.0 && enabled`.
///
/// `is_master()` returns `wiz_level >= max_wiz && enabled`.
///
/// `wiz_fraction()` returns `(wiz_level / max_wiz).clamp(0.0, 1.0)`.
///
/// `effective_skill(base)` returns `base * (1.0 + wiz_fraction())` when
/// enabled — up to 2× at full mastery; returns `base` when disabled.
///
/// Default: `new(10.0, 1.0)` — mastery cap 10, grow 1 unit per practice.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wiz {
    /// Current mastery level [0, max_wiz].
    pub wiz_level: f32,
    /// Maximum mastery. Clamped >= 1.0.
    pub max_wiz: f32,
    /// Amount gained per `practice()` call. Clamped >= 0.0.
    pub grow_rate: f32,
    pub just_practiced: bool,
    pub just_mastered: bool,
    pub enabled: bool,
}

impl Wiz {
    pub fn new(max_wiz: f32, grow_rate: f32) -> Self {
        Self {
            wiz_level: 0.0,
            max_wiz: max_wiz.max(1.0),
            grow_rate: grow_rate.max(0.0),
            just_practiced: false,
            just_mastered: false,
            enabled: true,
        }
    }

    /// Add `grow_rate` mastery. Fires `just_practiced`. Fires `just_mastered`
    /// on first reaching cap. No-op when already at cap or disabled.
    pub fn practice(&mut self) {
        if !self.enabled || self.wiz_level >= self.max_wiz {
            return;
        }
        let prev = self.wiz_level;
        self.wiz_level = (self.wiz_level + self.grow_rate).min(self.max_wiz);
        self.just_practiced = true;
        if prev < self.max_wiz && self.wiz_level >= self.max_wiz {
            self.just_mastered = true;
        }
    }

    /// Reset mastery to 0. No-op when already at 0.
    pub fn reset(&mut self) {
        if self.wiz_level == 0.0 {
            return;
        }
        self.wiz_level = 0.0;
    }

    /// Advance one frame: clear one-frame flags only. No time-based changes.
    pub fn tick(&mut self, _dt: f32) {
        self.just_practiced = false;
        self.just_mastered = false;
    }

    /// `true` when mastery is at 0 and component is enabled.
    pub fn is_novice(&self) -> bool {
        self.wiz_level == 0.0 && self.enabled
    }

    /// `true` when mastery is at maximum and component is enabled.
    pub fn is_master(&self) -> bool {
        self.wiz_level >= self.max_wiz && self.enabled
    }

    /// Mastery level as a fraction of maximum [0.0, 1.0].
    pub fn wiz_fraction(&self) -> f32 {
        (self.wiz_level / self.max_wiz).clamp(0.0, 1.0)
    }

    /// Scale `base` by mastery. Returns `base * (1.0 + wiz_fraction())`
    /// when enabled — up to 2× at full mastery; `base` when disabled.
    pub fn effective_skill(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.wiz_fraction())
    }
}

impl Default for Wiz {
    fn default() -> Self {
        Self::new(10.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wiz {
        Wiz::new(10.0, 1.0) // max=10, 1 unit/practice
    }

    #[test]
    fn new_starts_as_novice() {
        let w = w();
        assert_eq!(w.wiz_level, 0.0);
        assert!(!w.just_practiced);
        assert!(!w.just_mastered);
        assert!(w.is_novice());
        assert!(!w.is_master());
    }

    // --- practice ---

    #[test]
    fn practice_increments_level() {
        let mut w = w();
        w.practice();
        assert!((w.wiz_level - 1.0).abs() < 1e-5);
    }

    #[test]
    fn practice_fires_just_practiced() {
        let mut w = w();
        w.practice();
        assert!(w.just_practiced);
    }

    #[test]
    fn practice_fires_just_mastered_at_cap() {
        let mut w = Wiz::new(3.0, 1.0);
        w.practice(); // 1
        w.practice(); // 2
        w.practice(); // 3 — just_mastered
        assert!(w.just_mastered);
        assert!(w.just_practiced);
    }

    #[test]
    fn practice_does_not_refire_just_mastered() {
        let mut w = Wiz::new(1.0, 1.0);
        w.practice(); // just_mastered=true
        w.tick(0.016); // clear flags
        w.practice(); // already at cap, no-op
        assert!(!w.just_mastered);
        assert!(!w.just_practiced);
    }

    #[test]
    fn practice_no_op_at_cap() {
        let mut w = Wiz::new(3.0, 1.0);
        w.practice();
        w.practice();
        w.practice(); // at cap
        w.practice(); // no-op
        assert!((w.wiz_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn practice_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.practice();
        assert_eq!(w.wiz_level, 0.0);
        assert!(!w.just_practiced);
    }

    #[test]
    fn practice_clamps_at_max_with_large_grow_rate() {
        let mut w = Wiz::new(10.0, 100.0); // grow_rate > max
        w.practice(); // 0 + 100 → clamped to 10
        assert!((w.wiz_level - 10.0).abs() < 1e-5);
        assert!(w.just_mastered);
    }

    #[test]
    fn zero_grow_rate_never_progresses() {
        let mut w = Wiz::new(10.0, 0.0);
        w.practice();
        assert_eq!(w.wiz_level, 0.0);
        assert!(w.just_practiced); // fired even though nothing changed
    }

    // --- reset ---

    #[test]
    fn reset_clears_level() {
        let mut w = w();
        w.practice();
        w.practice();
        w.reset();
        assert_eq!(w.wiz_level, 0.0);
    }

    #[test]
    fn reset_no_op_when_at_zero() {
        let mut w = w();
        w.reset(); // already 0 — no-op
        assert_eq!(w.wiz_level, 0.0);
    }

    #[test]
    fn reset_then_practice_starts_fresh() {
        let mut w = w();
        w.practice();
        w.practice();
        w.reset();
        w.practice(); // back to 1
        assert!((w.wiz_level - 1.0).abs() < 1e-5);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_practiced() {
        let mut w = w();
        w.practice(); // just_practiced=true
        w.tick(0.016);
        assert!(!w.just_practiced);
    }

    #[test]
    fn tick_clears_just_mastered() {
        let mut w = Wiz::new(1.0, 1.0);
        w.practice(); // just_mastered=true
        w.tick(0.016);
        assert!(!w.just_mastered);
    }

    #[test]
    fn tick_does_not_decay_level() {
        let mut w = w();
        w.practice();
        w.practice();
        for _ in 0..100 {
            w.tick(1.0); // no decay
        }
        assert!((w.wiz_level - 2.0).abs() < 1e-5);
    }

    // --- is_novice / is_master ---

    #[test]
    fn is_novice_true_at_zero() {
        let w = w();
        assert!(w.is_novice());
    }

    #[test]
    fn is_novice_false_after_practice() {
        let mut w = w();
        w.practice();
        assert!(!w.is_novice());
    }

    #[test]
    fn is_novice_false_when_disabled() {
        let w_disabled = {
            let mut w = w();
            w.enabled = false;
            w
        };
        // level=0, but disabled → false
        assert!(!w_disabled.is_novice());
    }

    #[test]
    fn is_master_false_at_zero() {
        let w = w();
        assert!(!w.is_master());
    }

    #[test]
    fn is_master_true_at_cap() {
        let mut w = Wiz::new(3.0, 1.0);
        w.practice();
        w.practice();
        w.practice();
        assert!(w.is_master());
    }

    #[test]
    fn is_master_false_when_disabled() {
        let mut w = Wiz::new(1.0, 1.0);
        w.practice();
        w.enabled = false;
        assert!(!w.is_master());
    }

    // --- wiz_fraction ---

    #[test]
    fn wiz_fraction_zero_at_novice() {
        let w = w();
        assert_eq!(w.wiz_fraction(), 0.0);
    }

    #[test]
    fn wiz_fraction_at_partial() {
        let mut w = Wiz::new(4.0, 1.0);
        w.practice();
        w.practice(); // 2/4 = 0.5
        assert!((w.wiz_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn wiz_fraction_one_at_master() {
        let mut w = Wiz::new(3.0, 1.0);
        w.practice();
        w.practice();
        w.practice();
        assert!((w.wiz_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_skill ---

    #[test]
    fn effective_skill_passthrough_at_novice() {
        let w = w();
        assert!((w.effective_skill(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_skill_scaled_at_partial() {
        let mut w = Wiz::new(4.0, 1.0);
        w.practice();
        w.practice(); // fraction=0.5 → 100*(1+0.5)=150
        assert!((w.effective_skill(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_skill_doubled_at_master() {
        let mut w = Wiz::new(3.0, 1.0);
        w.practice();
        w.practice();
        w.practice(); // fraction=1.0 → 100*(1+1)=200
        assert!((w.effective_skill(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_skill_passthrough_when_disabled() {
        let mut w = Wiz::new(3.0, 1.0);
        w.practice();
        w.practice();
        w.practice();
        w.enabled = false;
        assert!((w.effective_skill(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_wiz_clamped_to_one() {
        let w = Wiz::new(0.0, 1.0);
        assert!((w.max_wiz - 1.0).abs() < 1e-5);
    }

    #[test]
    fn grow_rate_clamped_to_zero() {
        let w = Wiz::new(10.0, -1.0);
        assert_eq!(w.grow_rate, 0.0);
    }

    // --- ratchet property ---

    #[test]
    fn level_never_decreases_without_reset() {
        let mut w = w();
        w.practice();
        w.practice();
        let level_before = w.wiz_level;
        w.tick(100.0); // no decay
        w.tick(100.0);
        assert!((w.wiz_level - level_before).abs() < 1e-5);
    }

    #[test]
    fn many_practices_accumulate_correctly() {
        let mut w = Wiz::new(10.0, 0.5); // 0.5/practice
        for _ in 0..20 {
            w.practice(); // 20 * 0.5 = 10 → capped
        }
        assert!((w.wiz_level - 10.0).abs() < 1e-4);
    }
}
