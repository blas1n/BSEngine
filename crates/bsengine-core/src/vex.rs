use bevy_ecs::prelude::Component;

/// Stacking CC-amplification debuff: each stack extends the duration of crowd-
/// control effects applied to this entity (stuns, slows, freezes, etc.).
///
/// When the entity receives a CC effect, the applying system should call
/// `amplified_duration(base_duration)` to compute the extended duration.
/// Call `add_stack()` after each successful CC to accumulate stacks (capped
/// at `max_stacks`). Stacks are removed by `remove_stack()`, `clear_all()`,
/// or external timers.
///
/// `tick()` clears one-frame flags `just_vexed` and `just_cleared`.
///
/// Distinct from `Curse` (generic debuff container), `Hex` (converts active
/// buffs into debuffs), and `Slow` (movement-speed penalty): Vex is a
/// **CC amplifier** — it doesn't impose a CC itself, it makes every incoming
/// CC last proportionally longer the more stacks are present.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vex {
    pub stacks: u32,
    pub max_stacks: u32,
    /// Fraction of base CC duration added per stack. Clamped to [0.0, 1.0].
    /// e.g. 0.2 = each stack extends CC duration by 20%.
    pub cc_duration_amplify_per_stack: f32,
    pub just_vexed: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Vex {
    pub fn new(cc_duration_amplify_per_stack: f32, max_stacks: u32) -> Self {
        Self {
            stacks: 0,
            max_stacks: max_stacks.max(1),
            cc_duration_amplify_per_stack: cc_duration_amplify_per_stack.clamp(0.0, 1.0),
            just_vexed: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Add one stack (capped at `max_stacks`). Sets `just_vexed` on the first
    /// stack (0 → 1 transition). No-op when disabled.
    pub fn add_stack(&mut self) {
        if !self.enabled {
            return;
        }
        let was_zero = self.stacks == 0;
        if self.stacks < self.max_stacks {
            self.stacks += 1;
        }
        if was_zero && self.stacks > 0 {
            self.just_vexed = true;
        }
    }

    /// Remove one stack manually (e.g., from a cleanse or stack-shedding mechanic).
    pub fn remove_stack(&mut self) {
        if self.stacks > 0 {
            self.stacks -= 1;
        }
    }

    /// Remove all stacks immediately. Sets `just_cleared`.
    pub fn clear_all(&mut self) {
        if self.stacks > 0 {
            self.stacks = 0;
            self.just_cleared = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_vexed = false;
        self.just_cleared = false;
    }

    pub fn is_active(&self) -> bool {
        self.stacks > 0
    }

    /// Total CC duration multiplier: `1.0 + stacks * cc_duration_amplify_per_stack`.
    /// Returns `1.0` when no stacks are present.
    pub fn total_amplify(&self) -> f32 {
        1.0 + self.stacks as f32 * self.cc_duration_amplify_per_stack
    }

    /// Extends `base_duration` by the current vex amplifier. Returns
    /// `base_duration * total_amplify()` when active and enabled,
    /// `base_duration` otherwise.
    pub fn amplified_duration(&self, base_duration: f32) -> f32 {
        if self.is_active() && self.enabled {
            base_duration * self.total_amplify()
        } else {
            base_duration
        }
    }

    /// Fraction of max stacks accumulated [0.0 = none, 1.0 = capped].
    pub fn stack_fraction(&self) -> f32 {
        if self.max_stacks == 0 {
            return 0.0;
        }
        (self.stacks as f32 / self.max_stacks as f32).clamp(0.0, 1.0)
    }
}

impl Default for Vex {
    fn default() -> Self {
        Self::new(0.2, 5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_stack_increments() {
        let mut v = Vex::new(0.2, 5);
        v.add_stack();
        assert_eq!(v.stacks, 1);
        assert!(v.is_active());
        assert!(v.just_vexed);
    }

    #[test]
    fn add_stack_caps_at_max() {
        let mut v = Vex::new(0.2, 3);
        for _ in 0..5 {
            v.add_stack();
            v.tick();
        }
        assert_eq!(v.stacks, 3);
    }

    #[test]
    fn just_vexed_only_on_first_stack() {
        let mut v = Vex::new(0.2, 5);
        v.add_stack();
        v.tick();
        v.add_stack(); // second stack
        assert!(!v.just_vexed);
    }

    #[test]
    fn remove_stack_decrements() {
        let mut v = Vex::new(0.2, 5);
        v.add_stack();
        v.add_stack();
        v.remove_stack();
        assert_eq!(v.stacks, 1);
    }

    #[test]
    fn remove_stack_no_underflow() {
        let mut v = Vex::new(0.2, 5);
        v.remove_stack(); // stacks = 0, no-op
        assert_eq!(v.stacks, 0);
    }

    #[test]
    fn clear_all_removes_all_stacks() {
        let mut v = Vex::new(0.2, 5);
        v.add_stack();
        v.add_stack();
        v.clear_all();
        assert_eq!(v.stacks, 0);
        assert!(v.just_cleared);
        assert!(!v.is_active());
    }

    #[test]
    fn tick_clears_just_vexed() {
        let mut v = Vex::new(0.2, 5);
        v.add_stack();
        v.tick();
        assert!(!v.just_vexed);
    }

    #[test]
    fn tick_clears_just_cleared() {
        let mut v = Vex::new(0.2, 5);
        v.add_stack();
        v.clear_all();
        v.tick();
        assert!(!v.just_cleared);
    }

    #[test]
    fn total_amplify_no_stacks() {
        let v = Vex::new(0.2, 5);
        assert!((v.total_amplify() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn total_amplify_with_stacks() {
        let mut v = Vex::new(0.2, 5);
        v.add_stack();
        v.add_stack();
        // 1.0 + 2 * 0.2 = 1.4
        assert!((v.total_amplify() - 1.4).abs() < 1e-5);
    }

    #[test]
    fn amplified_duration_extended() {
        let mut v = Vex::new(0.2, 5);
        v.add_stack();
        v.add_stack();
        // 3.0 * 1.4 = 4.2
        assert!((v.amplified_duration(3.0) - 4.2).abs() < 1e-4);
    }

    #[test]
    fn amplified_duration_base_when_no_stacks() {
        let v = Vex::new(0.2, 5);
        assert!((v.amplified_duration(3.0) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut v = Vex::new(0.2, 4);
        v.add_stack();
        v.tick();
        v.add_stack();
        assert!((v.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_add_stack_no_op() {
        let mut v = Vex::new(0.2, 5);
        v.enabled = false;
        v.add_stack();
        assert_eq!(v.stacks, 0);
    }

    #[test]
    fn disabled_amplified_duration_base() {
        let mut v = Vex::new(0.2, 5);
        v.add_stack();
        v.enabled = false;
        assert!((v.amplified_duration(3.0) - 3.0).abs() < 1e-5);
    }
}
