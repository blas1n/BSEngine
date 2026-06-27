use bevy_ecs::prelude::Component;

/// Lone-wolf power surge: grants a bonus to outgoing damage and incoming
/// defense when the entity is operating without nearby allies. The proximity
/// system calls `set_alone(true)` when no allies are within its detection
/// radius, and `set_alone(false)` when at least one ally is nearby. This
/// component only tracks the state and provides the multiplier methods.
///
/// `set_alone(alone)` fires `just_became_alone` on the grouped → alone
/// transition and `just_joined_group` on the alone → grouped transition.
/// `tick()` clears one-frame flags at the start of each call.
///
/// `effective_damage(base)` returns `base * (1 + damage_bonus)` and
/// `effective_defense(base)` returns `base * (1 + defense_bonus)` when
/// `is_alone` and `enabled`; both return `base` otherwise.
///
/// Distinct from `Surge` (unconditional timed burst), `Rampage` (kill-streak
/// escalation), and `Stealth` (visibility reduction): Recluse is a **lone-wolf
/// posture buff** — its bonuses are continuous while the entity stays isolated,
/// not a one-shot or decaying resource.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Recluse {
    pub is_alone: bool,
    /// Outgoing damage bonus fraction when alone. Clamped ≥ 0.0.
    pub damage_bonus: f32,
    /// Incoming defense bonus fraction when alone. Clamped ≥ 0.0.
    pub defense_bonus: f32,
    pub just_became_alone: bool,
    pub just_joined_group: bool,
    pub enabled: bool,
}

impl Recluse {
    pub fn new(damage_bonus: f32, defense_bonus: f32) -> Self {
        Self {
            is_alone: false,
            damage_bonus: damage_bonus.max(0.0),
            defense_bonus: defense_bonus.max(0.0),
            just_became_alone: false,
            just_joined_group: false,
            enabled: true,
        }
    }

    /// Update the entity's social state. Fires `just_became_alone` on the
    /// grouped → alone transition and `just_joined_group` on the reverse.
    /// No-op when the state hasn't changed or the component is disabled.
    pub fn set_alone(&mut self, alone: bool) {
        if !self.enabled || alone == self.is_alone {
            return;
        }
        if alone {
            self.just_became_alone = true;
        } else {
            self.just_joined_group = true;
        }
        self.is_alone = alone;
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_became_alone = false;
        self.just_joined_group = false;
    }

    /// Effective outgoing damage when alone and enabled.
    /// Returns `base * (1 + damage_bonus)`; returns `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_alone && self.enabled {
            base * (1.0 + self.damage_bonus)
        } else {
            base
        }
    }

    /// Effective incoming defense when alone and enabled.
    /// Returns `base * (1 + defense_bonus)`; returns `base` otherwise.
    pub fn effective_defense(&self, base: f32) -> f32 {
        if self.is_alone && self.enabled {
            base * (1.0 + self.defense_bonus)
        } else {
            base
        }
    }
}

impl Default for Recluse {
    fn default() -> Self {
        Self::new(0.25, 0.15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_alone_transitions_to_alone() {
        let mut r = Recluse::new(0.25, 0.15);
        r.set_alone(true);
        assert!(r.is_alone);
        assert!(r.just_became_alone);
        assert!(!r.just_joined_group);
    }

    #[test]
    fn set_alone_transitions_back_to_group() {
        let mut r = Recluse::new(0.25, 0.15);
        r.set_alone(true);
        r.tick();
        r.set_alone(false);
        assert!(!r.is_alone);
        assert!(r.just_joined_group);
        assert!(!r.just_became_alone);
    }

    #[test]
    fn set_alone_no_op_when_same_state() {
        let mut r = Recluse::new(0.25, 0.15);
        r.set_alone(false); // already false
        assert!(!r.just_joined_group);
        assert!(!r.just_became_alone);
    }

    #[test]
    fn set_alone_no_op_when_already_alone() {
        let mut r = Recluse::new(0.25, 0.15);
        r.set_alone(true);
        r.tick();
        r.set_alone(true); // already alone
        assert!(!r.just_became_alone);
    }

    #[test]
    fn tick_clears_just_became_alone() {
        let mut r = Recluse::new(0.25, 0.15);
        r.set_alone(true);
        r.tick();
        assert!(!r.just_became_alone);
    }

    #[test]
    fn tick_clears_just_joined_group() {
        let mut r = Recluse::new(0.25, 0.15);
        r.set_alone(true);
        r.tick();
        r.set_alone(false);
        r.tick();
        assert!(!r.just_joined_group);
    }

    #[test]
    fn effective_damage_bonus_when_alone() {
        let mut r = Recluse::new(0.25, 0.0);
        r.set_alone(true);
        // 100 * 1.25 = 125
        assert!((r.effective_damage(100.0) - 125.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_grouped() {
        let r = Recluse::new(0.25, 0.0);
        assert!((r.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_defense_bonus_when_alone() {
        let mut r = Recluse::new(0.0, 0.15);
        r.set_alone(true);
        // 100 * 1.15 = 115
        assert!((r.effective_defense(100.0) - 115.0).abs() < 1e-3);
    }

    #[test]
    fn effective_defense_base_when_grouped() {
        let r = Recluse::new(0.0, 0.15);
        assert!((r.effective_defense(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_set_alone_no_op() {
        let mut r = Recluse::new(0.25, 0.15);
        r.enabled = false;
        r.set_alone(true);
        assert!(!r.is_alone);
        assert!(!r.just_became_alone);
    }

    #[test]
    fn disabled_effective_damage_base() {
        let mut r = Recluse::new(0.25, 0.0);
        r.set_alone(true);
        r.enabled = false;
        assert!((r.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_defense_base() {
        let mut r = Recluse::new(0.0, 0.15);
        r.set_alone(true);
        r.enabled = false;
        assert!((r.effective_defense(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn bonuses_apply_together() {
        let mut r = Recluse::new(0.3, 0.2);
        r.set_alone(true);
        // damage: 100 * 1.3 = 130, defense: 100 * 1.2 = 120
        assert!((r.effective_damage(100.0) - 130.0).abs() < 1e-3);
        assert!((r.effective_defense(100.0) - 120.0).abs() < 1e-3);
    }
}
