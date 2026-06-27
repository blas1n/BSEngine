use bevy_ecs::prelude::Component;

/// Stack-based thermal burn that amplifies incoming damage. External sources
/// apply heat stacks via `apply_stack(n)`; while stacks remain, the entity
/// takes increased incoming damage via `effective_incoming()`. Each stack
/// decays after `stack_duration` seconds — `tick(dt)` removes one stack per
/// interval and fires `just_cooled` when the last one is gone.
///
/// `apply_stack(n)` adds `n` heat stacks (capped at `max_stacks`). Fires
/// `just_scalded` on the first application (0 → nonzero transition). Refreshes
/// the per-stack decay timer to `stack_duration` on every successful apply. No-op
/// when disabled or `n == 0`.
///
/// `tick(dt)` clears one-frame flags at start; counts down `stack_timer`;
/// when the timer reaches zero with stacks remaining, removes one stack and
/// fires `just_cooled` if the count reaches zero, then resets the timer for the
/// next stack. No-op when disabled or no stacks remain.
///
/// `is_scalded()` returns `heat_stacks > 0 && enabled`.
///
/// `stack_fraction()` returns `(heat_stacks / max_stacks).clamp(0, 1)`.
///
/// `effective_incoming(base)` returns
/// `base * (1.0 + amplify_per_stack * heat_stacks)` when scalded and enabled;
/// returns `base` otherwise.
///
/// Distinct from `Burn` (fire DoT that deals outgoing damage per tick),
/// `Ignite` (sets the entity ablaze triggering fire spread), `Blaze` (fire
/// aura that emits heat to nearby entities), and `Scorch` (ground-scorching
/// terrain effect): Scald is a **stack-based incoming amplifier** — each
/// application of scalding liquid or steam adds heat stacks that make the
/// entity more vulnerable to damage; stacks decay one-by-one over time.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Scald {
    /// Current heat stack count.
    pub heat_stacks: u32,
    /// Maximum heat stacks. Clamped >= 1.
    pub max_stacks: u32,
    /// Multiplicative incoming damage amplifier per stack. Clamped >= 0.0.
    /// Total amplifier = amplify_per_stack * heat_stacks.
    pub amplify_per_stack: f32,
    /// Countdown timer for the current top stack. Resets on each stack removal.
    pub stack_timer: f32,
    /// Duration of each stack in seconds. Clamped >= 0.1.
    pub stack_duration: f32,
    pub just_scalded: bool,
    pub just_cooled: bool,
    pub enabled: bool,
}

impl Scald {
    pub fn new(max_stacks: u32, amplify_per_stack: f32, stack_duration: f32) -> Self {
        Self {
            heat_stacks: 0,
            max_stacks: max_stacks.max(1),
            amplify_per_stack: amplify_per_stack.max(0.0),
            stack_timer: 0.0,
            stack_duration: stack_duration.max(0.1),
            just_scalded: false,
            just_cooled: false,
            enabled: true,
        }
    }

    /// Apply `n` heat stacks. Caps at `max_stacks`. Refreshes the decay timer.
    /// Fires `just_scalded` on first application. No-op when disabled or `n == 0`.
    pub fn apply_stack(&mut self, n: u32) {
        if !self.enabled || n == 0 {
            return;
        }
        let was_zero = self.heat_stacks == 0;
        self.heat_stacks = (self.heat_stacks + n).min(self.max_stacks);
        self.stack_timer = self.stack_duration;
        if was_zero {
            self.just_scalded = true;
        }
    }

    /// Advance the stack decay timer. Clears one-frame flags first. When the
    /// timer expires, removes one stack; if the last stack is removed, fires
    /// `just_cooled` and stops. No-op when disabled or no stacks remain.
    pub fn tick(&mut self, dt: f32) {
        self.just_scalded = false;
        self.just_cooled = false;

        if !self.enabled || self.heat_stacks == 0 {
            return;
        }

        self.stack_timer = (self.stack_timer - dt).max(0.0);
        if self.stack_timer == 0.0 {
            self.heat_stacks -= 1;
            if self.heat_stacks == 0 {
                self.just_cooled = true;
            } else {
                self.stack_timer = self.stack_duration;
            }
        }
    }

    /// `true` when any heat stacks remain and the component is enabled.
    pub fn is_scalded(&self) -> bool {
        self.heat_stacks > 0 && self.enabled
    }

    /// Fraction of maximum stacks currently applied. [0.0 = clean, 1.0 = max].
    pub fn stack_fraction(&self) -> f32 {
        (self.heat_stacks as f32 / self.max_stacks as f32).clamp(0.0, 1.0)
    }

    /// Incoming damage amplified by heat stacks.
    /// Returns `base * (1.0 + amplify_per_stack * heat_stacks)` when scalded
    /// and enabled; returns `base` otherwise.
    pub fn effective_incoming(&self, base: f32) -> f32 {
        if self.is_scalded() {
            base * (1.0 + self.amplify_per_stack * self.heat_stacks as f32)
        } else {
            base
        }
    }
}

impl Default for Scald {
    fn default() -> Self {
        Self::new(5, 0.1, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_with_no_stacks() {
        let s = Scald::new(5, 0.1, 3.0);
        assert_eq!(s.heat_stacks, 0);
        assert!(!s.is_scalded());
    }

    #[test]
    fn apply_stack_adds_stacks() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.apply_stack(2);
        assert_eq!(s.heat_stacks, 2);
        assert!(s.is_scalded());
    }

    #[test]
    fn apply_stack_fires_just_scalded_on_first() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.apply_stack(1);
        assert!(s.just_scalded);
    }

