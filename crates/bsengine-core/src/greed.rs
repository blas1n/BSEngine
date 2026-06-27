use bevy_ecs::prelude::Component;

/// Consecutive-kill loot/resource multiplier that resets on damage. Each kill
/// while undamaged increments `kill_streak`, scaling `drop_multiplier()` up to
/// `max_multiplier`. Taking any hit breaks the streak and fires `just_reset`.
///
/// `kill()` increments `kill_streak` by 1. No-op when disabled.
///
/// `damage_taken()` resets `kill_streak` to 0 and fires `just_reset` if the
/// streak was > 0. No-op when disabled.
///
/// `tick()` clears `just_reset` each frame.
///
/// `drop_multiplier()` returns `(1.0 + loot_bonus * kill_streak).min(max_multiplier)`
/// when enabled; returns `1.0` when disabled (no bonus, but no penalty either).
///
/// Distinct from `Rampage` (kill-based movement/attack speed stack),
/// `Fervor` (on-kill outgoing-damage stack), and `LootTable` (static weighted
/// drop table unrelated to risk): Greed is a **risk-and-reward loot scaler**
/// — the entity earns better drops by maintaining a flawless kill streak,
/// but any incoming hit zeroes the multiplier, punishing greedy play.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Greed {
    /// Number of consecutive kills without taking damage.
    pub kill_streak: u32,
    /// Loot/resource bonus added per kill in the streak. Clamped ≥ 0.0.
    /// e.g. `0.1` gives +10 % per kill (streak 5 → +50 %).
    pub loot_bonus: f32,
    /// Maximum achievable drop multiplier. Clamped ≥ 1.0.
    pub max_multiplier: f32,
    pub just_reset: bool,
    pub enabled: bool,
}

impl Greed {
    pub fn new(loot_bonus: f32, max_multiplier: f32) -> Self {
        Self {
            kill_streak: 0,
            loot_bonus: loot_bonus.max(0.0),
            max_multiplier: max_multiplier.max(1.0),
            just_reset: false,
            enabled: true,
        }
    }

    /// Register a kill: increment `kill_streak` by 1. No-op when disabled.
    pub fn kill(&mut self) {
        if !self.enabled {
            return;
        }
        self.kill_streak += 1;
    }

    /// Register incoming damage: reset `kill_streak` to 0 and set `just_reset`
    /// if there was an active streak. No-op when disabled.
    pub fn damage_taken(&mut self) {
        if !self.enabled {
            return;
        }
        if self.kill_streak > 0 {
            self.kill_streak = 0;
            self.just_reset = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_reset = false;
    }

    /// `true` when there is an active streak and the component is enabled.
    pub fn is_greedy(&self) -> bool {
        self.kill_streak > 0 && self.enabled
    }

    /// Loot/resource drop multiplier for the current streak.
    /// Returns `(1.0 + loot_bonus * kill_streak).min(max_multiplier)` when
    /// enabled; returns `1.0` when disabled (no bonus).
    pub fn drop_multiplier(&self) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        (1.0 + self.loot_bonus * self.kill_streak as f32).min(self.max_multiplier)
    }
}

impl Default for Greed {
    fn default() -> Self {
        Self::new(0.1, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero() {
        let g = Greed::new(0.1, 3.0);
        assert_eq!(g.kill_streak, 0);
        assert!(!g.is_greedy());
    }

    #[test]
    fn kill_increments_streak() {
        let mut g = Greed::new(0.1, 3.0);
        g.kill();
        g.kill();
        assert_eq!(g.kill_streak, 2);
    }

    #[test]
    fn kill_no_op_when_disabled() {
        let mut g = Greed::new(0.1, 3.0);
        g.enabled = false;
        g.kill();
        assert_eq!(g.kill_streak, 0);
    }

    #[test]
    fn damage_taken_resets_streak() {
        let mut g = Greed::new(0.1, 3.0);
        g.kill();
        g.kill();
        g.damage_taken();
        assert_eq!(g.kill_streak, 0);
    }

    #[test]
    fn damage_taken_fires_just_reset() {
        let mut g = Greed::new(0.1, 3.0);
        g.kill();
        g.damage_taken();
        assert!(g.just_reset);
    }

    #[test]
    fn damage_taken_no_just_reset_when_streak_zero() {
        let mut g = Greed::new(0.1, 3.0);
        g.damage_taken();
        assert!(!g.just_reset);
    }

    #[test]
    fn damage_taken_no_op_when_disabled() {
        let mut g = Greed::new(0.1, 3.0);
        g.kill();
        g.enabled = false;
        g.damage_taken();
        assert_eq!(g.kill_streak, 1);
    }

    #[test]
    fn tick_clears_just_reset() {
        let mut g = Greed::new(0.1, 3.0);
        g.kill();
        g.damage_taken();
        g.tick();
        assert!(!g.just_reset);
    }

    #[test]
    fn is_greedy_true_when_streak_active() {
        let mut g = Greed::new(0.1, 3.0);
        g.kill();
        assert!(g.is_greedy());
    }

    #[test]
    fn is_greedy_false_when_disabled() {
        let mut g = Greed::new(0.1, 3.0);
        g.kill();
        g.enabled = false;
        assert!(!g.is_greedy());
    }

    #[test]
    fn drop_multiplier_one_at_zero_streak() {
        let g = Greed::new(0.1, 3.0);
        assert!((g.drop_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn drop_multiplier_scales_with_streak() {
        let mut g = Greed::new(0.1, 5.0);
        g.kill(); // 1 * 0.1 = 0.1 → 1.1
        assert!((g.drop_multiplier() - 1.1).abs() < 1e-5);
        g.kill(); // 2 * 0.1 = 0.2 → 1.2
        assert!((g.drop_multiplier() - 1.2).abs() < 1e-5);
        g.kill(); // 3 * 0.1 = 0.3 → 1.3
        assert!((g.drop_multiplier() - 1.3).abs() < 1e-5);
    }

    #[test]
    fn drop_multiplier_caps_at_max() {
        let mut g = Greed::new(1.0, 2.0);
        for _ in 0..10 {
            g.kill(); // would be 11 without cap
        }
        assert!((g.drop_multiplier() - 2.0).abs() < 1e-5);
    }

    #[test]
    fn drop_multiplier_one_when_disabled() {
        let mut g = Greed::new(0.1, 3.0);
        g.kill();
        g.kill();
        g.enabled = false;
        assert!((g.drop_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn streak_rebuilds_after_reset() {
        let mut g = Greed::new(0.1, 5.0);
        g.kill();
        g.kill();
        g.damage_taken(); // streak = 0
        g.tick();
        g.kill();
        g.kill();
        g.kill(); // streak = 3
        assert_eq!(g.kill_streak, 3);
        assert!((g.drop_multiplier() - 1.3).abs() < 1e-5);
    }

    #[test]
    fn loot_bonus_clamped_non_negative() {
        let g = Greed::new(-0.5, 3.0);
        assert_eq!(g.loot_bonus, 0.0);
    }

    #[test]
    fn max_multiplier_clamped_to_one() {
        let g = Greed::new(0.1, 0.5);
        assert!((g.max_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn zero_loot_bonus_always_one() {
        let mut g = Greed::new(0.0, 3.0);
        for _ in 0..100 {
            g.kill();
        }
        assert!((g.drop_multiplier() - 1.0).abs() < 1e-5);
    }
}
