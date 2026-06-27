use bevy_ecs::prelude::Component;

/// Cold-stacking status: entity accumulates chill stacks (applied by attacks
/// or auras from cold sources). Each stack reduces movement speed by
/// `speed_reduction_per_stack`. When stacks reach `max_stacks` the entity
/// is considered frozen and `just_froze` fires once.
///
/// `apply(count)` adds stacks, capping at `max_stacks` and firing `just_froze`
/// on the transition that reaches the cap. No-op when disabled.
///
/// `dispel(count)` silently removes stacks (no event). No-op when disabled.
///
/// `tick()` clears `just_froze` each frame.
///
/// `effective_speed(base)` returns
/// `(base * (1 - speed_reduction_per_stack * chill_stacks)).max(0.0)` when
/// enabled; returns `base` otherwise.
///
/// `is_frozen()` returns `chill_stacks >= max_stacks && enabled`.
///
/// Distinct from `Freeze` (the fully-immobilized state entered from chill),
/// `Slow` (generic speed reduction applied directly without stacking),
/// and `Frostbite` (cold damage-over-time to HP): Chill is a **cold-stack
/// accumulator** — slow penalties stack up per hit, and crossing the freeze
/// threshold signals other systems to transition to full immobility.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Chill {
    /// Current chill stacks [0, max_stacks].
    pub chill_stacks: u32,
    /// Stack count at which the entity is considered frozen. Clamped ≥ 1.
    pub max_stacks: u32,
    /// Speed reduction fraction per stack. Clamped [0.0, 1.0).
    /// e.g. 0.1 means each stack removes 10% of base speed.
    pub speed_reduction_per_stack: f32,
    pub just_froze: bool,
    pub enabled: bool,
}

impl Chill {
    pub fn new(max_stacks: u32, speed_reduction_per_stack: f32) -> Self {
        Self {
            chill_stacks: 0,
            max_stacks: max_stacks.max(1),
            speed_reduction_per_stack: speed_reduction_per_stack.clamp(0.0, 1.0),
            just_froze: false,
            enabled: true,
        }
    }

    /// Add `count` chill stacks, capping at `max_stacks`. Fires `just_froze`
    /// on the transition that first reaches `max_stacks`. No-op when disabled
    /// or `count == 0`.
    pub fn apply(&mut self, count: u32) {
        if !self.enabled || count == 0 {
            return;
        }
        let was_frozen = self.is_frozen();
        self.chill_stacks = (self.chill_stacks + count).min(self.max_stacks);
        if !was_frozen && self.is_frozen() {
            self.just_froze = true;
        }
    }

    /// Remove `count` chill stacks, flooring at 0. Silent — fires no event.
    /// No-op when disabled or `count == 0`.
    pub fn dispel(&mut self, count: u32) {
        if !self.enabled || count == 0 {
            return;
        }
        self.chill_stacks = self.chill_stacks.saturating_sub(count);
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_froze = false;
    }

    /// `true` when stacks have reached the freeze threshold and enabled.
    pub fn is_frozen(&self) -> bool {
        self.chill_stacks >= self.max_stacks && self.enabled
    }

    /// Effective movement speed after chill penalty.
    /// Returns `(base * (1 - speed_reduction_per_stack * chill_stacks)).max(0.0)`
    /// when enabled; returns `base` otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.enabled {
            let reduction = self.speed_reduction_per_stack * self.chill_stacks as f32;
            (base * (1.0 - reduction)).max(0.0)
        } else {
            base
        }
    }

    /// Stack fill fraction [0.0, 1.0].
    pub fn stack_fraction(&self) -> f32 {
        self.chill_stacks as f32 / self.max_stacks as f32
    }
}

