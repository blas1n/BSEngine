use bevy_ecs::prelude::Component;

/// Water/cold covering that suppresses fire-based effects for its duration.
///
/// While active, fire damage is reduced by `fire_resistance` (a multiplier
/// applied to incoming fire damage). Systems that apply Burn, Blaze, or Ignite
/// should check `is_active()` and skip ignition; systems dealing fire damage
/// should scale it via `effective_fire_damage(incoming)`.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_dried` when the douse expires. `clear()` removes it early.
///
/// Distinct from `Freeze` (ice immobilization), `Frostbite` (cold DoT), and
/// `Slow` (movement penalty): Douse is specifically an anti-fire status — it
/// does not impede movement or deal cold damage, it only quenches flames.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Douse {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] of fire damage that is suppressed.
    /// 0.0 = no fire resistance; 1.0 = complete immunity to fire damage.
    pub fire_resistance: f32,
    pub just_doused: bool,
    pub just_dried: bool,
    pub enabled: bool,
}

impl Douse {
    pub fn new(fire_resistance: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            fire_resistance: fire_resistance.clamp(0.0, 1.0),
            just_doused: false,
            just_dried: false,
            enabled: true,
        }
    }

    /// Apply or extend the douse for `duration` seconds. High-watermark: only
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
                self.just_doused = true;
            }
        }
    }

    /// Remove the douse immediately (e.g. entity steps into fire).
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_dried = true;
        }
    }

    /// Advance the timer; sets `just_dried` when the douse expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_doused = false;
        self.just_dried = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_dried = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `incoming * (1.0 - fire_resistance)` while active, `incoming` otherwise.
    pub fn effective_fire_damage(&self, incoming: f32) -> f32 {
        if self.is_active() {
            incoming * (1.0 - self.fire_resistance)
        } else {
            incoming
        }
    }

    /// Fraction of the douse duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Douse {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_douse() {
        let mut d = Douse::new(1.0);
        d.apply(3.0);
        assert!(d.is_active());
        assert!(d.just_doused);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut d = Douse::new(1.0);
        d.apply(2.0);
        d.tick(0.016);
        d.apply(5.0);
        assert!((d.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut d = Douse::new(1.0);
        d.apply(5.0);
        d.apply(2.0);
        assert!((d.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_douse() {
        let mut d = Douse::new(1.0);
        d.apply(1.0);
        d.tick(1.1);
        assert!(!d.is_active());
        assert!(d.just_dried);
    }

    #[test]
    fn clear_ends_early() {
        let mut d = Douse::new(1.0);
        d.apply(5.0);
        d.clear();
        assert!(!d.is_active());
        assert!(d.just_dried);
    }

    #[test]
    fn effective_fire_damage_full_resistance() {
        let mut d = Douse::new(1.0);
        d.apply(3.0);
        assert!((d.effective_fire_damage(100.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn effective_fire_damage_partial_resistance() {
        let mut d = Douse::new(0.6);
        d.apply(3.0);
        assert!((d.effective_fire_damage(100.0) - 40.0).abs() < 1e-3); // 100 * 0.4
    }

    #[test]
    fn effective_fire_damage_when_inactive() {
        let d = Douse::new(1.0);
        assert!((d.effective_fire_damage(80.0) - 80.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut d = Douse::new(1.0);
        d.apply(2.0);
        d.tick(1.0);
        assert!((d.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut d = Douse::new(1.0);
        d.enabled = false;
        d.apply(5.0);
        assert!(!d.is_active());
    }

    #[test]
    fn tick_clears_just_doused() {
        let mut d = Douse::new(1.0);
        d.apply(3.0);
        d.tick(0.016);
        assert!(!d.just_doused);
    }
}
