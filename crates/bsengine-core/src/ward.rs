use bevy_ecs::prelude::Component;

/// Rechargeable absorb field with a dedicated HP pool. When the entity takes
/// damage, the damage system calls `absorb(damage)`, which drains `ward_hp`
/// first and returns the leftover damage that spills through to the entity's
/// main health pool. When `ward_hp` reaches 0, the ward breaks and sets
/// `just_broke`. `reinforce(amount)` tops the ward back up (capped at
/// `max_ward_hp`), setting `just_reinforced` on the 0 → positive transition.
/// `tick()` clears one-frame flags.
///
/// Distinct from `Barrier` (always-active single HP buffer that can't be
/// selectively reinforced), `Overshield` (bonus HP above the health ceiling),
/// and `Shield` (directional block that negates all damage from one angle):
/// Ward is a **rechargeable absorb field** — it has its own HP pool that
/// depletes proportionally to incoming damage, can be restored by a spell or
/// ability, and breaks decisively when empty.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ward {
    pub ward_hp: f32,
    pub max_ward_hp: f32,
    pub just_broke: bool,
    pub just_reinforced: bool,
    pub enabled: bool,
}

impl Ward {
    /// Create a ward with the given maximum capacity. Starts empty (broken)
    /// until `reinforce()` is called.
    pub fn new(max_ward_hp: f32) -> Self {
        Self {
            ward_hp: 0.0,
            max_ward_hp: max_ward_hp.max(0.0),
            just_broke: false,
            just_reinforced: false,
            enabled: true,
        }
    }

    /// Add `amount` to `ward_hp` (capped at `max_ward_hp`). Sets
    /// `just_reinforced` on the 0 → positive transition (ward was broken and
    /// is now restored). No-op when disabled or `amount ≤ 0`.
    pub fn reinforce(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_broken = self.ward_hp <= 0.0;
        self.ward_hp = (self.ward_hp + amount).min(self.max_ward_hp);
        if was_broken && self.ward_hp > 0.0 {
            self.just_reinforced = true;
        }
    }

    /// Called by the damage system when the entity takes `damage`. The ward
    /// absorbs as much as it can; sets `just_broke` if the ward HP hits 0.
    /// Returns the remaining (spillover) damage that should hit the entity's
    /// main health pool. Returns `damage` unchanged when the ward is inactive
    /// or disabled.
    pub fn absorb(&mut self, damage: f32) -> f32 {
        if !self.enabled || self.ward_hp <= 0.0 || damage <= 0.0 {
            return damage;
        }
        if damage >= self.ward_hp {
            let spillover = damage - self.ward_hp;
            self.ward_hp = 0.0;
            self.just_broke = true;
            spillover
        } else {
            self.ward_hp -= damage;
            0.0
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_broke = false;
        self.just_reinforced = false;
    }

    pub fn is_active(&self) -> bool {
        self.ward_hp > 0.0
    }

    /// Fraction of max ward HP remaining [0.0 = broken, 1.0 = full].
    pub fn fraction(&self) -> f32 {
        if self.max_ward_hp <= 0.0 {
            return 0.0;
        }
        (self.ward_hp / self.max_ward_hp).clamp(0.0, 1.0)
    }
}

impl Default for Ward {
    fn default() -> Self {
        Self::new(50.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reinforce_fills_ward() {
        let mut w = Ward::new(100.0);
        w.reinforce(60.0);
        assert!((w.ward_hp - 60.0).abs() < 1e-5);
        assert!(w.is_active());
        assert!(w.just_reinforced);
    }

    #[test]
    fn reinforce_clamps_at_max() {
        let mut w = Ward::new(100.0);
        w.reinforce(80.0);
        w.reinforce(50.0); // would overflow
        assert!((w.ward_hp - 100.0).abs() < 1e-5);
    }

    #[test]
    fn reinforce_no_just_reinforced_when_already_active() {
        let mut w = Ward::new(100.0);
        w.reinforce(50.0);
        w.tick();
        w.reinforce(20.0);
        assert!(!w.just_reinforced);
    }

    #[test]
    fn absorb_partial_damage() {
        let mut w = Ward::new(100.0);
        w.reinforce(100.0);
        let spillover = w.absorb(30.0);
        assert!((spillover).abs() < 1e-5);
        assert!((w.ward_hp - 70.0).abs() < 1e-5);
        assert!(!w.just_broke);
    }

    #[test]
    fn absorb_breaks_ward_with_spillover() {
        let mut w = Ward::new(100.0);
        w.reinforce(40.0);
        let spillover = w.absorb(60.0);
        assert!((spillover - 20.0).abs() < 1e-5);
        assert_eq!(w.ward_hp, 0.0);
        assert!(w.just_broke);
        assert!(!w.is_active());
    }

    #[test]
    fn absorb_exact_damage_breaks_ward_no_spillover() {
        let mut w = Ward::new(100.0);
        w.reinforce(50.0);
        let spillover = w.absorb(50.0);
        assert!((spillover).abs() < 1e-5);
        assert!(w.just_broke);
    }

    #[test]
    fn absorb_returns_full_damage_when_broken() {
        let mut w = Ward::new(100.0);
        let spillover = w.absorb(40.0);
        assert!((spillover - 40.0).abs() < 1e-5);
        assert!(!w.just_broke);
    }

    #[test]
    fn tick_clears_just_broke() {
        let mut w = Ward::new(100.0);
        w.reinforce(20.0);
        w.absorb(30.0);
        w.tick();
        assert!(!w.just_broke);
    }

    #[test]
    fn tick_clears_just_reinforced() {
        let mut w = Ward::new(100.0);
        w.reinforce(50.0);
        w.tick();
        assert!(!w.just_reinforced);
    }

    #[test]
    fn reinforce_after_break_sets_just_reinforced() {
        let mut w = Ward::new(100.0);
        w.reinforce(20.0);
        w.absorb(30.0); // breaks it
        w.tick();
        w.reinforce(50.0);
        assert!(w.just_reinforced);
    }

    #[test]
    fn fraction_at_half() {
        let mut w = Ward::new(100.0);
        w.reinforce(50.0);
        assert!((w.fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn fraction_zero_when_broken() {
        let w = Ward::new(100.0);
        assert!((w.fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_reinforce_no_op() {
        let mut w = Ward::new(100.0);
        w.enabled = false;
        w.reinforce(50.0);
        assert!(!w.is_active());
    }

    #[test]
    fn disabled_absorb_returns_full_damage() {
        let mut w = Ward::new(100.0);
        w.reinforce(100.0);
        w.enabled = false;
        let spillover = w.absorb(40.0);
        assert!((spillover - 40.0).abs() < 1e-5);
        assert!((w.ward_hp - 100.0).abs() < 1e-5); // ward_hp unchanged
    }
}
