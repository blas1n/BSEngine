use bevy_ecs::prelude::Component;

/// Reckless attacking stance: the entity consciously trades protection for
/// aggression. While `is_reckless()` and enabled, outgoing damage is boosted
/// and incoming defense is penalized — the entity hits harder but gets hurt
/// more.
///
/// `charge(duration)` starts or extends the stance (high-watermark); sets
/// `just_entered` on the inactive → active transition. `snap_out()` ends it
/// early. `tick(dt)` counts down and sets `just_exited` on expiry.
///
/// Distinct from `Rage` (anger state, triggers on low HP, involuntary),
/// `Surge` (one-time flat stat burst, no duration), and `Enlarge` (physical
/// size change with speed penalty): Reckless is a **tactical stance choice** —
/// the entity deliberately accepts more incoming damage in exchange for dealing
/// more outgoing damage for a set window.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Reckless {
    pub duration: f32,
    pub timer: f32,
    /// Fraction of base damage added while reckless. Clamped ≥ 0.0.
    /// e.g. 0.4 → entity deals 140% damage.
    pub damage_bonus: f32,
    /// Fraction of effective defense lost while reckless. Clamped [0.0, 1.0].
    /// e.g. 0.3 → entity takes 30% more damage (defense at 70%).
    pub defense_penalty: f32,
    pub just_entered: bool,
    pub just_exited: bool,
    pub enabled: bool,
}

impl Reckless {
    pub fn new(damage_bonus: f32, defense_penalty: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage_bonus: damage_bonus.max(0.0),
            defense_penalty: defense_penalty.clamp(0.0, 1.0),
            just_entered: false,
            just_exited: false,
            enabled: true,
        }
    }

    /// Enter or extend the reckless stance for `duration` seconds.
    /// High-watermark: only replaces the current timer when `duration > timer`.
    /// Sets `just_entered` on the inactive → active transition. No-op when
    /// disabled or `duration ≤ 0`.
    pub fn charge(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_reckless = self.is_reckless();
            self.duration = duration;
            self.timer = duration;
            if !was_reckless {
                self.just_entered = true;
            }
        }
    }

    /// End the reckless stance early (e.g., entity is stunned or chooses to
    /// stop). Sets `just_exited`. No-op when not reckless.
    pub fn snap_out(&mut self) {
        if !self.is_reckless() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_exited = true;
    }

    /// Advance the reckless timer. Sets `just_exited` when the stance expires
    /// naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_entered = false;
        self.just_exited = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_exited = true;
            }
        }
    }

    pub fn is_reckless(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective outgoing damage while reckless.
    /// Returns `base * (1 + damage_bonus)` when reckless and enabled,
    /// `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_reckless() && self.enabled {
            base * (1.0 + self.damage_bonus)
        } else {
            base
        }
    }

    /// Effective incoming defense while reckless. Defense systems multiply
    /// their mitigation by this value. Returns `1 - defense_penalty` when
    /// reckless and enabled, `1.0` otherwise.
    pub fn defense_multiplier(&self) -> f32 {
        if self.is_reckless() && self.enabled {
            1.0 - self.defense_penalty
        } else {
            1.0
        }
    }

    /// Fraction of the reckless duration remaining [1.0 = just charged, 0.0 = normal].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Reckless {
    fn default() -> Self {
        Self::new(0.4, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_starts_reckless() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(3.0);
        assert!(r.is_reckless());
        assert!(r.just_entered);
    }

    #[test]
    fn charge_extends_on_longer_duration() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(2.0);
        r.tick(0.016);
        r.charge(5.0);
        assert!((r.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn charge_no_extend_on_shorter_duration() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(5.0);
        r.charge(2.0);
        assert!((r.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn just_entered_not_set_on_extend() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(2.0);
        r.tick(0.016);
        r.charge(5.0);
        assert!(!r.just_entered);
    }

    #[test]
    fn snap_out_ends_reckless() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(3.0);
        r.snap_out();
        assert!(!r.is_reckless());
        assert!(r.just_exited);
    }

    #[test]
    fn snap_out_no_op_when_not_reckless() {
        let mut r = Reckless::new(0.4, 0.3);
        r.snap_out();
        assert!(!r.just_exited);
    }

    #[test]
    fn tick_expires_reckless() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(1.0);
        r.tick(1.1);
        assert!(!r.is_reckless());
        assert!(r.just_exited);
    }

    #[test]
    fn tick_clears_just_entered() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(3.0);
        r.tick(0.016);
        assert!(!r.just_entered);
    }

    #[test]
    fn tick_clears_just_exited() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(0.5);
        r.tick(1.0);
        r.tick(0.016);
        assert!(!r.just_exited);
    }

    #[test]
    fn effective_damage_boosted_while_reckless() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(3.0);
        assert!((r.effective_damage(100.0) - 140.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_normal() {
        let r = Reckless::new(0.4, 0.3);
        assert!((r.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn defense_multiplier_reduced_while_reckless() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(3.0);
        assert!((r.defense_multiplier() - 0.7).abs() < 1e-5);
    }

    #[test]
    fn defense_multiplier_one_when_normal() {
        let r = Reckless::new(0.4, 0.3);
        assert!((r.defense_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(4.0);
        r.tick(2.0);
        assert!((r.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_normal() {
        let r = Reckless::new(0.4, 0.3);
        assert!((r.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_charge_no_op() {
        let mut r = Reckless::new(0.4, 0.3);
        r.enabled = false;
        r.charge(3.0);
        assert!(!r.is_reckless());
    }

    #[test]
    fn disabled_effective_damage_base() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(3.0);
        r.enabled = false;
        assert!((r.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_defense_multiplier_one() {
        let mut r = Reckless::new(0.4, 0.3);
        r.charge(3.0);
        r.enabled = false;
        assert!((r.defense_multiplier() - 1.0).abs() < 1e-5);
    }
}
