use bevy_ecs::prelude::Component;

/// Battle-frenzy buff that increases attack speed at the cost of slight
/// self-damage on each swing.
///
/// While in fervor, the attack system should:
/// - Multiply the entity's attack speed by `effective_attack_speed(base)` to
///   reduce the interval between attacks.
/// - Call `on_attack()` each time the entity lands a hit and apply the
///   returned value as self-damage to the attacker.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_subsided` when the fervor burns out.
///
/// Distinct from `Rage` (damage multiplier with HP cost per tick), `Haste`
/// (general movement and action speed), and `Galvanize` (defense stack buff):
/// Fervor is specifically an attack-speed rush with per-swing self-recklessness
/// — the entity attacks in a frenzy, hitting more often but also taking minor
/// self-damage each swing.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Fervor {
    pub duration: f32,
    pub timer: f32,
    /// Additional fraction added to the attack speed multiplier while frenzied.
    /// e.g. 0.5 = entity attacks 50% faster (1.5× base attack speed).
    pub attack_speed_bonus: f32,
    /// Self-damage applied each time the entity attacks while in fervor.
    pub self_damage_per_attack: f32,
    pub just_enraged: bool,
    pub just_subsided: bool,
    pub enabled: bool,
}

impl Fervor {
    pub fn new(attack_speed_bonus: f32, self_damage_per_attack: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            attack_speed_bonus: attack_speed_bonus.max(0.0),
            self_damage_per_attack: self_damage_per_attack.max(0.0),
            just_enraged: false,
            just_subsided: false,
            enabled: true,
        }
    }

    /// Apply or extend fervor for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_enraged = true;
            }
        }
    }

    /// End fervor immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_subsided = true;
        }
    }

    /// Advance the timer; sets `just_subsided` when the fervor expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_enraged = false;
        self.just_subsided = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_subsided = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective attack speed multiplier. Returns `base * (1 + attack_speed_bonus)`
    /// while active, `base` otherwise.
    pub fn effective_attack_speed(&self, base: f32) -> f32 {
        if self.is_active() {
            base * (1.0 + self.attack_speed_bonus)
        } else {
            base
        }
    }

    /// Self-damage to apply each time the entity attacks. Returns
    /// `self_damage_per_attack` while active, `0.0` otherwise.
    pub fn on_attack(&self) -> f32 {
        if self.is_active() {
            self.self_damage_per_attack
        } else {
            0.0
        }
    }

    /// Fraction of the fervor duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Fervor {
    fn default() -> Self {
        Self::new(0.5, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_fervor() {
        let mut f = Fervor::new(0.5, 5.0);
        f.apply(3.0);
        assert!(f.is_active());
        assert!(f.just_enraged);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut f = Fervor::new(0.5, 5.0);
        f.apply(2.0);
        f.tick(0.016);
        f.apply(5.0);
        assert!((f.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut f = Fervor::new(0.5, 5.0);
        f.apply(5.0);
        f.apply(2.0);
        assert!((f.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_fervor() {
        let mut f = Fervor::new(0.5, 5.0);
        f.apply(1.0);
        f.tick(1.1);
        assert!(!f.is_active());
        assert!(f.just_subsided);
    }

    #[test]
    fn clear_ends_early() {
        let mut f = Fervor::new(0.5, 5.0);
        f.apply(5.0);
        f.clear();
        assert!(!f.is_active());
        assert!(f.just_subsided);
    }

    #[test]
    fn effective_attack_speed_while_active() {
        let mut f = Fervor::new(0.5, 5.0);
        f.apply(3.0);
        assert!((f.effective_attack_speed(1.0) - 1.5).abs() < 1e-5);
    }

    #[test]
    fn effective_attack_speed_when_inactive() {
        let f = Fervor::new(0.5, 5.0);
        assert!((f.effective_attack_speed(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn on_attack_while_active() {
        let mut f = Fervor::new(0.5, 5.0);
        f.apply(3.0);
        assert!((f.on_attack() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn on_attack_when_inactive() {
        let f = Fervor::new(0.5, 5.0);
        assert!((f.on_attack() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut f = Fervor::new(0.5, 5.0);
        f.apply(2.0);
        f.tick(1.0);
        assert!((f.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut f = Fervor::new(0.5, 5.0);
        f.enabled = false;
        f.apply(5.0);
        assert!(!f.is_active());
    }

    #[test]
    fn tick_clears_just_enraged() {
        let mut f = Fervor::new(0.5, 5.0);
        f.apply(3.0);
        f.tick(0.016);
        assert!(!f.just_enraged);
    }

    #[test]
    fn negative_bonus_clamped_to_zero() {
        let f = Fervor::new(-0.3, -1.0);
        assert!((f.attack_speed_bonus - 0.0).abs() < 1e-5);
        assert!((f.self_damage_per_attack - 0.0).abs() < 1e-5);
    }
}
