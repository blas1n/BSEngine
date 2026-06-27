use bevy_ecs::prelude::Component;

/// Agility buff that grants passive dodge chance and a movement speed bonus.
///
/// While nimble, the movement system should use `effective_speed(base)` for
/// the entity's speed, and the combat system should consult `dodge_chance`
/// (a probability in [0.0, 1.0]) to decide whether an incoming attack is
/// avoided. The component does not roll dice itself — callers apply their own
/// RNG against `dodge_chance`.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_faded` on expiry.
///
/// Distinct from `Haste` (pure movement speed multiplier, no dodge benefit),
/// `Dodge` (active roll mechanic with a cooldown), and `Evade` (pure dodge-
/// rate boost with no speed bonus): Nimble is a combined agility surge — it
/// simultaneously makes the entity harder to hit and faster to move.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Nimble {
    pub duration: f32,
    pub timer: f32,
    /// Probability [0.0, 1.0] that an incoming hit is avoided while nimble.
    pub dodge_chance: f32,
    /// Fractional speed bonus while nimble (e.g. 0.3 = +30% speed).
    pub speed_bonus_fraction: f32,
    pub just_quickened: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Nimble {
    pub fn new(dodge_chance: f32, speed_bonus_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            dodge_chance: dodge_chance.clamp(0.0, 1.0),
            speed_bonus_fraction: speed_bonus_fraction.max(0.0),
            just_quickened: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Apply or extend the nimble buff for `duration` seconds. High-watermark:
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
                self.just_quickened = true;
            }
        }
    }

    /// Remove the buff immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_faded = true;
        }
    }

    /// Advance the timer; sets `just_faded` when the buff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_quickened = false;
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

    /// Effective movement speed. Returns `base * (1.0 + speed_bonus_fraction)`
    /// while active, `base` otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.is_active() {
            base * (1.0 + self.speed_bonus_fraction)
        } else {
            base
        }
    }

    /// Active dodge chance. Returns `dodge_chance` while active, `0.0` otherwise.
    pub fn active_dodge_chance(&self) -> f32 {
        if self.is_active() {
            self.dodge_chance
        } else {
            0.0
        }
    }

    /// Fraction of the buff duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Nimble {
    fn default() -> Self {
        Self::new(0.2, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_nimble() {
        let mut n = Nimble::new(0.2, 0.3);
        n.apply(3.0);
        assert!(n.is_active());
        assert!(n.just_quickened);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut n = Nimble::new(0.2, 0.3);
        n.apply(2.0);
        n.tick(0.016);
        n.apply(5.0);
        assert!((n.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut n = Nimble::new(0.2, 0.3);
        n.apply(5.0);
        n.apply(2.0);
        assert!((n.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_nimble() {
        let mut n = Nimble::new(0.2, 0.3);
        n.apply(1.0);
        n.tick(1.1);
        assert!(!n.is_active());
        assert!(n.just_faded);
    }

    #[test]
    fn clear_ends_early() {
        let mut n = Nimble::new(0.2, 0.3);
        n.apply(5.0);
        n.clear();
        assert!(!n.is_active());
        assert!(n.just_faded);
    }

    #[test]
    fn effective_speed_while_active() {
        let mut n = Nimble::new(0.2, 0.3);
        n.apply(3.0);
        assert!((n.effective_speed(10.0) - 13.0).abs() < 1e-4); // 10 * 1.3
    }

    #[test]
    fn effective_speed_when_inactive() {
        let n = Nimble::new(0.2, 0.3);
        assert!((n.effective_speed(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn active_dodge_chance_while_active() {
        let mut n = Nimble::new(0.25, 0.3);
        n.apply(3.0);
        assert!((n.active_dodge_chance() - 0.25).abs() < 1e-5);
    }

    #[test]
    fn active_dodge_chance_when_inactive() {
        let n = Nimble::new(0.25, 0.3);
        assert!((n.active_dodge_chance() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut n = Nimble::new(0.2, 0.3);
        n.apply(2.0);
        n.tick(1.0);
        assert!((n.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut n = Nimble::new(0.2, 0.3);
        n.enabled = false;
        n.apply(5.0);
        assert!(!n.is_active());
    }

    #[test]
    fn tick_clears_just_quickened() {
        let mut n = Nimble::new(0.2, 0.3);
        n.apply(3.0);
        n.tick(0.016);
        assert!(!n.just_quickened);
    }

    #[test]
    fn dodge_chance_clamped() {
        let n = Nimble::new(1.5, 0.3);
        assert!((n.dodge_chance - 1.0).abs() < 1e-5);
        let n2 = Nimble::new(-0.2, 0.3);
        assert!((n2.dodge_chance - 0.0).abs() < 1e-5);
    }

    #[test]
    fn speed_bonus_clamped_to_zero() {
        let n = Nimble::new(0.2, -0.5);
        assert!((n.speed_bonus_fraction - 0.0).abs() < 1e-5);
    }
}
