use bevy_ecs::prelude::Component;

/// Active combat stance for a three-way trade-off between attack and defense.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum StanceKind {
    /// No modification to attack or defense.
    #[default]
    Neutral,
    /// Increases outgoing damage by `offense_bonus` fraction.
    Offensive,
    /// Reduces incoming damage by `defense_reduction` fraction.
    Defensive,
}

/// Three-way combat-stance modifier: entity cycles between `Neutral`,
/// `Offensive`, and `Defensive` stances. The ability system calls
/// `set_stance(kind)` to switch; `just_changed` fires for one frame on
/// every stance transition. `tick()` clears one-frame flags.
///
/// `effective_attack(base)` returns `base * (1 + offense_bonus)` in
/// Offensive stance and `base` in all others. `effective_defense(base)`
/// returns `base * (1 - defense_reduction)` floored at `0.0` in Defensive
/// stance and `base` in all others.
///
/// The relationship between the two stances is intentionally open — setting
/// an `offense_bonus` does NOT automatically impose a defense penalty; the
/// game designer wires those trade-offs through the ability system that
/// switches stances, keeping this component a pure value store.
///
/// Distinct from `Buff`/`Debuff` (timed stat modifications), `Guard`
/// (stance-specific block window), and `Reckless` (constant attack/defense
/// swap): Stance is a **designer-controlled three-position switch** that
/// immediately and persistently changes the effective attack/defense scalars
/// until switched again.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stance {
    pub current: StanceKind,
    /// Extra outgoing damage fraction in Offensive stance. Clamped ≥ 0.0.
    pub offense_bonus: f32,
    /// Incoming damage reduction fraction in Defensive stance. Clamped [0.0, 1.0].
    pub defense_reduction: f32,
    pub just_changed: bool,
    pub enabled: bool,
}

impl Stance {
    pub fn new(offense_bonus: f32, defense_reduction: f32) -> Self {
        Self {
            current: StanceKind::Neutral,
            offense_bonus: offense_bonus.max(0.0),
            defense_reduction: defense_reduction.clamp(0.0, 1.0),
            just_changed: false,
            enabled: true,
        }
    }

    /// Switch to `kind`. Sets `just_changed` when the stance actually differs
    /// from the current one. No-op when disabled or stance is unchanged.
    pub fn set_stance(&mut self, kind: StanceKind) {
        if !self.enabled || self.current == kind {
            return;
        }
        self.current = kind;
        self.just_changed = true;
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_changed = false;
    }

    /// Effective outgoing damage. Returns `base * (1 + offense_bonus)` in
    /// Offensive stance when enabled; returns `base` otherwise.
    pub fn effective_attack(&self, base: f32) -> f32 {
        if self.enabled && self.current == StanceKind::Offensive {
            base * (1.0 + self.offense_bonus)
        } else {
            base
        }
    }

    /// Effective incoming damage. Returns `base * (1 - defense_reduction)`
    /// floored at `0.0` in Defensive stance when enabled; returns `base`
    /// otherwise.
    pub fn effective_defense(&self, base: f32) -> f32 {
        if self.enabled && self.current == StanceKind::Defensive {
            (base * (1.0 - self.defense_reduction)).max(0.0)
        } else {
            base
        }
    }

    pub fn is_offensive(&self) -> bool {
        self.current == StanceKind::Offensive && self.enabled
    }

    pub fn is_defensive(&self) -> bool {
        self.current == StanceKind::Defensive && self.enabled
    }
}

