use bevy_ecs::prelude::Component;

/// Melee lifesteal buff that converts a fraction of physical damage dealt into
/// self-healing.
///
/// While reaving, the damage pipeline calls `heal_from_damage(dealt)` after
/// each melee hit and adds the return value to the attacker's health. At
/// `leech_fraction` = 0.3 the entity heals 30% of every melee hit as HP.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_faded` when the buff expires. `clear()` ends it early (silence,
/// dispel).
///
/// Distinct from `Leech` (a general-purpose resource drain) and `Regen`
/// (flat HP recovery per second): Reave scales with outgoing melee damage and
/// returns zero when the entity is not attacking, making it a combat-tempo
/// sustain rather than a passive recovery tool.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Reave {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] of melee damage converted to healing while reaving.
    pub leech_fraction: f32,
    pub just_reaving: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Reave {
    pub fn new(leech_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            leech_fraction: leech_fraction.clamp(0.0, 1.0),
            just_reaving: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Apply or extend the reave for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_reaving = true;
            }
        }
    }

    /// End the reave immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_faded = true;
        }
    }

    /// Advance the timer; sets `just_faded` when the buff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_reaving = false;
        self.just_faded = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_faded = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// HP to heal based on `damage_dealt` from a melee hit.
    /// Returns `damage * leech_fraction` while active, `0.0` otherwise.
    pub fn heal_from_damage(&self, damage: f32) -> f32 {
        if self.is_active() {
            damage * self.leech_fraction
        } else {
            0.0
        }
    }

    /// Fraction of the reave duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Reave {
    fn default() -> Self {
        Self::new(0.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_reave() {
        let mut r = Reave::new(0.2);
        r.apply(3.0);
        assert!(r.is_active());
        assert!(r.just_reaving);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut r = Reave::new(0.2);
        r.apply(2.0);
        r.tick(0.016);
        r.apply(5.0);
        assert!((r.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut r = Reave::new(0.2);
        r.apply(5.0);
        r.apply(2.0);
        assert!((r.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_reave() {
        let mut r = Reave::new(0.2);
        r.apply(1.0);
        r.tick(1.1);
        assert!(!r.is_active());
        assert!(r.just_faded);
    }

    #[test]
    fn clear_ends_early() {
        let mut r = Reave::new(0.2);
        r.apply(5.0);
        r.clear();
        assert!(!r.is_active());
        assert!(r.just_faded);
    }

    #[test]
    fn heal_from_damage_while_active() {
        let mut r = Reave::new(0.3);
        r.apply(3.0);
        assert!((r.heal_from_damage(100.0) - 30.0).abs() < 1e-4); // 100 * 0.3
    }

    #[test]
    fn heal_from_damage_zero_when_inactive() {
        let r = Reave::new(0.3);
        assert!((r.heal_from_damage(100.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut r = Reave::new(0.2);
        r.apply(2.0);
        r.tick(1.0);
        assert!((r.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut r = Reave::new(0.2);
        r.enabled = false;
        r.apply(5.0);
        assert!(!r.is_active());
    }

    #[test]
    fn tick_clears_just_reaving() {
        let mut r = Reave::new(0.2);
        r.apply(3.0);
        r.tick(0.016);
        assert!(!r.just_reaving);
    }
}
