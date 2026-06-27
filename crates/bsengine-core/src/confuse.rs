use bevy_ecs::prelude::Component;

/// CC status effect that causes an entity to act erratically — attacking random
/// targets, moving in wrong directions, or wasting abilities on allies.
///
/// The game's AI/input system calls `check_confused(rng_value)` each frame
/// before acting. If it returns `true`, the action should be redirected randomly
/// (e.g. pick a random target, flip movement direction). `chance` [0.0, 1.0]
/// is the per-check probability.
///
/// `apply(duration)` extends via high-watermark. `tick(dt)` counts down and
/// sets `just_unconfused` when the effect ends.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Confuse {
    pub duration: f32,
    pub timer: f32,
    /// Probability [0.0, 1.0] that any given action is redirected.
    pub chance: f32,
    pub just_confused: bool,
    pub just_unconfused: bool,
    pub enabled: bool,
}

impl Confuse {
    pub fn new(chance: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            chance: chance.clamp(0.0, 1.0),
            just_confused: false,
            just_unconfused: false,
            enabled: true,
        }
    }

    /// Apply a confusion of `duration` seconds. High-watermark: only replaces
    /// the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_confused = true;
            }
        }
    }

    /// Remove the effect immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_unconfused = true;
        }
    }

    /// Advance the timer; sets `just_unconfused` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_confused = false;
        self.just_unconfused = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_unconfused = true;
            }
        }
    }

    /// Returns `true` if the entity is confused AND `rng_value` (in [0.0, 1.0))
    /// falls below `chance`. Pass a pre-generated random value each frame.
    pub fn check_confused(&self, rng_value: f32) -> bool {
        self.is_active() && rng_value < self.chance
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the confusion duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Confuse {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_confuse() {
        let mut c = Confuse::new(0.75);
        c.apply(3.0);
        assert!(c.is_active());
        assert!(c.just_confused);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut c = Confuse::new(0.75);
        c.apply(2.0);
        c.tick(0.016);
        c.apply(5.0);
        assert!((c.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut c = Confuse::new(0.75);
        c.apply(5.0);
        c.apply(2.0);
        assert!((c.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_confuse() {
        let mut c = Confuse::new(1.0);
        c.apply(1.0);
        c.tick(1.1);
        assert!(!c.is_active());
        assert!(c.just_unconfused);
    }

    #[test]
    fn clear_ends_confuse_early() {
        let mut c = Confuse::new(1.0);
        c.apply(5.0);
        c.clear();
        assert!(!c.is_active());
        assert!(c.just_unconfused);
    }

    #[test]
    fn check_confused_full_chance() {
        let mut c = Confuse::new(1.0);
        c.apply(5.0);
        // rng_value 0.0 < 1.0 → always confused
        assert!(c.check_confused(0.0));
        assert!(c.check_confused(0.999));
    }

    #[test]
    fn check_confused_zero_chance() {
        let mut c = Confuse::new(0.0);
        c.apply(5.0);
        assert!(!c.check_confused(0.0));
    }

    #[test]
    fn check_confused_inactive() {
        let c = Confuse::new(1.0);
        assert!(!c.check_confused(0.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut c = Confuse::new(0.5);
        c.apply(2.0);
        c.tick(1.0);
        let frac = c.remaining_fraction();
        assert!((frac - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut c = Confuse::new(1.0);
        c.enabled = false;
        c.apply(5.0);
        assert!(!c.is_active());
    }
}
