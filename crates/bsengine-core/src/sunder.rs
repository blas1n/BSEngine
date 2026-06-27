use bevy_ecs::prelude::Component;

/// Permanent-until-cleansed armor-shattering debuff that accumulates in
/// shards, each removing a fixed fraction of the target's damage reduction.
///
/// Unlike `Crush` (timed flat reduction), Sunder stacks and persists until
/// explicitly repaired. While sundered, the damage pipeline should compute
/// effective damage reduction via `effective_damage_reduction(base_dr)`.
///
/// `apply(count)` adds shards up to `max_shards` and sets `just_sundered` on
/// the first stack. `repair(count)` removes shards. `tick()` clears the
/// one-frame `just_sundered` flag.
///
/// Distinct from `Crush` (timed temporary armor reduction), `Corrosion`
/// (damage-over-time armor decay), and `ShieldBreak` (destroys a shield
/// layer): Sunder permanently strips DR shards that only come back through an
/// explicit repair mechanic (item, ability, or encounter reset).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Sunder {
    /// Current number of shards embedded.
    pub shards: u32,
    pub max_shards: u32,
    /// Fraction of base damage reduction removed per shard. Clamped to [0.0, 1.0].
    /// e.g. 0.1 = 10% DR removed per shard; 5 shards → 50% total DR removed.
    pub damage_reduction_per_shard: f32,
    pub just_sundered: bool,
    pub enabled: bool,
}

impl Sunder {
    pub fn new(damage_reduction_per_shard: f32, max_shards: u32) -> Self {
        Self {
            shards: 0,
            max_shards: max_shards.max(1),
            damage_reduction_per_shard: damage_reduction_per_shard.clamp(0.0, 1.0),
            just_sundered: false,
            enabled: true,
        }
    }

    /// Add `count` shards, capped at `max_shards`. Sets `just_sundered` on
    /// the first shard applied (transition from 0 → active). No-op when disabled.
    pub fn apply(&mut self, count: u32) {
        if !self.enabled || count == 0 {
            return;
        }

        let was_active = self.is_active();
        self.shards = (self.shards + count).min(self.max_shards);
        if !was_active && self.is_active() {
            self.just_sundered = true;
        }
    }

    /// Remove `count` shards, flooring at 0.
    pub fn repair(&mut self, count: u32) {
        self.shards = self.shards.saturating_sub(count);
    }

    /// Fully remove all shards.
    pub fn repair_all(&mut self) {
        self.shards = 0;
    }

    /// Clear the one-frame `just_sundered` flag. Call every frame.
    pub fn tick(&mut self) {
        self.just_sundered = false;
    }

    pub fn is_active(&self) -> bool {
        self.shards > 0
    }

    /// Total fraction of base DR removed by all current shards. Clamped to [0.0, 1.0].
    pub fn total_reduction(&self) -> f32 {
        (self.shards as f32 * self.damage_reduction_per_shard).min(1.0)
    }

    /// Effective damage reduction after sunder. Returns
    /// `base_dr * (1 - total_reduction())` while active, `base_dr` otherwise.
    pub fn effective_damage_reduction(&self, base_dr: f32) -> f32 {
        if self.is_active() {
            (base_dr * (1.0 - self.total_reduction())).max(0.0)
        } else {
            base_dr
        }
    }

    /// Fraction of the shard cap filled [0.0 = no shards, 1.0 = max shards].
    pub fn shard_fraction(&self) -> f32 {
        if self.max_shards == 0 {
            return 0.0;
        }
        (self.shards as f32 / self.max_shards as f32).clamp(0.0, 1.0)
    }
}

impl Default for Sunder {
    fn default() -> Self {
        Self::new(0.1, 5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_sunder() {
        let mut s = Sunder::new(0.1, 5);
        s.apply(1);
        assert!(s.is_active());
        assert!(s.just_sundered);
    }

    #[test]
    fn apply_accumulates_shards() {
        let mut s = Sunder::new(0.1, 5);
        s.apply(2);
        s.tick();
        s.apply(2);
        assert_eq!(s.shards, 4);
    }

    #[test]
    fn apply_caps_at_max_shards() {
        let mut s = Sunder::new(0.1, 5);
        s.apply(10);
        assert_eq!(s.shards, 5);
    }

    #[test]
    fn just_sundered_only_on_first_shard() {
        let mut s = Sunder::new(0.1, 5);
        s.apply(1);
        s.tick();
        s.apply(1);
        assert!(!s.just_sundered);
    }

    #[test]
    fn repair_removes_shards() {
        let mut s = Sunder::new(0.1, 5);
        s.apply(4);
        s.repair(2);
        assert_eq!(s.shards, 2);
    }

    #[test]
    fn repair_floors_at_zero() {
        let mut s = Sunder::new(0.1, 5);
        s.apply(3);
        s.repair(10);
        assert_eq!(s.shards, 0);
        assert!(!s.is_active());
    }

    #[test]
    fn repair_all_clears_shards() {
        let mut s = Sunder::new(0.1, 5);
        s.apply(5);
        s.repair_all();
        assert_eq!(s.shards, 0);
    }

    #[test]
    fn total_reduction_sums_shards() {
        let mut s = Sunder::new(0.1, 5);
        s.apply(3);
        assert!((s.total_reduction() - 0.3).abs() < 1e-5);
    }

    #[test]
    fn total_reduction_capped_at_one() {
        let mut s = Sunder::new(0.3, 5);
        s.apply(5);
        assert!((s.total_reduction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_dr_while_active() {
        let mut s = Sunder::new(0.1, 5);
        s.apply(3);
        assert!((s.effective_damage_reduction(0.5) - 0.35).abs() < 1e-5);
    }

    #[test]
    fn effective_dr_when_inactive() {
        let s = Sunder::new(0.1, 5);
        assert!((s.effective_damage_reduction(0.5) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn effective_dr_cannot_go_below_zero() {
        let mut s = Sunder::new(0.5, 5);
        s.apply(5);
        assert!((s.effective_damage_reduction(0.3) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn shard_fraction_at_half() {
        let mut s = Sunder::new(0.1, 4);
        s.apply(2);
        assert!((s.shard_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut s = Sunder::new(0.1, 5);
        s.enabled = false;
        s.apply(3);
        assert!(!s.is_active());
    }

    #[test]
    fn tick_clears_just_sundered() {
        let mut s = Sunder::new(0.1, 5);
        s.apply(1);
        s.tick();
        assert!(!s.just_sundered);
    }
}