    #[test]
    fn apply_stack_no_just_scalded_on_reapply() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.apply_stack(1);
        s.tick(0.0); // clear flags
        s.apply_stack(1);
        assert!(!s.just_scalded);
    }

    #[test]
    fn apply_stack_caps_at_max() {
        let mut s = Scald::new(3, 0.1, 3.0);
        s.apply_stack(10);
        assert_eq!(s.heat_stacks, 3);
    }

    #[test]
    fn apply_stack_no_op_when_disabled() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.enabled = false;
        s.apply_stack(2);
        assert_eq!(s.heat_stacks, 0);
    }

    #[test]
    fn apply_stack_no_op_when_n_zero() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.apply_stack(0);
        assert_eq!(s.heat_stacks, 0);
    }

    #[test]
    fn apply_stack_refreshes_timer() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.apply_stack(1);
        s.tick(2.0); // timer drops to 1.0
        s.apply_stack(1); // refreshes to 3.0
        assert!((s.stack_timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_counts_down_timer() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.apply_stack(2);
        s.tick(1.0);
        assert!((s.stack_timer - 2.0).abs() < 1e-5);
        assert_eq!(s.heat_stacks, 2);
    }

    #[test]
    fn tick_removes_one_stack_on_expiry() {
        let mut s = Scald::new(5, 0.1, 1.0);
        s.apply_stack(3);
        s.tick(1.0); // first stack expires
        assert_eq!(s.heat_stacks, 2);
        assert!(!s.just_cooled);
    }

    #[test]
    fn tick_resets_timer_after_stack_removed() {
        let mut s = Scald::new(5, 0.1, 1.0);
        s.apply_stack(2);
        s.tick(1.0); // removes one stack
        assert!((s.stack_timer - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_cooled_on_last_stack_removed() {
        let mut s = Scald::new(5, 0.1, 1.0);
        s.apply_stack(1);
        s.tick(1.0);
        assert!(s.just_cooled);
        assert_eq!(s.heat_stacks, 0);
    }

    #[test]
    fn tick_no_just_cooled_when_stacks_remain() {
        let mut s = Scald::new(5, 0.1, 1.0);
        s.apply_stack(3);
        s.tick(1.0);
        assert!(!s.just_cooled);
    }

    #[test]
    fn tick_clears_just_scalded() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.apply_stack(1);
        s.tick(0.016);
        assert!(!s.just_scalded);
    }

    #[test]
    fn tick_clears_just_cooled_next_frame() {
        let mut s = Scald::new(5, 0.1, 1.0);
        s.apply_stack(1);
        s.tick(1.0); // just_cooled = true
        s.tick(0.016); // cleared
        assert!(!s.just_cooled);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut s = Scald::new(5, 0.1, 1.0);
        s.apply_stack(2);
        s.enabled = false;
        s.tick(1.0);
        assert_eq!(s.heat_stacks, 2); // unchanged
    }

    #[test]
    fn tick_no_op_when_no_stacks() {
        let mut s = Scald::new(5, 0.1, 1.0);
        s.tick(10.0); // no panic, no change
        assert_eq!(s.heat_stacks, 0);
    }

    #[test]
    fn is_scalded_false_when_no_stacks() {
        let s = Scald::new(5, 0.1, 3.0);
        assert!(!s.is_scalded());
    }

    #[test]
    fn is_scalded_false_when_disabled() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.heat_stacks = 3;
        s.enabled = false;
        assert!(!s.is_scalded());
    }

    #[test]
    fn stack_fraction_at_zero() {
        let s = Scald::new(5, 0.1, 3.0);
        assert_eq!(s.stack_fraction(), 0.0);
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut s = Scald::new(4, 0.1, 3.0);
        s.apply_stack(2);
        assert!((s.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_full() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.apply_stack(5);
        assert!((s.stack_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_amplified_by_stacks() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.apply_stack(3);
        // 100 * (1.0 + 0.1 * 3) = 100 * 1.3 = 130
        assert!((s.effective_incoming(100.0) - 130.0).abs() < 1e-3);
    }

    #[test]
    fn effective_incoming_base_when_no_stacks() {
        let s = Scald::new(5, 0.1, 3.0);
        assert!((s.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_base_when_disabled() {
        let mut s = Scald::new(5, 0.1, 3.0);
        s.heat_stacks = 3;
        s.enabled = false;
        assert!((s.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_base_after_all_stacks_expire() {
        let mut s = Scald::new(5, 0.1, 1.0);
        s.apply_stack(1);
        s.tick(1.0);
        assert!((s.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn max_stacks_clamped_to_one() {
        let s = Scald::new(0, 0.1, 3.0);
        assert_eq!(s.max_stacks, 1);
    }

    #[test]
    fn amplify_per_stack_clamped_to_zero() {
        let s = Scald::new(5, -0.5, 3.0);
        assert_eq!(s.amplify_per_stack, 0.0);
    }

    #[test]
    fn stack_duration_clamped_to_minimum() {
        let s = Scald::new(5, 0.1, 0.0);
        assert!((s.stack_duration - 0.1).abs() < 1e-5);
    }

    #[test]
    fn reapply_after_cooled_fires_just_scalded_again() {
        let mut s = Scald::new(5, 0.1, 1.0);
        s.apply_stack(1);
        s.tick(0.0); // clear flags
        s.tick(1.0); // expires, just_cooled=true
        s.apply_stack(1);
        assert!(s.just_scalded);
    }

    #[test]
    fn multiple_stacks_decay_one_per_interval() {
        let mut s = Scald::new(5, 0.1, 1.0);
        s.apply_stack(3);
        s.tick(1.0); // 3→2
        assert_eq!(s.heat_stacks, 2);
        s.tick(1.0); // 2→1
        assert_eq!(s.heat_stacks, 1);
        s.tick(1.0); // 1→0
        assert_eq!(s.heat_stacks, 0);
        assert!(s.just_cooled);
    }
}
