use bevy_ecs::prelude::Component;

/// Stacking slashing debuff where each new hit adds a stack and refreshes the
/// full duration. Damage per second scales linearly with stack count.
///
/// `apply(duration)` adds one stack (capped at `max_stacks`) and resets the
/// timer to `duration` regardless of the current timer. `tick(dt)` returns
/// `stacks * damage_per_stack_per_second * dt` and sets `just_closed` when
/// the debuff expires. `clear()` removes all stacks immediately.
///
/// Distinct from `Bleed` (single-instance flat DoT that high-watermarks on
/// duration): Lacerate stacks accumulate, and each new hit refreshes the full
/// window rather than competing with the existing timer.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Lacerate {
    pub duration: f32,
    pub timer: f32,
    /// Current stack count [0, max_stacks].
    pub stacks: u32,
    /// Maximum stacks that can accumulate.
    pub max_stacks: u32,
    /// Damage dealt per stack per second.
    pub damage_per_stack_per_second: f32,
    pub just_lacerated: bool,
    pub just_closed: bool,
    pub enabled: bool,
}

impl Lacerate {
    pub fn new(damage_per_stack_per_second: f32, max_stacks: u32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            stacks: 0,
            max_stacks: max_stacks.max(1),
            damage_per_stack_per_second: damage_per_stack_per_second.max(0.0),
            just_lacerated: false,
            just_closed: false,
            enabled: true,
        }
    }

    /// Add one stack and refresh the full duration. No-op when disabled.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        let was_active = self.is_active();
        if self.stacks < self.max_stacks {
            self.stacks += 1;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        if !was_active {
            self.just_lacerated = true;
        }
    }

    /// Remove all stacks immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.stacks = 0;
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_closed = true;
        }
    }

    /// Advance the timer and return damage dealt this frame.
    /// Sets `just_closed` when the debuff expires.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_lacerated = false;
        self.just_closed = false;

        if !self.is_active() {
            return 0.0;
        }

        let damage = self.stacks as f32 * self.damage_per_stack_per_second * dt;

        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer = 0.0;
            self.duration = 0.0;
            self.stacks = 0;
            self.just_closed = true;
        }

        damage
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0 && self.stacks > 0
    }

    /// Fraction of the current duration remaining [1.0 = just applied, 0.0 = closed].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Lacerate {
    fn default() -> Self {
        Self::new(5.0, 5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_and_adds_stack() {
        let mut l = Lacerate::new(5.0, 5);
        l.apply(3.0);
        assert!(l.is_active());
        assert_eq!(l.stacks, 1);
        assert!(l.just_lacerated);
    }

    #[test]
    fn apply_accumulates_stacks() {
        let mut l = Lacerate::new(5.0, 5);
        l.apply(3.0);
        l.tick(0.016);
        l.apply(3.0);
        assert_eq!(l.stacks, 2);
    }

    #[test]
    fn apply_capped_at_max_stacks() {
        let mut l = Lacerate::new(5.0, 3);
        l.apply(3.0);
        l.apply(3.0);
        l.apply(3.0);
        l.apply(3.0); // 4th should be capped
        assert_eq!(l.stacks, 3);
    }

    #[test]
    fn apply_refreshes_timer() {
        let mut l = Lacerate::new(5.0, 5);
        l.apply(5.0);
        l.tick(4.0);
        l.apply(5.0); // refresh to full 5 seconds
        assert!((l.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_returns_damage() {
        let mut l = Lacerate::new(10.0, 5);
        l.apply(3.0);
        l.apply(3.0); // 2 stacks
        let dmg = l.tick(1.0);
        assert!((dmg - 20.0).abs() < 1e-4); // 2 stacks * 10 dps * 1s
    }

    #[test]
    fn tick_expires_lacerate() {
        let mut l = Lacerate::new(5.0, 5);
        l.apply(1.0);
        l.tick(1.1);
        assert!(!l.is_active());
        assert!(l.just_closed);
        assert_eq!(l.stacks, 0);
    }

    #[test]
    fn clear_ends_early() {
        let mut l = Lacerate::new(5.0, 5);
        l.apply(5.0);
        l.clear();
        assert!(!l.is_active());
        assert!(l.just_closed);
        assert_eq!(l.stacks, 0);
    }

    #[test]
    fn tick_zero_when_inactive() {
        let mut l = Lacerate::new(5.0, 5);
        let dmg = l.tick(0.1);
        assert_eq!(dmg, 0.0);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut l = Lacerate::new(5.0, 5);
        l.apply(2.0);
        l.tick(1.0);
        assert!((l.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut l = Lacerate::new(5.0, 5);
        l.enabled = false;
        l.apply(3.0);
        assert!(!l.is_active());
    }

    #[test]
    fn tick_clears_just_lacerated() {
        let mut l = Lacerate::new(5.0, 5);
        l.apply(3.0);
        l.tick(0.016);
        assert!(!l.just_lacerated);
    }
}
