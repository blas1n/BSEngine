use bevy_ecs::prelude::Component;

/// Threshold-triggered primal mode: entity enters feral state when HP falls at
/// or below `hp_threshold_fraction * max_hp`, gaining a flat attack speed bonus
/// until HP recovers above the threshold.
///
/// The combat system calls `update(hp, max_hp)` each frame. `feral_active` is
/// set or cleared automatically; `just_entered` fires once on the inactive →
/// active transition and `just_exited` fires once on the active → inactive
/// transition. Both flags are cleared by `tick()`.
///
/// `effective_attack_speed(base)` returns `base * (1 + attack_speed_bonus)`
/// while feral and enabled; otherwise returns `base`.
///
/// `update()` is a no-op when `max_hp ≤ 0` or disabled. When disabled,
/// `feral_active` is still readable but `is_feral()` returns `false`.
///
/// Distinct from `Fury` (continuously scaled by the fraction of HP lost —
/// higher damage the less HP remains, not a snap threshold), `Rage` (a
/// manually triggered discrete anger state with its own duration), and
/// `Rampage` (stacks from kills, not HP): Feral is a **snap threshold**
/// — the bonus is binary (on or off), activates when HP crosses a fixed
/// fraction, and specifically affects attack speed rather than damage.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Feral {
    /// HP fraction at or below which feral activates. Clamped [0.0, 1.0].
    pub hp_threshold_fraction: f32,
    /// Attack speed multiplier added while feral. Clamped ≥ 0.0.
    pub attack_speed_bonus: f32,
    /// Whether feral mode is currently active.
    pub feral_active: bool,
    pub just_entered: bool,
    pub just_exited: bool,
    pub enabled: bool,
}

impl Feral {
    pub fn new(hp_threshold_fraction: f32, attack_speed_bonus: f32) -> Self {
        Self {
            hp_threshold_fraction: hp_threshold_fraction.clamp(0.0, 1.0),
            attack_speed_bonus: attack_speed_bonus.max(0.0),
            feral_active: false,
            just_entered: false,
            just_exited: false,
            enabled: true,
        }
    }

    /// Evaluate current HP against the threshold and update `feral_active`.
    /// Fires `just_entered` on the transition to active, `just_exited` on the
    /// transition to inactive. No-op when disabled or `max_hp ≤ 0`.
    pub fn update(&mut self, hp: f32, max_hp: f32) {
        if !self.enabled || max_hp <= 0.0 {
            return;
        }
        let should_be_feral = hp / max_hp <= self.hp_threshold_fraction;
        if should_be_feral && !self.feral_active {
            self.feral_active = true;
            self.just_entered = true;
        } else if !should_be_feral && self.feral_active {
            self.feral_active = false;
            self.just_exited = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_entered = false;
        self.just_exited = false;
    }

    /// `true` when feral mode is active and the component is enabled.
    pub fn is_feral(&self) -> bool {
        self.feral_active && self.enabled
    }

    /// Effective attack speed scaled by the feral bonus.
    /// Returns `base * (1 + attack_speed_bonus)` while feral and enabled;
    /// returns `base` otherwise.
    pub fn effective_attack_speed(&self, base: f32) -> f32 {
        if self.is_feral() {
            base * (1.0 + self.attack_speed_bonus)
        } else {
            base
        }
    }
}

impl Default for Feral {
    fn default() -> Self {
        Self::new(0.3, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let f = Feral::new(0.3, 0.5);
        assert!(!f.feral_active);
        assert!(!f.is_feral());
    }

    #[test]
    fn update_activates_at_threshold() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(30.0, 100.0); // 0.30 <= 0.30 → active
        assert!(f.feral_active);
        assert!(f.just_entered);
    }

    #[test]
    fn update_activates_below_threshold() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(20.0, 100.0); // 0.20 <= 0.30 → active
        assert!(f.feral_active);
    }

    #[test]
    fn update_inactive_above_threshold() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(50.0, 100.0); // 0.50 > 0.30 → inactive
        assert!(!f.feral_active);
        assert!(!f.just_entered);
    }

    #[test]
    fn update_fires_just_exited_on_recovery() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(20.0, 100.0); // enters feral
        f.tick();
        f.update(50.0, 100.0); // HP recovers
        assert!(!f.feral_active);
        assert!(f.just_exited);
    }

    #[test]
    fn update_no_just_entered_when_already_feral() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(20.0, 100.0); // enters
        f.tick();
        f.update(10.0, 100.0); // still feral, deeper
        assert!(!f.just_entered);
    }

    #[test]
    fn update_no_just_exited_when_already_inactive() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(80.0, 100.0);
        f.tick();
        f.update(90.0, 100.0);
        assert!(!f.just_exited);
    }

    #[test]
    fn update_no_op_when_disabled() {
        let mut f = Feral::new(0.3, 0.5);
        f.enabled = false;
        f.update(10.0, 100.0);
        assert!(!f.feral_active);
    }

    #[test]
    fn update_no_op_when_max_hp_zero() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(0.0, 0.0);
        assert!(!f.feral_active);
    }

    #[test]
    fn tick_clears_just_entered() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(20.0, 100.0);
        f.tick();
        assert!(!f.just_entered);
    }

    #[test]
    fn tick_clears_just_exited() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(20.0, 100.0);
        f.tick();
        f.update(80.0, 100.0);
        f.tick();
        assert!(!f.just_exited);
    }

    #[test]
    fn is_feral_false_when_disabled() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(10.0, 100.0); // active
        f.enabled = false;
        assert!(!f.is_feral());
    }

    #[test]
    fn effective_attack_speed_boosted_while_feral() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(20.0, 100.0);
        // 100 * (1 + 0.5) = 150
        assert!((f.effective_attack_speed(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_attack_speed_base_when_inactive() {
        let f = Feral::new(0.3, 0.5);
        assert!((f.effective_attack_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_attack_speed_base_when_disabled() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(20.0, 100.0);
        f.enabled = false;
        assert!((f.effective_attack_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn re_enters_after_recovery_and_redamage() {
        let mut f = Feral::new(0.3, 0.5);
        f.update(20.0, 100.0); // enters
        f.tick();
        f.update(80.0, 100.0); // recovers
        f.tick();
        f.update(10.0, 100.0); // enters again
        assert!(f.just_entered);
    }

    #[test]
    fn hp_threshold_fraction_clamped() {
        let f = Feral::new(2.0, 0.5);
        assert!((f.hp_threshold_fraction - 1.0).abs() < 1e-5);
        let f2 = Feral::new(-0.5, 0.5);
        assert_eq!(f2.hp_threshold_fraction, 0.0);
    }

    #[test]
    fn attack_speed_bonus_clamped_non_negative() {
        let f = Feral::new(0.3, -1.0);
        assert_eq!(f.attack_speed_bonus, 0.0);
    }

    #[test]
    fn threshold_zero_never_activates_unless_hp_zero() {
        let mut f = Feral::new(0.0, 0.5);
        f.update(1.0, 100.0); // 0.01 > 0.0 → inactive
        assert!(!f.feral_active);
        f.update(0.0, 100.0); // 0.0 <= 0.0 → active
        assert!(f.feral_active);
    }

    #[test]
    fn threshold_one_always_activates() {
        let mut f = Feral::new(1.0, 0.5);
        f.update(100.0, 100.0); // 1.0 <= 1.0 → active
        assert!(f.feral_active);
    }
}
