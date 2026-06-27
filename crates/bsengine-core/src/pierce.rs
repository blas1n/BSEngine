use bevy_ecs::prelude::Component;

/// Attack-penetration trait: outgoing attacks can pass through multiple targets.
///
/// At the start of each attack call `begin_attack()` to reset per-attack state.
/// After hitting the first target, call `try_pierce(rng_value)` for each
/// subsequent target in the hit sequence. If it returns `true` the attack
/// continues to that target and `pierced_this_attack` increments. Once
/// `is_exhausted()` returns `true` the attack stops piercing.
///
/// `pierce_chance == 1.0` guarantees a pierce on every eligible target up to
/// `max_pierce`. Distinct from `Ricochet` (bounces) and `Reflect` (returns
/// toward source): Pierce travels straight through.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Pierce {
    /// Maximum number of additional targets a single attack can pierce through.
    pub max_pierce: u32,
    /// Probability [0.0, 1.0] that each additional pierce attempt succeeds.
    pub pierce_chance: f32,
    /// How many targets have been pierced in the current attack sequence.
    pub pierced_this_attack: u32,
    /// True on the frame/attack where a pierce actually occurred.
    pub just_pierced: bool,
    pub enabled: bool,
}

impl Pierce {
    pub fn new(max_pierce: u32) -> Self {
        Self {
            max_pierce,
            pierce_chance: 1.0,
            pierced_this_attack: 0,
            just_pierced: false,
            enabled: true,
        }
    }

    pub fn with_chance(mut self, chance: f32) -> Self {
        self.pierce_chance = chance.clamp(0.0, 1.0);
        self
    }

    /// Reset per-attack state. Call at the start of every new attack.
    pub fn begin_attack(&mut self) {
        self.pierced_this_attack = 0;
        self.just_pierced = false;
    }

    /// Attempt to pierce the next target. `rng_value` must be a pre-rolled
    /// float in [0.0, 1.0). Returns `true` when the attack pierces through.
    pub fn try_pierce(&mut self, rng_value: f32) -> bool {
        if !self.enabled || self.is_exhausted() {
            return false;
        }

        if rng_value < self.pierce_chance {
            self.pierced_this_attack += 1;
            self.just_pierced = true;
            true
        } else {
            false
        }
    }

    /// True when this attack has used all available pierces.
    pub fn is_exhausted(&self) -> bool {
        self.pierced_this_attack >= self.max_pierce
    }

    /// Remaining pierces available for the current attack sequence.
    pub fn remaining_pierces(&self) -> u32 {
        self.max_pierce.saturating_sub(self.pierced_this_attack)
    }
}

impl Default for Pierce {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_pierce_succeeds_at_full_chance() {
        let mut p = Pierce::new(3);
        assert!(p.try_pierce(0.99));
        assert!(p.just_pierced);
        assert_eq!(p.pierced_this_attack, 1);
    }

    #[test]
    fn try_pierce_fails_below_chance() {
        let mut p = Pierce::new(3).with_chance(0.5);
        assert!(!p.try_pierce(0.6)); // 0.6 >= 0.5 → fail
    }

    #[test]
    fn try_pierce_succeeds_above_chance() {
        let mut p = Pierce::new(3).with_chance(0.5);
        assert!(p.try_pierce(0.4)); // 0.4 < 0.5 → success
    }

    #[test]
    fn exhausted_after_max_pierces() {
        let mut p = Pierce::new(2);
        p.try_pierce(0.0);
        p.try_pierce(0.0);
        assert!(p.is_exhausted());
        assert!(!p.try_pierce(0.0)); // blocked when exhausted
    }

    #[test]
    fn begin_attack_resets_state() {
        let mut p = Pierce::new(2);
        p.try_pierce(0.0);
        p.try_pierce(0.0);
        assert!(p.is_exhausted());
        p.begin_attack();
        assert!(!p.is_exhausted());
        assert_eq!(p.pierced_this_attack, 0);
        assert!(!p.just_pierced);
    }

    #[test]
    fn remaining_pierces_decrements() {
        let mut p = Pierce::new(3);
        assert_eq!(p.remaining_pierces(), 3);
        p.try_pierce(0.0);
        assert_eq!(p.remaining_pierces(), 2);
    }

    #[test]
    fn zero_chance_never_pierces() {
        let mut p = Pierce::new(5).with_chance(0.0);
        assert!(!p.try_pierce(0.0));
    }

    #[test]
    fn disabled_try_pierce_false() {
        let mut p = Pierce::new(5);
        p.enabled = false;
        assert!(!p.try_pierce(0.0));
    }

    #[test]
    fn zero_max_pierce_always_exhausted() {
        let mut p = Pierce::new(0);
        assert!(p.is_exhausted());
        assert!(!p.try_pierce(0.0));
    }
}