impl Default for Stance {
    fn default() -> Self {
        Self::new(0.3, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_stance_is_neutral() {
        let s = Stance::new(0.3, 0.3);
        assert_eq!(s.current, StanceKind::Neutral);
    }

    #[test]
    fn set_stance_changes_kind_and_flags() {
        let mut s = Stance::new(0.3, 0.3);
        s.set_stance(StanceKind::Offensive);
        assert_eq!(s.current, StanceKind::Offensive);
        assert!(s.just_changed);
    }

    #[test]
    fn set_stance_no_op_when_same() {
        let mut s = Stance::new(0.3, 0.3);
        s.set_stance(StanceKind::Neutral);
        assert!(!s.just_changed);
    }

    #[test]
    fn set_stance_no_op_when_disabled() {
        let mut s = Stance::new(0.3, 0.3);
        s.enabled = false;
        s.set_stance(StanceKind::Offensive);
        assert_eq!(s.current, StanceKind::Neutral);
        assert!(!s.just_changed);
    }

    #[test]
    fn tick_clears_just_changed() {
        let mut s = Stance::new(0.3, 0.3);
        s.set_stance(StanceKind::Offensive);
        s.tick();
        assert!(!s.just_changed);
    }

    #[test]
    fn effective_attack_boosted_in_offensive() {
        let mut s = Stance::new(0.5, 0.3);
        s.set_stance(StanceKind::Offensive);
        // 100 * (1 + 0.5) = 150
        assert!((s.effective_attack(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_attack_base_in_neutral() {
        let s = Stance::new(0.5, 0.3);
        assert!((s.effective_attack(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_attack_base_in_defensive() {
        let mut s = Stance::new(0.5, 0.3);
        s.set_stance(StanceKind::Defensive);
        assert!((s.effective_attack(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_defense_reduced_in_defensive() {
        let mut s = Stance::new(0.3, 0.4);
        s.set_stance(StanceKind::Defensive);
        // 100 * (1 - 0.4) = 60
        assert!((s.effective_defense(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_defense_base_in_neutral() {
        let s = Stance::new(0.3, 0.4);
        assert!((s.effective_defense(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_defense_base_in_offensive() {
        let mut s = Stance::new(0.3, 0.4);
        s.set_stance(StanceKind::Offensive);
        assert!((s.effective_defense(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_defense_full_reduction_zero() {
        let mut s = Stance::new(0.3, 1.0);
        s.set_stance(StanceKind::Defensive);
        assert!((s.effective_defense(100.0)).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_attack_base_regardless_of_stance() {
        let mut s = Stance::new(0.5, 0.3);
        s.set_stance(StanceKind::Offensive);
        s.enabled = false;
        assert!((s.effective_attack(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_defense_base_regardless_of_stance() {
        let mut s = Stance::new(0.3, 0.4);
        s.set_stance(StanceKind::Defensive);
        s.enabled = false;
        assert!((s.effective_defense(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn is_offensive_true_only_in_offensive_and_enabled() {
        let mut s = Stance::new(0.3, 0.3);
        assert!(!s.is_offensive());
        s.set_stance(StanceKind::Offensive);
        assert!(s.is_offensive());
        s.enabled = false;
        assert!(!s.is_offensive());
    }

    #[test]
    fn is_defensive_true_only_in_defensive_and_enabled() {
        let mut s = Stance::new(0.3, 0.3);
        assert!(!s.is_defensive());
        s.set_stance(StanceKind::Defensive);
        assert!(s.is_defensive());
        s.enabled = false;
        assert!(!s.is_defensive());
    }

    #[test]
    fn cycle_through_all_stances() {
        let mut s = Stance::new(0.3, 0.4);
        s.set_stance(StanceKind::Offensive);
        assert!(s.just_changed);
        s.tick();
        s.set_stance(StanceKind::Defensive);
        assert!(s.just_changed);
        s.tick();
        s.set_stance(StanceKind::Neutral);
        assert!(s.just_changed);
    }

    #[test]
    fn offense_bonus_clamped_non_negative() {
        let s = Stance::new(-1.0, 0.3);
        assert!(s.offense_bonus >= 0.0);
    }

    #[test]
    fn defense_reduction_clamped_zero_to_one() {
        let s_low = Stance::new(0.3, -1.0);
        let s_high = Stance::new(0.3, 5.0);
        assert!(s_low.defense_reduction >= 0.0);
        assert!(s_high.defense_reduction <= 1.0);
    }
}
