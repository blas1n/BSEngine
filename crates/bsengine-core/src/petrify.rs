use bevy_ecs::prelude::Component;

/// Full-immobilization CC that turns the entity to stone: all movement and
/// actions are blocked, but the entity gains `armor_bonus` fractional damage
/// reduction while petrified (stone is harder to damage).
///
/// Distinct from `Stun` (no armor bonus) and `Freeze` (ice amplification):
/// Petrify trades vulnerability for extra toughness while immobilized.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_unpetrified` when the stone wears off.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Petrify {
    pub duration: f32,
    pub timer: f32,
    /// Additional damage reduction fraction [0.0, 1.0] while petrified.
    /// e.g. 0.3 = 30% less damage taken (stone skin).
    pub armor_bonus: f32,
    pub just_petrified: bool,
    pub just_unpetrified: bool,
    pub enabled: bool,
}

impl Petrify {
    pub fn new(armor_bonus: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            armor_bonus: armor_bonus.clamp(0.0, 1.0),
            just_petrified: false,
            just_unpetrified: false,
            enabled: true,
        }
    }

    /// Apply or extend a petrify of `duration` seconds. High-watermark: only
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
                self.just_petrified = true;
            }
        }
    }

    /// Break the stone immediately (e.g. from a shatter ability).
    pub fn shatter(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_unpetrified = true;
        }
    }

    /// Advance the timer; sets `just_unpetrified` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_petrified = false;
        self.just_unpetrified = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_unpetrified = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective incoming damage multiplier while petrified.
    /// Returns `1.0 - armor_bonus` when active, otherwise 1.0.
    pub fn damage_multiplier(&self) -> f32 {
        if self.is_active() {
            1.0 - self.armor_bonus
        } else {
            1.0
        }
    }

    /// Fraction of the petrify duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Petrify {
    fn default() -> Self {
        Self::new(0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_petrify() {
        let mut p = Petrify::new(0.3);
        p.apply(3.0);
        assert!(p.is_active());
        assert!(p.just_petrified);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut p = Petrify::new(0.3);
        p.apply(2.0);
        p.tick(0.016);
        p.apply(5.0);
        assert!((p.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut p = Petrify::new(0.3);
        p.apply(5.0);
        p.apply(2.0);
        assert!((p.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_petrify() {
        let mut p = Petrify::new(0.3);
        p.apply(1.0);
        p.tick(1.1);
        assert!(!p.is_active());
        assert!(p.just_unpetrified);
    }

    #[test]
    fn shatter_ends_early() {
        let mut p = Petrify::new(0.3);
        p.apply(5.0);
        p.shatter();
        assert!(!p.is_active());
        assert!(p.just_unpetrified);
    }

    #[test]
    fn damage_multiplier_while_active() {
        let mut p = Petrify::new(0.4);
        p.apply(3.0);
        let m = p.damage_multiplier();
        assert!((m - 0.6).abs() < 1e-5);
    }

    #[test]
    fn damage_multiplier_when_inactive() {
        let p = Petrify::new(0.4);
        assert!((p.damage_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut p = Petrify::new(0.3);
        p.apply(2.0);
        p.tick(1.0);
        let frac = p.remaining_fraction();
        assert!((frac - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut p = Petrify::new(0.3);
        p.enabled = false;
        p.apply(5.0);
        assert!(!p.is_active());
    }
}
