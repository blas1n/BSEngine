use bevy_ecs::prelude::Component;

/// Strength-reduction debuff that lowers the entity's outgoing damage output.
///
/// While weakened, the damage pipeline should multiply outgoing damage by
/// `damage_multiplier()` (< 1.0). Distinct from `Slow` (movement penalty),
/// `Daze` (aim deviation), and `Confuse` (random misfires): `Weaken` is a
/// clean, multiplicative penalty on raw damage output.
///
/// Examples: grievous wounds, muscle weakness, armour-break debuffs.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_unweakened` when the debuff expires.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weaken {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] applied to outgoing damage while weakened.
    /// e.g. 0.7 = entity deals 70% of normal damage (30% reduction).
    pub strength_fraction: f32,
    pub just_weakened: bool,
    pub just_unweakened: bool,
    pub enabled: bool,
}

impl Weaken {
    pub fn new(strength_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            strength_fraction: strength_fraction.clamp(0.0, 1.0),
            just_weakened: false,
            just_unweakened: false,
            enabled: true,
        }
    }

    /// Apply or extend the weaken for `duration` seconds. High-watermark: only
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
                self.just_weakened = true;
            }
        }
    }

    /// Remove the weaken immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_unweakened = true;
        }
    }

    /// Advance the timer; sets `just_unweakened` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_weakened = false;
        self.just_unweakened = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_unweakened = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Multiply outgoing damage by this value.
    /// Returns `strength_fraction` while active, `1.0` otherwise.
    pub fn damage_multiplier(&self) -> f32 {
        if self.is_active() {
            self.strength_fraction
        } else {
            1.0
        }
    }

    /// Fraction of the weaken duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Weaken {
    fn default() -> Self {
        Self::new(0.7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_weaken() {
        let mut w = Weaken::new(0.7);
        w.apply(3.0);
        assert!(w.is_active());
        assert!(w.just_weakened);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut w = Weaken::new(0.7);
        w.apply(2.0);
        w.tick(0.016);
        w.apply(5.0);
        assert!((w.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut w = Weaken::new(0.7);
        w.apply(5.0);
        w.apply(2.0);
        assert!((w.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_weaken() {
        let mut w = Weaken::new(0.7);
        w.apply(1.0);
        w.tick(1.1);
        assert!(!w.is_active());
        assert!(w.just_unweakened);
    }

    #[test]
    fn clear_ends_early() {
        let mut w = Weaken::new(0.7);
        w.apply(5.0);
        w.clear();
        assert!(!w.is_active());
        assert!(w.just_unweakened);
    }

    #[test]
    fn damage_multiplier_while_active() {
        let mut w = Weaken::new(0.6);
        w.apply(3.0);
        assert!((w.damage_multiplier() - 0.6).abs() < 1e-5);
    }

    #[test]
    fn damage_multiplier_when_inactive() {
        let w = Weaken::new(0.6);
        assert!((w.damage_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut w = Weaken::new(0.7);
        w.apply(2.0);
        w.tick(1.0);
        assert!((w.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut w = Weaken::new(0.7);
        w.enabled = false;
        w.apply(5.0);
        assert!(!w.is_active());
    }

    #[test]
    fn tick_clears_just_weakened() {
        let mut w = Weaken::new(0.7);
        w.apply(3.0);
        w.tick(0.016);
        assert!(!w.just_weakened);
    }
}
