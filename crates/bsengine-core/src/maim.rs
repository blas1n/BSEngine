use bevy_ecs::prelude::Component;

/// Persistent wound debuff that stacks severity and lingers until healed.
///
/// Unlike timed debuffs, Maim has no built-in duration — stacks accumulate
/// until `heal(amount)` reduces them to zero. Each stack reduces effective
/// move speed (`effective_speed`) and contributes bleed damage via `tick(dt)`.
///
/// `apply(amount)` adds stacks up to `max_stacks`. `heal(amount)` removes them.
/// `tick(dt)` returns total bleed damage for the frame (`bleed_per_stack_per_second
/// * stacks * dt`) and sets `just_healed` when stacks reach zero.
///
/// Distinct from `Bleed` (a separate timer-based DoT), `Lacerate` (wound that
/// amplifies incoming physical damage), and `Cripple` (timed speed penalty):
/// Maim is a _permanent-until-healed_ wound whose severity grows with stacks
/// and whose only natural removal is explicit healing.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Maim {
    pub stacks: u32,
    pub max_stacks: u32,
    /// Fraction of move speed lost per stack. Clamped to [0.0, 1.0].
    /// e.g. 0.1 with 3 stacks = 30% speed reduction.
    pub speed_fraction_per_stack: f32,
    /// Bleed damage dealt per stack per second.
    pub bleed_per_stack_per_second: f32,
    pub just_maimed: bool,
    pub just_healed: bool,
    pub enabled: bool,
}

impl Maim {
    pub fn new(speed_fraction_per_stack: f32, max_stacks: u32) -> Self {
        Self {
            stacks: 0,
            max_stacks: max_stacks.max(1),
            speed_fraction_per_stack: speed_fraction_per_stack.clamp(0.0, 1.0),
            bleed_per_stack_per_second: 0.0,
            just_maimed: false,
            just_healed: false,
            enabled: true,
        }
    }

    pub fn with_bleed(mut self, bleed_per_stack_per_second: f32) -> Self {
        self.bleed_per_stack_per_second = bleed_per_stack_per_second.max(0.0);
        self
    }

    /// Add `amount` stacks, capped at `max_stacks`. No-op when disabled.
    /// Sets `just_maimed` on the first stack applied to an unmaimed entity.
    pub fn apply(&mut self, amount: u32) {
        if !self.enabled || amount == 0 {
            return;
        }
        let was_active = self.is_active();
        self.stacks = (self.stacks + amount).min(self.max_stacks);
        if !was_active && self.is_active() {
            self.just_maimed = true;
        }
    }

    /// Remove `amount` stacks. Sets `just_healed` when stacks reach zero.
    pub fn heal(&mut self, amount: u32) {
        self.just_healed = false;
        if self.stacks == 0 || amount == 0 {
            return;
        }
        self.stacks = self.stacks.saturating_sub(amount);
        if self.stacks == 0 {
            self.just_healed = true;
        }
    }

    /// Advance the component one tick. Returns bleed damage for this frame
    /// (`bleed_per_stack_per_second * stacks * dt`). Sets `just_healed` if
    /// stacks drop to zero (e.g. via healing called earlier this frame).
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_maimed = false;

        if self.stacks == 0 {
            return 0.0;
        }
        self.bleed_per_stack_per_second * self.stacks as f32 * dt
    }

    pub fn is_active(&self) -> bool {
        self.stacks > 0
    }

    /// Effective move speed after applying the maim penalty.
    /// Returns `base * max(0.0, 1.0 - stacks * speed_fraction_per_stack)`.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if !self.is_active() {
            return base;
        }
        let reduction = (self.stacks as f32 * self.speed_fraction_per_stack).clamp(0.0, 1.0);
        base * (1.0 - reduction)
    }

    /// Fraction of max stacks currently applied [0.0 = none, 1.0 = full].
    pub fn stack_fraction(&self) -> f32 {
        if self.max_stacks == 0 {
            return 0.0;
        }
        (self.stacks as f32 / self.max_stacks as f32).clamp(0.0, 1.0)
    }
}

impl Default for Maim {
    fn default() -> Self {
        Self::new(0.15, 5).with_bleed(3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_adds_stacks() {
        let mut m = Maim::new(0.1, 5);
        m.apply(3);
        assert_eq!(m.stacks, 3);
        assert!(m.just_maimed);
        assert!(m.is_active());
    }

    #[test]
    fn apply_caps_at_max_stacks() {
        let mut m = Maim::new(0.1, 5);
        m.apply(10);
        assert_eq!(m.stacks, 5);
    }

    #[test]
    fn apply_no_op_when_disabled() {
        let mut m = Maim::new(0.1, 5);
        m.enabled = false;
        m.apply(3);
        assert_eq!(m.stacks, 0);
    }

    #[test]
    fn heal_removes_stacks() {
        let mut m = Maim::new(0.1, 5);
        m.apply(4);
        m.heal(2);
        assert_eq!(m.stacks, 2);
        assert!(!m.just_healed);
    }

    #[test]
    fn heal_to_zero_sets_just_healed() {
        let mut m = Maim::new(0.1, 5);
        m.apply(3);
        m.heal(3);
        assert_eq!(m.stacks, 0);
        assert!(m.just_healed);
        assert!(!m.is_active());
    }

    #[test]
    fn heal_saturates_at_zero() {
        let mut m = Maim::new(0.1, 5);
        m.apply(2);
        m.heal(10);
        assert_eq!(m.stacks, 0);
    }

    #[test]
    fn tick_returns_bleed_damage() {
        let mut m = Maim::new(0.1, 5).with_bleed(2.0);
        m.apply(3);
        let dmg = m.tick(1.0);
        assert!((dmg - 6.0).abs() < 1e-5); // 2.0 * 3 stacks * 1.0s
    }

    #[test]
    fn tick_zero_when_no_stacks() {
        let mut m = Maim::new(0.1, 5).with_bleed(5.0);
        let dmg = m.tick(1.0);
        assert!((dmg - 0.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_maimed() {
        let mut m = Maim::new(0.1, 5);
        m.apply(1);
        m.tick(0.016);
        assert!(!m.just_maimed);
    }

    #[test]
    fn effective_speed_with_two_stacks() {
        let mut m = Maim::new(0.2, 5);
        m.apply(2);
        // 1.0 - 2 * 0.2 = 0.6; base 10 * 0.6 = 6
        assert!((m.effective_speed(10.0) - 6.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_clamped_to_zero_at_max() {
        let mut m = Maim::new(0.5, 4);
        m.apply(4); // 4 * 0.5 = 2.0 → clamped to 1.0 reduction → 0 speed
        assert!((m.effective_speed(10.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_when_inactive() {
        let m = Maim::new(0.2, 5);
        assert!((m.effective_speed(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut m = Maim::new(0.1, 4);
        m.apply(2);
        assert!((m.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn just_maimed_only_on_first_stack() {
        let mut m = Maim::new(0.1, 5);
        m.apply(1);
        assert!(m.just_maimed);
        m.tick(0.016); // clears just_maimed
        m.apply(1); // already active
        assert!(!m.just_maimed);
    }
}