impl Default for Chill {
    fn default() -> Self {
        Self::new(5, 0.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_with_no_stacks() {
        let c = Chill::new(5, 0.1);
        assert_eq!(c.chill_stacks, 0);
        assert!(!c.is_frozen());
    }

    #[test]
    fn apply_adds_stacks() {
        let mut c = Chill::new(5, 0.1);
        c.apply(2);
        assert_eq!(c.chill_stacks, 2);
    }

    #[test]
    fn apply_caps_at_max_stacks() {
        let mut c = Chill::new(5, 0.1);
        c.apply(10);
        assert_eq!(c.chill_stacks, 5);
    }

    #[test]
    fn apply_fires_just_froze_at_threshold() {
        let mut c = Chill::new(3, 0.1);
        c.apply(3);
        assert!(c.just_froze);
        assert!(c.is_frozen());
    }

    #[test]
    fn apply_no_just_froze_when_already_frozen() {
        let mut c = Chill::new(3, 0.1);
        c.apply(3); // freeze
        c.tick();
        c.apply(1); // already frozen
        assert!(!c.just_froze);
    }

    #[test]
    fn apply_just_froze_on_exact_threshold() {
        let mut c = Chill::new(3, 0.1);
        c.apply(2);
        assert!(!c.just_froze);
        c.apply(1);
        assert!(c.just_froze);
    }

    #[test]
    fn apply_no_op_when_disabled() {
        let mut c = Chill::new(5, 0.1);
        c.enabled = false;
        c.apply(5);
        assert_eq!(c.chill_stacks, 0);
    }

    #[test]
    fn apply_no_op_at_zero_count() {
        let mut c = Chill::new(5, 0.1);
        c.apply(0);
        assert_eq!(c.chill_stacks, 0);
    }

    #[test]
    fn dispel_removes_stacks() {
        let mut c = Chill::new(5, 0.1);
        c.apply(4);
        c.dispel(2);
        assert_eq!(c.chill_stacks, 2);
    }

    #[test]
    fn dispel_floors_at_zero() {
        let mut c = Chill::new(5, 0.1);
        c.apply(2);
        c.dispel(10);
        assert_eq!(c.chill_stacks, 0);
    }

    #[test]
    fn dispel_no_op_when_disabled() {
        let mut c = Chill::new(5, 0.1);
        c.chill_stacks = 3;
        c.enabled = false;
        c.dispel(2);
        assert_eq!(c.chill_stacks, 3);
    }

    #[test]
    fn tick_clears_just_froze() {
        let mut c = Chill::new(3, 0.1);
        c.apply(3);
        c.tick();
        assert!(!c.just_froze);
    }

    #[test]
    fn is_frozen_false_when_disabled() {
        let mut c = Chill::new(3, 0.1);
        c.apply(3);
        c.enabled = false;
        assert!(!c.is_frozen());
    }

    #[test]
    fn effective_speed_no_stacks() {
        let c = Chill::new(5, 0.1);
        // 0 stacks → no penalty
        assert!((c.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_two_stacks() {
        let mut c = Chill::new(5, 0.1);
        c.apply(2);
        // 100 * (1 - 0.1 * 2) = 80
        assert!((c.effective_speed(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_max_stacks() {
        let mut c = Chill::new(5, 0.2);
        c.apply(5);
        // 100 * (1 - 0.2 * 5) = 0
        assert!(c.effective_speed(100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_floored_at_zero() {
        let mut c = Chill::new(10, 0.2);
        c.chill_stacks = 10; // direct set; 10 * 0.2 = 2.0 reduction
        assert!(c.effective_speed(100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_base_when_disabled() {
        let mut c = Chill::new(5, 0.1);
        c.apply(5);
        c.enabled = false;
        assert!((c.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut c = Chill::new(4, 0.1);
        c.apply(2);
        assert!((c.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn max_stacks_clamped_to_one() {
        let c = Chill::new(0, 0.1);
        assert_eq!(c.max_stacks, 1);
    }

    #[test]
    fn speed_reduction_clamped_to_one() {
        let c = Chill::new(5, 2.0);
        assert!((c.speed_reduction_per_stack - 1.0).abs() < 1e-5);
    }

    #[test]
    fn dispel_after_freeze_unfreezes() {
        let mut c = Chill::new(3, 0.1);
        c.apply(3);
        assert!(c.is_frozen());
        c.dispel(1);
        assert!(!c.is_frozen());
    }

    #[test]
    fn refreeze_after_dispel_fires_just_froze_again() {
        let mut c = Chill::new(3, 0.1);
        c.apply(3); // freeze
        c.tick();
        c.dispel(1); // unfreeze
        c.apply(1); // refreeze
        assert!(c.just_froze);
    }
}
