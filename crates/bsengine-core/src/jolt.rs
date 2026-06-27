use bevy_ecs::prelude::Component;

/// Brief electrical CC that interrupts actions and can arc to nearby entities.
///
/// Lighter than `Stun` (no full action lockout, shorter durations) but with
/// a chain mechanic: when the jolted entity takes the hit, the ability system
/// can call `check_chain(rng_value)` to see whether the effect should spread
/// to an adjacent target.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_expired` on the frame the jolt wears off.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Jolt {
    pub duration: f32,
    pub timer: f32,
    /// Probability [0.0, 1.0] that this jolt arcs to a nearby entity.
    pub chain_chance: f32,
    /// Fraction of the original damage/duration passed to chained targets.
    pub chain_fraction: f32,
    pub just_jolted: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Jolt {
    pub fn new(chain_chance: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            chain_chance: chain_chance.clamp(0.0, 1.0),
            chain_fraction: 0.5,
            just_jolted: false,
            just_expired: false,
            enabled: true,
        }
    }

    pub fn with_chain_fraction(mut self, fraction: f32) -> Self {
        self.chain_fraction = fraction.clamp(0.0, 1.0);
        self
    }

    /// Apply or extend a jolt of `duration` seconds. High-watermark: only
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
                self.just_jolted = true;
            }
        }
    }

    /// Remove the jolt immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_expired = true;
        }
    }

    /// Advance the timer; sets `just_expired` when the effect ends.
    pub fn tick(&mut self, dt: f32) {
        self.just_jolted = false;
        self.just_expired = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_expired = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `true` if the jolt should chain to a nearby entity.
    /// `rng_value` must be a pre-rolled float in [0.0, 1.0).
    pub fn check_chain(&self, rng_value: f32) -> bool {
        self.enabled && rng_value < self.chain_chance
    }

    /// Fraction of the jolt duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Jolt {
    fn default() -> Self {
        Self::new(0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_jolt() {
        let mut j = Jolt::new(0.3);
        j.apply(0.5);
        assert!(j.is_active());
        assert!(j.just_jolted);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut j = Jolt::new(0.3);
        j.apply(0.5);
        j.tick(0.016);
        j.apply(1.0);
        assert!((j.timer - 1.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut j = Jolt::new(0.3);
        j.apply(1.0);
        j.apply(0.3);
        assert!((j.timer - 1.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_jolt() {
        let mut j = Jolt::new(0.3);
        j.apply(0.5);
        j.tick(0.6);
        assert!(!j.is_active());
        assert!(j.just_expired);
    }

    #[test]
    fn clear_ends_early() {
        let mut j = Jolt::new(0.3);
        j.apply(1.0);
        j.clear();
        assert!(!j.is_active());
        assert!(j.just_expired);
    }

    #[test]
    fn check_chain_below_chance() {
        let j = Jolt::new(0.5);
        assert!(j.check_chain(0.3)); // 0.3 < 0.5 → chain
    }

    #[test]
    fn check_chain_above_chance() {
        let j = Jolt::new(0.5);
        assert!(!j.check_chain(0.7)); // 0.7 >= 0.5 → no chain
    }

    #[test]
    fn check_chain_zero_chance() {
        let j = Jolt::new(0.0);
        assert!(!j.check_chain(0.0));
    }

    #[test]
    fn check_chain_full_chance() {
        let j = Jolt::new(1.0);
        assert!(j.check_chain(0.999));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut j = Jolt::new(0.3);
        j.apply(2.0);
        j.tick(1.0);
        assert!((j.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut j = Jolt::new(0.3);
        j.enabled = false;
        j.apply(1.0);
        assert!(!j.is_active());
    }

    #[test]
    fn disabled_check_chain_false() {
        let mut j = Jolt::new(1.0);
        j.enabled = false;
        assert!(!j.check_chain(0.0));
    }
}
