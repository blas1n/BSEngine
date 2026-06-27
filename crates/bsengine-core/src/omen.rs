use bevy_ecs::prelude::Component;

/// Stacking curse amplifier: each `add_stack()` call marks the entity with
/// another layer of ill omen. When the effect detonates (via `consume()`),
/// all stacks are spent and the returned multiplier is applied to the
/// triggering damage or debuff.
///
/// `total_multiplier()` returns `1.0 + stacks * damage_multiplier_per_stack`.
/// `consume()` returns that value and clears all stacks, setting `just_consumed`.
/// `add_stack()` adds one stack (capped at `max_stacks`), setting `just_stacked`.
/// `tick()` clears one-frame flags.
///
/// Distinct from `Curse` (a timed debuff that decays passively), `Malice`
/// (persistent damage-over-time), and `Hex` (single-use denial effect): Omen
/// is a **charge-accumulating curse amplifier** — stacks build up silently and
/// then detonate at the moment chosen by the damage system, multiplying the
/// impact of the next hit or debuff application.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Omen {
    pub stacks: u32,
    pub max_stacks: u32,
    /// Damage multiplier increase per stack. Clamped ≥ 0.0.
    pub damage_multiplier_per_stack: f32,
    pub just_stacked: bool,
    pub just_consumed: bool,
    pub enabled: bool,
}

impl Omen {
    pub fn new(max_stacks: u32, damage_multiplier_per_stack: f32) -> Self {
        Self {
            stacks: 0,
            max_stacks: max_stacks.max(1),
            damage_multiplier_per_stack: damage_multiplier_per_stack.max(0.0),
            just_stacked: false,
            just_consumed: false,
            enabled: true,
        }
    }

    /// Add one omen stack (capped at `max_stacks`). Sets `just_stacked` when
    /// a stack is actually added. No-op when disabled or already at max stacks.
    pub fn add_stack(&mut self) {
        if !self.enabled || self.stacks >= self.max_stacks {
            return;
        }
        self.stacks += 1;
        self.just_stacked = true;
    }

    /// Detonate all stacks: returns the total multiplier and clears stacks.
    /// Sets `just_consumed`. Returns `1.0` (no amplification) when there are
    /// no stacks or the component is disabled.
    pub fn consume(&mut self) -> f32 {
        if !self.enabled || self.stacks == 0 {
            return 1.0;
        }
        let multiplier = self.total_multiplier();
        self.stacks = 0;
        self.just_consumed = true;
        multiplier
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_stacked = false;
        self.just_consumed = false;
    }

    pub fn is_ominous(&self) -> bool {
        self.stacks > 0
    }

    /// Total damage multiplier for the current stack count.
    /// Returns `1.0 + stacks * damage_multiplier_per_stack`.
    pub fn total_multiplier(&self) -> f32 {
        1.0 + self.stacks as f32 * self.damage_multiplier_per_stack
    }

    /// Fraction of max stacks currently loaded [0.0 = empty, 1.0 = full].
    pub fn stack_fraction(&self) -> f32 {
        if self.max_stacks == 0 {
            return 0.0;
        }
        (self.stacks as f32 / self.max_stacks as f32).clamp(0.0, 1.0)
    }
}

impl Default for Omen {
    fn default() -> Self {
        Self::new(5, 0.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_stack_increments() {
        let mut o = Omen::new(5, 0.2);
        o.add_stack();
        assert_eq!(o.stacks, 1);
        assert!(o.just_stacked);
        assert!(o.is_ominous());
    }

    #[test]
    fn add_stack_caps_at_max() {
        let mut o = Omen::new(3, 0.2);
        o.add_stack();
        o.add_stack();
        o.add_stack();
        o.add_stack(); // over cap
        assert_eq!(o.stacks, 3);
    }

    #[test]
    fn add_stack_no_just_stacked_at_cap() {
        let mut o = Omen::new(1, 0.2);
        o.add_stack(); // reaches cap
        o.tick();
        o.add_stack(); // at cap — no-op
        assert!(!o.just_stacked);
    }

    #[test]
    fn total_multiplier_scales_with_stacks() {
        let mut o = Omen::new(5, 0.2);
        o.add_stack();
        o.add_stack();
        o.add_stack();
        // 1.0 + 3 * 0.2 = 1.6
        assert!((o.total_multiplier() - 1.6).abs() < 1e-5);
    }

    #[test]
    fn total_multiplier_one_when_empty() {
        let o = Omen::new(5, 0.2);
        assert!((o.total_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn consume_returns_multiplier_and_clears_stacks() {
        let mut o = Omen::new(5, 0.25);
        o.add_stack();
        o.add_stack();
        // 1.0 + 2 * 0.25 = 1.5
        let m = o.consume();
        assert!((m - 1.5).abs() < 1e-5);
        assert_eq!(o.stacks, 0);
        assert!(o.just_consumed);
        assert!(!o.is_ominous());
    }

    #[test]
    fn consume_returns_one_when_empty() {
        let mut o = Omen::new(5, 0.2);
        let m = o.consume();
        assert!((m - 1.0).abs() < 1e-5);
        assert!(!o.just_consumed);
    }

    #[test]
    fn tick_clears_just_stacked() {
        let mut o = Omen::new(5, 0.2);
        o.add_stack();
        o.tick();
        assert!(!o.just_stacked);
    }

    #[test]
    fn tick_clears_just_consumed() {
        let mut o = Omen::new(5, 0.2);
        o.add_stack();
        o.consume();
        o.tick();
        assert!(!o.just_consumed);
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut o = Omen::new(4, 0.2);
        o.add_stack();
        o.add_stack();
        assert!((o.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_zero_when_empty() {
        let o = Omen::new(5, 0.2);
        assert!((o.stack_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_add_stack_no_op() {
        let mut o = Omen::new(5, 0.2);
        o.enabled = false;
        o.add_stack();
        assert_eq!(o.stacks, 0);
        assert!(!o.just_stacked);
    }

    #[test]
    fn disabled_consume_returns_one() {
        let mut o = Omen::new(5, 0.2);
        o.add_stack();
        o.enabled = false;
        let m = o.consume();
        assert!((m - 1.0).abs() < 1e-5);
        assert_eq!(o.stacks, 1); // stacks not consumed
        assert!(!o.just_consumed);
    }

    #[test]
    fn multiple_stacks_accumulate_additive() {
        let mut o = Omen::new(5, 0.1);
        for _ in 0..5 {
            o.add_stack();
        }
        // 1.0 + 5 * 0.1 = 1.5
        assert!((o.total_multiplier() - 1.5).abs() < 1e-5);
        let m = o.consume();
        assert!((m - 1.5).abs() < 1e-5);
        assert_eq!(o.stacks, 0);
    }
}
