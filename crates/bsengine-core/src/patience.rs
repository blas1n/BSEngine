use bevy_ecs::prelude::Component;

/// Idle-buildup offensive multiplier that rewards waiting before striking.
/// While the entity does not attack, `patience_level` climbs toward
/// `max_patience`. Once full, `is_patient()` is true and
/// `effective_outgoing()` applies a scaled bonus. The moment the entity
/// attacks via `on_attack()`, all patience resets to zero.
///
/// `tick(dt)` clears one-frame flags first; increments `patience_level`
/// (capped at `max_patience`); fires `just_primed` on the first tick that
/// reaches the cap. No-op when disabled.
///
/// `on_attack()` resets `patience_level` to 0.0 and fires `just_spent` if
/// the entity was patient at the time. No-op when disabled.
///
/// `is_patient()` returns `patience_level >= max_patience && enabled`.
///
/// `patience_fraction()` returns `(patience_level / max_patience).clamp(0, 1)`.
///
/// `effective_outgoing(base)` returns
/// `base * (1.0 + patience_bonus * patience_fraction())` when enabled —
/// at full patience the entity deals up to `1 + patience_bonus` × base.
/// Returns `base` when disabled.
///
/// Distinct from `Combo` (hit-count bonuses — builds FROM attacking),
/// `Edge` (uninterrupted-offense streak — builds FROM attacking without
/// being hit), `Rampage` (kill-count momentum burst), and `Galvanize`
/// (damage-received charge meter): Patience is a **pre-attack windup
/// multiplier** — the entity grows more dangerous the longer it holds
/// back; the first strike spends all accumulated patience instantly.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Patience {
    /// Accumulated idle time [0.0, max_patience].
    pub patience_level: f32,
    /// Idle seconds required to reach the full bonus. Clamped ≥ 1.0.
    pub max_patience: f32,
    /// Outgoing damage multiplier bonus at full patience. Clamped ≥ 0.0.
    /// At full patience, effective = base * (1 + patience_bonus).
    pub patience_bonus: f32,
    pub just_primed: bool,
    pub just_spent: bool,
    pub enabled: bool,
}

impl Patience {
    pub fn new(max_patience: f32, patience_bonus: f32) -> Self {
        Self {
            patience_level: 0.0,
            max_patience: max_patience.max(1.0),
            patience_bonus: patience_bonus.max(0.0),
            just_primed: false,
            just_spent: false,
            enabled: true,
        }
    }

    /// Advance idle timer. Clears `just_primed` and `just_spent` first;
    /// increments `patience_level` (capped at `max_patience`); fires
    /// `just_primed` on the first tick that reaches the cap. No-op when
    /// disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_primed = false;
        self.just_spent = false;

        if !self.enabled {
            return;
        }

        let was_below = self.patience_level < self.max_patience;
        self.patience_level = (self.patience_level + dt).min(self.max_patience);
        if was_below && self.patience_level >= self.max_patience {
            self.just_primed = true;
        }
    }

    /// Register an attack. Resets `patience_level` to 0.0 and fires
    /// `just_spent` when patience was at max. No-op when disabled.
    pub fn on_attack(&mut self) {
        if !self.enabled {
            return;
        }
        if self.is_patient() {
            self.just_spent = true;
        }
        self.patience_level = 0.0;
    }

    /// `true` when the patience timer has reached `max_patience` and the
    /// component is enabled.
    pub fn is_patient(&self) -> bool {
        self.patience_level >= self.max_patience && self.enabled
    }

    /// Patience fill fraction [0.0 = just attacked, 1.0 = fully primed].
    /// Always in [0, 1].
    pub fn patience_fraction(&self) -> f32 {
        (self.patience_level / self.max_patience).clamp(0.0, 1.0)
    }

    /// Effective outgoing damage scaled by accumulated patience. Returns
    /// `base * (1.0 + patience_bonus * patience_fraction())` when enabled.
    /// Returns `base` when disabled.
    pub fn effective_outgoing(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.patience_bonus * self.patience_fraction())
    }
}

