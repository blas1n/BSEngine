use bevy_ecs::prelude::Component;

/// Peak-condition offensive bonus that rewards staying healthy. When the
/// entity's HP fraction is at or above `hp_threshold`, `is_prideful()`
/// is true and `effective_outgoing()` applies a flat `damage_bonus`. The
/// bonus vanishes the moment HP dips below the threshold, representing
/// the entity fighting at its peak versus struggling while hurt.
///
/// `update(hp, max_hp)` recomputes the prideful state. Fires `just_humbled`
/// on the prideful → not-prideful transition (HP dropped below threshold).
/// Fires `just_restored` on the not-prideful → prideful transition (HP
/// recovered back above threshold). No-op when disabled or `max_hp ≤ 0`.
///
/// `tick()` clears `just_humbled` and `just_restored` each frame.
///
/// `is_prideful()` returns `prideful && enabled`.
///
/// `effective_outgoing(base)` returns `base + damage_bonus` when prideful
/// and enabled; returns `base` otherwise.
///
/// Distinct from `Pluck` (low-HP → crit bonus — desperation grit),
/// `Feral` (low-HP → attack speed bonus — wounded frenzy), `Reckless`
/// (risk/reward offence-defence swap), and `Wrath` (timed voluntary
/// offence burst): Proud is a **peak-condition damage bonus** — the entity
/// fights best when fully healthy and loses its edge when injured.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Proud {
    /// HP fraction at or above which pride holds. Clamped [0.0, 1.0].
    pub hp_threshold: f32,
    /// Flat outgoing damage bonus while prideful. Clamped ≥ 0.0.
    pub damage_bonus: f32,
    pub prideful: bool,
    pub just_humbled: bool,
    pub just_restored: bool,
    pub enabled: bool,
}

impl Proud {
    pub fn new(hp_threshold: f32, damage_bonus: f32) -> Self {
        Self {
            hp_threshold: hp_threshold.clamp(0.0, 1.0),
            damage_bonus: damage_bonus.max(0.0),
            prideful: false,
            just_humbled: false,
            just_restored: false,
            enabled: true,
        }
    }

    /// Recompute prideful state from current HP. Fires `just_humbled` on
    /// the true → false transition and `just_restored` on the false → true
    /// transition. No-op when disabled or `max_hp ≤ 0`.
    pub fn update(&mut self, hp: f32, max_hp: f32) {
        if !self.enabled || max_hp <= 0.0 {
            return;
        }
        let fraction = (hp / max_hp).clamp(0.0, 1.0);
        let should_be_prideful = fraction >= self.hp_threshold;

        if self.prideful && !should_be_prideful {
            self.prideful = false;
            self.just_humbled = true;
        } else if !self.prideful && should_be_prideful {
            self.prideful = true;
            self.just_restored = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_humbled = false;
        self.just_restored = false;
    }

    /// `true` when HP is at or above the threshold and the component is enabled.
    pub fn is_prideful(&self) -> bool {
        self.prideful && self.enabled
    }

    /// Outgoing damage elevated by `damage_bonus` while prideful. Returns
    /// `base + damage_bonus` when prideful and enabled; `base` otherwise.
    pub fn effective_outgoing(&self, base: f32) -> f32 {
        if self.is_prideful() {
            base + self.damage_bonus
        } else {
            base
        }
    }
}

impl Default for Proud {
    fn default() -> Self {
        Self::new(0.8, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_not_prideful() {
        let p = Proud::new(0.8, 5.0);
        assert!(!p.prideful);
        assert!(!p.is_prideful());
    }

    #[test]
    fn update_sets_prideful_above_threshold() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(90.0, 100.0); // 0.9 >= 0.8
        assert!(p.prideful);
        assert!(p.just_restored);
        assert!(p.is_prideful());
    }

    #[test]
    fn update_not_prideful_below_threshold() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(70.0, 100.0); // 0.7 < 0.8
        assert!(!p.prideful);
        assert!(!p.just_restored);
    }

    #[test]
    fn update_fires_just_humbled_on_drop() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(90.0, 100.0); // prideful
        p.tick();
        p.update(70.0, 100.0); // humbled
        assert!(!p.prideful);
        assert!(p.just_humbled);
    }

