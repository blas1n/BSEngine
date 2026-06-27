use bevy_ecs::prelude::Component;

/// HP-threshold critical hit boost for desperate last-ditch fighting. When
/// the entity's HP falls below `hp_threshold` (as a fraction of `max_hp`),
/// `is_plucky()` becomes true and `effective_crit()` returns an elevated
/// critical hit chance. The entity fights more recklessly as it nears death.
///
/// `update(hp, max_hp)` recomputes the pluck state. Fires `just_triggered`
/// on the first transition to below-threshold and `just_recovered` when HP
/// rises back above the threshold. No-op when disabled or `max_hp ≤ 0`.
///
/// `tick()` clears `just_triggered` and `just_recovered` each frame.
///
/// `is_plucky()` returns `pluck_active && enabled`.
///
/// `effective_crit(base_crit)` returns `(base_crit + crit_bonus).clamp(0, 1)`
/// when plucky and enabled; returns `base_crit` otherwise.
///
/// Distinct from `Feral` (attack speed bonus from HP threshold — oriented
/// toward speed), `Survive` (death-prevention mechanics), `Wrath` (timed
/// voluntary trade of defense for damage), and `Doom` (lethal countdown
/// clock): Pluck is a **passive last-ditch critical modifier** — the boost
/// activates automatically when HP is low and disappears when health recovers,
/// representing grit and desperation without intentional sacrifice.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Pluck {
    /// HP fraction below which pluck activates. Clamped [0.0, 1.0].
    pub hp_threshold: f32,
    /// Additional critical hit chance while plucky. Clamped [0.0, 1.0].
    pub crit_bonus: f32,
    pub pluck_active: bool,
    pub just_triggered: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Pluck {
    pub fn new(hp_threshold: f32, crit_bonus: f32) -> Self {
        Self {
            hp_threshold: hp_threshold.clamp(0.0, 1.0),
            crit_bonus: crit_bonus.clamp(0.0, 1.0),
            pluck_active: false,
            just_triggered: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Recompute pluck state from current HP values. Fires `just_triggered`
    /// on the false → true transition and `just_recovered` on the true →
    /// false transition. No-op when disabled or `max_hp ≤ 0`.
    pub fn update(&mut self, hp: f32, max_hp: f32) {
        if !self.enabled || max_hp <= 0.0 {
            return;
        }
        let fraction = (hp / max_hp).clamp(0.0, 1.0);
        let should_be_active = fraction <= self.hp_threshold;

        if !self.pluck_active && should_be_active {
            self.pluck_active = true;
            self.just_triggered = true;
        } else if self.pluck_active && !should_be_active {
            self.pluck_active = false;
            self.just_recovered = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_triggered = false;
        self.just_recovered = false;
    }

    /// `true` when HP is below the threshold and the component is enabled.
    pub fn is_plucky(&self) -> bool {
        self.pluck_active && self.enabled
    }

    /// Critical hit chance elevated by `crit_bonus` while plucky.
    /// Returns `(base_crit + crit_bonus).clamp(0, 1)` when plucky and
    /// enabled; returns `base_crit` otherwise.
    pub fn effective_crit(&self, base_crit: f32) -> f32 {
        if self.is_plucky() {
            (base_crit + self.crit_bonus).clamp(0.0, 1.0)
        } else {
            base_crit
        }
    }
}

impl Default for Pluck {
    fn default() -> Self {
        Self::new(0.25, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let p = Pluck::new(0.25, 0.3);
        assert!(!p.pluck_active);
        assert!(!p.is_plucky());
    }

    #[test]
    fn update_activates_below_threshold() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(20.0, 100.0); // 0.2 <= 0.25
        assert!(p.pluck_active);
        assert!(p.just_triggered);
        assert!(p.is_plucky());
    }

    #[test]
    fn update_inactive_above_threshold() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(50.0, 100.0); // 0.5 > 0.25
        assert!(!p.pluck_active);
        assert!(!p.just_triggered);
    }

    #[test]
    fn update_fires_just_recovered_on_rising() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(20.0, 100.0); // activate
        p.tick();
        p.update(50.0, 100.0); // deactivate
        assert!(!p.pluck_active);
        assert!(p.just_recovered);
    }

    #[test]
    fn update_no_just_triggered_when_already_active() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(20.0, 100.0); // activate
        p.tick();
        p.update(15.0, 100.0); // still active
        assert!(!p.just_triggered);
        assert!(!p.just_recovered);
    }

    #[test]
    fn update_no_just_recovered_when_already_inactive() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(50.0, 100.0);
        p.tick();
        p.update(60.0, 100.0);
        assert!(!p.just_recovered);
    }

    #[test]
    fn update_no_op_when_disabled() {
        let mut p = Pluck::new(0.25, 0.3);
        p.enabled = false;
        p.update(10.0, 100.0);
        assert!(!p.pluck_active);
    }

    #[test]
    fn update_no_op_at_zero_max_hp() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(0.0, 0.0);
        assert!(!p.pluck_active);
    }

    #[test]
    fn update_activates_at_exact_threshold() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(25.0, 100.0); // exactly 0.25 <= 0.25
        assert!(p.pluck_active);
    }

    #[test]
    fn update_inactive_just_above_threshold() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(26.0, 100.0); // 0.26 > 0.25
        assert!(!p.pluck_active);
    }

    #[test]
    fn tick_clears_just_triggered() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(10.0, 100.0);
        p.tick();
        assert!(!p.just_triggered);
    }

    #[test]
    fn tick_clears_just_recovered() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(10.0, 100.0);
        p.tick();
        p.update(60.0, 100.0);
        p.tick();
        assert!(!p.just_recovered);
    }

    #[test]
    fn is_plucky_false_when_disabled() {
        let mut p = Pluck::new(0.25, 0.3);
        p.pluck_active = true;
        p.enabled = false;
        assert!(!p.is_plucky());
    }

    #[test]
    fn effective_crit_adds_bonus_when_plucky() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(10.0, 100.0);
        // 0.1 + 0.3 = 0.4
        assert!((p.effective_crit(0.1) - 0.4).abs() < 1e-5);
    }

    #[test]
    fn effective_crit_base_when_not_plucky() {
        let p = Pluck::new(0.25, 0.3);
        assert!((p.effective_crit(0.1) - 0.1).abs() < 1e-5);
    }

    #[test]
    fn effective_crit_base_when_disabled() {
        let mut p = Pluck::new(0.25, 0.3);
        p.pluck_active = true;
        p.enabled = false;
        assert!((p.effective_crit(0.1) - 0.1).abs() < 1e-5);
    }

    #[test]
    fn effective_crit_clamped_at_one() {
        let mut p = Pluck::new(0.25, 0.9);
        p.update(10.0, 100.0);
        // 0.5 + 0.9 = 1.4 → clamped to 1.0
        assert!((p.effective_crit(0.5) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn hp_threshold_clamped_at_one() {
        let p = Pluck::new(2.0, 0.3);
        assert!((p.hp_threshold - 1.0).abs() < 1e-5);
    }

    #[test]
    fn hp_threshold_clamped_at_zero() {
        let p = Pluck::new(-0.5, 0.3);
        assert_eq!(p.hp_threshold, 0.0);
    }

    #[test]
    fn crit_bonus_clamped_at_one() {
        let p = Pluck::new(0.25, 2.0);
        assert!((p.crit_bonus - 1.0).abs() < 1e-5);
    }

    #[test]
    fn crit_bonus_clamped_at_zero() {
        let p = Pluck::new(0.25, -0.5);
        assert_eq!(p.crit_bonus, 0.0);
    }

    #[test]
    fn re_triggers_after_recovery() {
        let mut p = Pluck::new(0.25, 0.3);
        p.update(10.0, 100.0); // plucky
        p.tick();
        p.update(60.0, 100.0); // recovered
        p.tick();
        p.update(10.0, 100.0); // plucky again
        assert!(p.just_triggered);
    }

    #[test]
    fn zero_threshold_never_activates() {
        let mut p = Pluck::new(0.0, 0.3);
        // hp fraction 0 <= 0 → activates
        p.update(0.0, 100.0);
        assert!(p.pluck_active);
        // any positive hp fraction > 0 → not active
        p.tick();
        p.update(1.0, 100.0);
        assert!(!p.pluck_active);
    }

    #[test]
    fn one_threshold_always_active_when_alive() {
        let mut p = Pluck::new(1.0, 0.3);
        p.update(100.0, 100.0); // fraction = 1.0 <= 1.0
        assert!(p.pluck_active);
    }
}