impl Default for Patience {
    fn default() -> Self {
        Self::new(5.0, 0.8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero_patience() {
        let p = Patience::new(5.0, 0.8);
        assert_eq!(p.patience_level, 0.0);
        assert!(!p.is_patient());
    }

    #[test]
    fn tick_increments_patience_level() {
        let mut p = Patience::new(5.0, 0.8);
        p.tick(2.0);
        assert!((p.patience_level - 2.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_max_patience() {
        let mut p = Patience::new(3.0, 0.8);
        p.tick(100.0);
        assert!((p.patience_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_primed_on_first_reach() {
        let mut p = Patience::new(2.0, 0.8);
        p.tick(2.0);
        assert!(p.just_primed);
        assert!(p.is_patient());
    }

    #[test]
    fn tick_no_just_primed_when_already_patient() {
        let mut p = Patience::new(2.0, 0.8);
        p.tick(2.0); // primed
        p.tick(0.016); // still patient, flag cleared
        assert!(!p.just_primed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut p = Patience::new(5.0, 0.8);
        p.enabled = false;
        p.tick(10.0);
        assert_eq!(p.patience_level, 0.0);
    }

    #[test]
    fn tick_clears_just_primed_each_frame() {
        let mut p = Patience::new(1.0, 0.8);
        p.tick(1.0); // just_primed = true
        p.tick(0.016); // cleared
        assert!(!p.just_primed);
    }

    #[test]
    fn tick_clears_just_spent_each_frame() {
        let mut p = Patience::new(1.0, 0.8);
        p.tick(1.0); // patient
        p.on_attack(); // just_spent = true
        p.tick(0.016); // cleared
        assert!(!p.just_spent);
    }

    #[test]
    fn on_attack_resets_patience_level() {
        let mut p = Patience::new(5.0, 0.8);
        p.tick(5.0); // patient
        p.on_attack();
        assert_eq!(p.patience_level, 0.0);
        assert!(!p.is_patient());
    }

    #[test]
    fn on_attack_fires_just_spent_when_patient() {
        let mut p = Patience::new(2.0, 0.8);
        p.tick(2.0); // patient
        p.on_attack();
        assert!(p.just_spent);
    }

    #[test]
    fn on_attack_no_just_spent_when_not_patient() {
        let mut p = Patience::new(5.0, 0.8);
        p.tick(2.0); // partial — not patient
        p.on_attack();
        assert!(!p.just_spent);
        assert_eq!(p.patience_level, 0.0);
    }

    #[test]
    fn on_attack_no_op_when_disabled() {
        let mut p = Patience::new(5.0, 0.8);
        p.tick(5.0); // patient
        p.enabled = false;
        p.on_attack();
        assert!((p.patience_level - 5.0).abs() < 1e-5); // unchanged
    }

    #[test]
    fn is_patient_false_before_max() {
        let mut p = Patience::new(5.0, 0.8);
        p.tick(4.0);
        assert!(!p.is_patient());
    }

    #[test]
    fn is_patient_true_at_max() {
        let mut p = Patience::new(3.0, 0.8);
        p.tick(3.0);
        assert!(p.is_patient());
    }

    #[test]
    fn is_patient_false_when_disabled() {
        let mut p = Patience::new(2.0, 0.8);
        p.patience_level = 2.0;
        p.enabled = false;
        assert!(!p.is_patient());
    }

    #[test]
    fn patience_fraction_at_zero() {
        let p = Patience::new(5.0, 0.8);
        assert_eq!(p.patience_fraction(), 0.0);
    }

    #[test]
    fn patience_fraction_at_half() {
        let mut p = Patience::new(10.0, 0.8);
        p.tick(5.0);
        assert!((p.patience_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn patience_fraction_at_full() {
        let mut p = Patience::new(4.0, 0.8);
        p.tick(4.0);
        assert!((p.patience_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_at_zero_patience() {
        let p = Patience::new(5.0, 1.0);
        assert!((p.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_at_half_patience() {
        let mut p = Patience::new(10.0, 1.0);
        p.tick(5.0); // 0.5 fraction → bonus = 1.0 * 0.5 = 0.5
                     // 100 * (1 + 0.5) = 150
        assert!((p.effective_outgoing(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_at_full_patience() {
        let mut p = Patience::new(5.0, 0.5);
        p.tick(5.0); // full → 100 * (1 + 0.5) = 150
        assert!((p.effective_outgoing(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_base_when_disabled() {
        let mut p = Patience::new(3.0, 1.0);
        p.patience_level = 3.0;
        p.enabled = false;
        assert!((p.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_base_after_attack_resets() {
        let mut p = Patience::new(3.0, 1.0);
        p.tick(3.0); // fully patient
        p.on_attack(); // resets
        assert!((p.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn patience_rebuilds_after_attack() {
        let mut p = Patience::new(4.0, 0.8);
        p.tick(4.0); // patient
        p.on_attack(); // reset
        p.tick(2.0); // half way back
        assert!((p.patience_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn re_primes_after_attack_and_wait() {
        let mut p = Patience::new(2.0, 0.8);
        p.tick(2.0); // primed
        p.tick(0.016);
        p.on_attack(); // spent
        p.tick(0.016); // clear flags
        p.tick(2.0); // primed again
        assert!(p.just_primed);
    }

    #[test]
    fn max_patience_clamped_to_one() {
        let p = Patience::new(0.0, 0.8);
        assert!((p.max_patience - 1.0).abs() < 1e-5);
    }

    #[test]
    fn patience_bonus_clamped_non_negative() {
        let p = Patience::new(5.0, -1.0);
        assert_eq!(p.patience_bonus, 0.0);
    }

    #[test]
    fn zero_patience_bonus_outgoing_unchanged() {
        let mut p = Patience::new(3.0, 0.0);
        p.tick(3.0);
        assert!((p.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }
}