    #[test]
    fn update_fires_just_restored_on_recovery() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(70.0, 100.0); // not prideful
        p.tick();
        p.update(90.0, 100.0); // restored
        assert!(p.prideful);
        assert!(p.just_restored);
    }

    #[test]
    fn update_no_flag_when_already_prideful() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(90.0, 100.0); // prideful
        p.tick();
        p.update(85.0, 100.0); // still prideful
        assert!(!p.just_restored);
        assert!(!p.just_humbled);
    }

    #[test]
    fn update_no_flag_when_already_not_prideful() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(70.0, 100.0); // not prideful
        p.tick();
        p.update(60.0, 100.0); // still not prideful
        assert!(!p.just_humbled);
        assert!(!p.just_restored);
    }

    #[test]
    fn update_no_op_when_disabled() {
        let mut p = Proud::new(0.8, 5.0);
        p.enabled = false;
        p.update(100.0, 100.0);
        assert!(!p.prideful);
    }

    #[test]
    fn update_no_op_at_zero_max_hp() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(0.0, 0.0);
        assert!(!p.prideful);
    }

    #[test]
    fn update_prideful_at_exact_threshold() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(80.0, 100.0); // exactly 0.8 >= 0.8
        assert!(p.prideful);
    }

    #[test]
    fn update_not_prideful_just_below_threshold() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(79.0, 100.0); // 0.79 < 0.8
        assert!(!p.prideful);
    }

    #[test]
    fn tick_clears_just_humbled() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(90.0, 100.0);
        p.tick();
        p.update(70.0, 100.0);
        p.tick();
        assert!(!p.just_humbled);
    }

    #[test]
    fn tick_clears_just_restored() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(90.0, 100.0);
        p.tick();
        assert!(!p.just_restored);
    }

    #[test]
    fn is_prideful_false_when_disabled() {
        let mut p = Proud::new(0.8, 5.0);
        p.prideful = true;
        p.enabled = false;
        assert!(!p.is_prideful());
    }

    #[test]
    fn effective_outgoing_adds_bonus_when_prideful() {
        let mut p = Proud::new(0.8, 10.0);
        p.update(90.0, 100.0);
        // 100 + 10 = 110
        assert!((p.effective_outgoing(100.0) - 110.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_base_when_not_prideful() {
        let p = Proud::new(0.8, 10.0);
        assert!((p.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_base_when_disabled() {
        let mut p = Proud::new(0.8, 10.0);
        p.prideful = true;
        p.enabled = false;
        assert!((p.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_base_after_humbled() {
        let mut p = Proud::new(0.8, 10.0);
        p.update(90.0, 100.0); // prideful
        p.tick();
        p.update(70.0, 100.0); // humbled
        assert!((p.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn hp_threshold_clamped_to_one() {
        let p = Proud::new(2.0, 5.0);
        assert!((p.hp_threshold - 1.0).abs() < 1e-5);
    }

    #[test]
    fn hp_threshold_clamped_to_zero() {
        let p = Proud::new(-0.5, 5.0);
        assert_eq!(p.hp_threshold, 0.0);
    }

    #[test]
    fn damage_bonus_clamped_non_negative() {
        let p = Proud::new(0.8, -3.0);
        assert_eq!(p.damage_bonus, 0.0);
    }

    #[test]
    fn zero_threshold_always_prideful_when_alive() {
        let mut p = Proud::new(0.0, 5.0);
        p.update(1.0, 100.0); // fraction = 0.01 >= 0.0
        assert!(p.prideful);
    }

    #[test]
    fn one_threshold_only_prideful_at_full_hp() {
        let mut p = Proud::new(1.0, 5.0);
        p.update(100.0, 100.0); // fraction = 1.0 >= 1.0
        assert!(p.prideful);
        p.tick();
        p.update(99.0, 100.0); // 0.99 < 1.0
        assert!(!p.prideful);
        assert!(p.just_humbled);
    }

    #[test]
    fn re_humbled_and_restored_cycle() {
        let mut p = Proud::new(0.8, 5.0);
        p.update(90.0, 100.0); // prideful
        p.tick();
        p.update(70.0, 100.0); // humbled
        p.tick();
        p.update(90.0, 100.0); // restored
        assert!(p.just_restored);
        assert!(p.prideful);
    }
}
