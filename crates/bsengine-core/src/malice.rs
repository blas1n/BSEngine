use bevy_ecs::prelude::Component;

/// Stacking damage-amplification debuff that builds under sustained attack.
///
/// Each time the target is hit, the attacker calls `add_stack()` to accumulate
/// one stack (up to `max_stacks`). The damage pipeline multiplies incoming
/// damage by `total_multiplier()`, which grows with each stack.
///
/// Stacks decay passively: `tick(dt)` advances `decay_timer`; when it reaches
/// `decay_interval`, one stack is removed and the timer resets. A hit resets
/// the decay timer, making sustained combat keep stacks high.
///
/// Distinct from `Expose` (flat one-time vulnerability), `Corrupt` (elemental
/// affinity shift), and `Weaken` (attacker-side damage penalty): Malice is a
/// growing pressure debuff — the longer a target is under attack without
/// respite, the more damage they take.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Malice {
    pub stacks: u32,
    pub max_stacks: u32,
    /// Incoming damage multiplied by `1 + stacks * damage_amplify_per_stack`.
    /// e.g. 0.05 = +5% per stack. Clamped to [0.0, 1.0] per stack.
    pub damage_amplify_per_stack: f32,
    /// Seconds between automatic removal of one stack when not hit.
    pub decay_interval: f32,
    pub decay_timer: f32,
    pub just_stacked: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Malice {
    pub fn new(damage_amplify_per_stack: f32, max_stacks: u32, decay_interval: f32) -> Self {
        Self {
            stacks: 0,
            max_stacks: max_stacks.max(1),
            damage_amplify_per_stack: damage_amplify_per_stack.clamp(0.0, 1.0),
            decay_interval: decay_interval.max(0.0),
            decay_timer: 0.0,
            just_stacked: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Add one stack (capped at `max_stacks`) and reset the decay timer.
    /// Sets `just_stacked` only on the first stack (0 → 1 transition). No-op
    /// when disabled.
    pub fn add_stack(&mut self) {
        if !self.enabled {
            return;
        }
        let was_zero = self.stacks == 0;
        if self.stacks < self.max_stacks {
            self.stacks += 1;
        }
        self.decay_timer = 0.0; // reset decay on hit
        if was_zero {
            self.just_stacked = true;
        }
    }

    /// Remove one stack manually (e.g., from a cleanse).
    pub fn remove_stack(&mut self) {
        if self.stacks > 0 {
            self.stacks -= 1;
        }
    }

    /// Remove all stacks immediately.
    pub fn clear_all(&mut self) {
        if self.stacks > 0 {
            self.stacks = 0;
            self.decay_timer = 0.0;
            self.just_cleared = true;
        }
    }

    /// Advance the decay timer. Removes one stack per elapsed `decay_interval`
    /// while stacks remain.
    pub fn tick(&mut self, dt: f32) {
        self.just_stacked = false;
        self.just_cleared = false;

        if self.stacks > 0 && self.decay_interval > 0.0 {
            self.decay_timer += dt;
            while self.decay_timer >= self.decay_interval && self.stacks > 0 {
                self.decay_timer -= self.decay_interval;
                self.stacks -= 1;
            }
            if self.stacks == 0 {
                self.decay_timer = 0.0;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.stacks > 0
    }

    /// Incoming damage multiplier: `1.0 + stacks * damage_amplify_per_stack`.
    /// Returns `1.0` when no stacks are present.
    pub fn total_multiplier(&self) -> f32 {
        1.0 + self.stacks as f32 * self.damage_amplify_per_stack
    }

    /// Fraction of max stacks accumulated [0.0 = none, 1.0 = full].
    pub fn stack_fraction(&self) -> f32 {
        if self.max_stacks == 0 {
            return 0.0;
        }
        (self.stacks as f32 / self.max_stacks as f32).clamp(0.0, 1.0)
    }
}

impl Default for Malice {
    fn default() -> Self {
        Self::new(0.05, 10, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_stack_increments_stacks() {
        let mut m = Malice::new(0.05, 10, 3.0);
        m.add_stack();
        assert_eq!(m.stacks, 1);
        assert!(m.is_active());
        assert!(m.just_stacked);
    }

    #[test]
    fn add_stack_caps_at_max() {
        let mut m = Malice::new(0.05, 3, 3.0);
        for _ in 0..5 {
            m.add_stack();
            m.tick(0.016);
        }
        assert_eq!(m.stacks, 3);
    }

    #[test]
    fn just_stacked_only_on_first_stack() {
        let mut m = Malice::new(0.05, 10, 3.0);
        m.add_stack();
        m.tick(0.016);
        m.add_stack(); // second stack — should not set just_stacked
        assert!(!m.just_stacked);
    }

    #[test]
    fn add_stack_resets_decay_timer() {
        let mut m = Malice::new(0.05, 10, 3.0);
        m.add_stack();
        m.tick(2.0); // advance decay timer
        m.add_stack(); // hit resets timer
        assert!(m.decay_timer < 1e-4);
    }

    #[test]
    fn remove_stack_decrements() {
        let mut m = Malice::new(0.05, 10, 3.0);
        m.add_stack();
        m.add_stack();
        m.remove_stack();
        assert_eq!(m.stacks, 1);
    }

    #[test]
    fn clear_all_removes_all_stacks() {
        let mut m = Malice::new(0.05, 10, 3.0);
        m.add_stack();
        m.add_stack();
        m.clear_all();
        assert_eq!(m.stacks, 0);
        assert!(m.just_cleared);
        assert!(!m.is_active());
    }

    #[test]
    fn tick_decays_one_stack_per_interval() {
        let mut m = Malice::new(0.05, 10, 2.0);
        m.add_stack();
        m.add_stack();
        m.tick(0.016); // reset just_stacked
        m.tick(2.1); // one interval
        assert_eq!(m.stacks, 1);
    }

    #[test]
    fn tick_decays_multiple_stacks_fast() {
        let mut m = Malice::new(0.05, 10, 1.0);
        for _ in 0..3 {
            m.add_stack();
        }
        m.tick(0.016);
        m.tick(3.5); // 3 full intervals
        assert_eq!(m.stacks, 0);
    }

    #[test]
    fn total_multiplier_no_stacks() {
        let m = Malice::new(0.05, 10, 3.0);
        assert!((m.total_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn total_multiplier_with_stacks() {
        let mut m = Malice::new(0.1, 10, 3.0);
        m.add_stack();
        m.add_stack();
        // 1.0 + 2 * 0.1 = 1.2
        assert!((m.total_multiplier() - 1.2).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut m = Malice::new(0.05, 10, 3.0);
        for _ in 0..5 {
            m.add_stack();
            m.tick(0.016);
        }
        assert!((m.stack_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_add_stack_no_op() {
        let mut m = Malice::new(0.05, 10, 3.0);
        m.enabled = false;
        m.add_stack();
        assert_eq!(m.stacks, 0);
    }

    #[test]
    fn tick_clears_just_stacked() {
        let mut m = Malice::new(0.05, 10, 3.0);
        m.add_stack();
        m.tick(0.016);
        assert!(!m.just_stacked);
    }
}
