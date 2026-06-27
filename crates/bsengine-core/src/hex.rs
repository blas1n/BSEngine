use bevy_ecs::prelude::Component;

/// Stacking debuff that reduces a target stat by a fixed fraction per stack.
///
/// Each `apply()` call adds one stack and resets the duration timer. The caller
/// decides which stat to penalize — typically move-speed, attack-speed, or
/// resource-regen — by multiplying the target value by `stat_multiplier()`.
///
/// Stacks cap at `max_stacks`. `tick(dt)` counts down and removes all stacks
/// on expiry (one timer shared across all stacks). `clear()` removes instantly.
///
/// `total_reduction()` returns `stacks * reduction_per_stack`, clamped to
/// `[0.0, 1.0]`, so `stat_multiplier()` is always in `[0.0, 1.0]`.
///
/// Distinct from `Curse` (typed, single-instance), `Weaken` (outgoing damage),
/// and `Slow` (movement only): Hex models a generic accumulating penalty that
/// callers apply to whichever stat makes sense for their game.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Hex {
    pub stacks: u32,
    pub max_stacks: u32,
    pub duration: f32,
    pub timer: f32,
    /// Stat fraction removed per stack [0.0, 1.0].
    pub reduction_per_stack: f32,
    pub just_applied: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Hex {
    pub fn new(reduction_per_stack: f32, max_stacks: u32) -> Self {
        Self {
            stacks: 0,
            max_stacks: max_stacks.max(1),
            duration: 0.0,
            timer: 0.0,
            reduction_per_stack: reduction_per_stack.clamp(0.0, 1.0),
            just_applied: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Add one stack (capped at `max_stacks`) and reset the duration timer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        if self.stacks < self.max_stacks {
            self.stacks += 1;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_applied = true;
    }

    /// Add `n` stacks at once and reset the timer.
    pub fn apply_n(&mut self, n: u32, duration: f32) {
        if !self.enabled || n == 0 {
            return;
        }
        self.stacks = (self.stacks + n).min(self.max_stacks);
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_applied = true;
    }

    /// Remove all stacks and stop the timer.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.stacks = 0;
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_expired = true;
        }
    }

    /// Advance the timer; removes all stacks and sets `just_expired` when it runs out.
    pub fn tick(&mut self, dt: f32) {
        self.just_applied = false;
        self.just_expired = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.stacks = 0;
                self.just_expired = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.stacks > 0
    }

    /// Combined stat reduction [0.0, 1.0]. Multiply the target stat by
    /// `stat_multiplier()` to apply the hex.
    pub fn total_reduction(&self) -> f32 {
        (self.stacks as f32 * self.reduction_per_stack).clamp(0.0, 1.0)
    }

    /// Remaining stat fraction [0.0 = fully penalized, 1.0 = no penalty].
    pub fn stat_multiplier(&self) -> f32 {
        1.0 - self.total_reduction()
    }

    /// Fraction of the current timer remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Hex {
    fn default() -> Self {
        Self::new(0.1, 5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_adds_stack() {
        let mut h = Hex::new(0.1, 5);
        h.apply(3.0);
        assert_eq!(h.stacks, 1);
        assert!(h.just_applied);
        assert!(h.is_active());
    }

    #[test]
    fn apply_resets_timer() {
        let mut h = Hex::new(0.1, 5);
        h.apply(3.0);
        h.tick(2.0);
        h.apply(5.0);
        assert!((h.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_caps_at_max_stacks() {
        let mut h = Hex::new(0.1, 3);
        h.apply(5.0);
        h.apply(5.0);
        h.apply(5.0);
        h.apply(5.0); // 4th — should stay at 3
        assert_eq!(h.stacks, 3);
    }

    #[test]
    fn apply_n_adds_multiple_stacks() {
        let mut h = Hex::new(0.1, 5);
        h.apply_n(3, 4.0);
        assert_eq!(h.stacks, 3);
    }

    #[test]
    fn apply_n_caps_at_max_stacks() {
        let mut h = Hex::new(0.1, 3);
        h.apply_n(10, 5.0);
        assert_eq!(h.stacks, 3);
    }

    #[test]
    fn tick_expires_all_stacks() {
        let mut h = Hex::new(0.1, 5);
        h.apply(1.0);
        h.apply(1.0);
        h.tick(1.1);
        assert_eq!(h.stacks, 0);
        assert!(!h.is_active());
        assert!(h.just_expired);
    }

    #[test]
    fn clear_removes_all_stacks() {
        let mut h = Hex::new(0.1, 5);
        h.apply(5.0);
        h.apply(5.0);
        h.clear();
        assert_eq!(h.stacks, 0);
        assert!(h.just_expired);
    }

    #[test]
    fn total_reduction_scales_with_stacks() {
        let mut h = Hex::new(0.2, 5);
        h.apply_n(3, 5.0);
        assert!((h.total_reduction() - 0.6).abs() < 1e-5);
    }

    #[test]
    fn total_reduction_clamped_to_one() {
        let mut h = Hex::new(0.5, 5);
        h.apply_n(5, 5.0); // 5 * 0.5 = 2.5, clamped to 1.0
        assert!((h.total_reduction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn stat_multiplier_at_zero_stacks() {
        let h = Hex::new(0.1, 5);
        assert!((h.stat_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn stat_multiplier_with_stacks() {
        let mut h = Hex::new(0.1, 5);
        h.apply_n(3, 5.0);
        assert!((h.stat_multiplier() - 0.7).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut h = Hex::new(0.1, 5);
        h.apply(2.0);
        h.tick(1.0);
        assert!((h.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut h = Hex::new(0.1, 5);
        h.enabled = false;
        h.apply(5.0);
        assert_eq!(h.stacks, 0);
    }

    #[test]
    fn tick_clears_just_applied() {
        let mut h = Hex::new(0.1, 5);
        h.apply(5.0);
        h.tick(0.016);
        assert!(!h.just_applied);
    }
}
