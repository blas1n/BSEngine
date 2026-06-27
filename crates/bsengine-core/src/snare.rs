use bevy_ecs::prelude::Component;

/// Trap or tether debuff that partially or fully restricts an entity's movement.
///
/// While snared, the movement pipeline should multiply the entity's speed by
/// `movement_fraction()` (0.0 = full root, 0.5 = half speed, etc.). Each frame
/// the AI/player can call `try_escape(rng_value)` — if it returns `true` the
/// snare is broken early and `just_escaped` is set.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_escaped` when the snare expires naturally (the same flag is used for
/// both escape and expiry to simplify callers).
///
/// Distinct from `Slow` (universal speed multiplier) and `Entangle` (full CC):
/// Snare is a physical trap mechanic that allows partial movement and active escape.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Snare {
    pub duration: f32,
    pub timer: f32,
    /// Movement fraction while snared [0.0 = full root, 1.0 = unimpeded].
    pub slow_fraction: f32,
    /// Probability [0.0, 1.0] per `try_escape` call of breaking free.
    pub escape_chance: f32,
    pub just_snared: bool,
    pub just_escaped: bool,
    pub enabled: bool,
}

impl Snare {
    pub fn new(slow_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            slow_fraction: slow_fraction.clamp(0.0, 1.0),
            escape_chance: 0.0,
            just_snared: false,
            just_escaped: false,
            enabled: true,
        }
    }

    pub fn with_escape_chance(mut self, chance: f32) -> Self {
        self.escape_chance = chance.clamp(0.0, 1.0);
        self
    }

    /// Apply or extend the snare for `duration` seconds. High-watermark: only
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
                self.just_snared = true;
            }
        }
    }

    /// Break the snare immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_escaped = true;
        }
    }

    /// Advance the timer. Sets `just_escaped` when the snare expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_snared = false;
        self.just_escaped = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_escaped = true;
            }
        }
    }

    /// Attempt to break free. `rng_value` must be a pre-rolled float in [0.0, 1.0).
    /// Returns `true` (and sets `just_escaped`) when the escape succeeds.
    pub fn try_escape(&mut self, rng_value: f32) -> bool {
        if !self.enabled || !self.is_active() {
            return false;
        }

        if rng_value < self.escape_chance {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_escaped = true;
            return true;
        }

        false
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Movement speed fraction while snared; `1.0` when inactive.
    pub fn movement_fraction(&self) -> f32 {
        if self.is_active() {
            self.slow_fraction
        } else {
            1.0
        }
    }

    /// Fraction of the snare duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Snare {
    fn default() -> Self {
        Self::new(0.0) // full root by default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_snare() {
        let mut s = Snare::new(0.3);
        s.apply(3.0);
        assert!(s.is_active());
        assert!(s.just_snared);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut s = Snare::new(0.3);
        s.apply(2.0);
        s.tick(0.016);
        s.apply(5.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut s = Snare::new(0.3);
        s.apply(5.0);
        s.apply(2.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_snare() {
        let mut s = Snare::new(0.3);
        s.apply(1.0);
        s.tick(1.1);
        assert!(!s.is_active());
        assert!(s.just_escaped);
    }

    #[test]
    fn clear_ends_early() {
        let mut s = Snare::new(0.3);
        s.apply(5.0);
        s.clear();
        assert!(!s.is_active());
        assert!(s.just_escaped);
    }

    #[test]
    fn movement_fraction_while_active() {
        let mut s = Snare::new(0.4);
        s.apply(3.0);
        assert!((s.movement_fraction() - 0.4).abs() < 1e-5);
    }

    #[test]
    fn movement_fraction_when_inactive() {
        let s = Snare::new(0.4);
        assert!((s.movement_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn try_escape_succeeds_below_chance() {
        let mut s = Snare::new(0.0).with_escape_chance(0.5);
        s.apply(5.0);
        assert!(s.try_escape(0.3));
        assert!(!s.is_active());
        assert!(s.just_escaped);
    }

    #[test]
    fn try_escape_fails_above_chance() {
        let mut s = Snare::new(0.0).with_escape_chance(0.5);
        s.apply(5.0);
        assert!(!s.try_escape(0.7));
        assert!(s.is_active());
    }

    #[test]
    fn try_escape_when_inactive_false() {
        let mut s = Snare::new(0.0).with_escape_chance(1.0);
        assert!(!s.try_escape(0.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut s = Snare::new(0.3);
        s.apply(2.0);
        s.tick(1.0);
        assert!((s.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut s = Snare::new(0.3);
        s.enabled = false;
        s.apply(5.0);
        assert!(!s.is_active());
    }

    #[test]
    fn disabled_try_escape_false() {
        let mut s = Snare::new(0.0).with_escape_chance(1.0);
        s.apply(5.0);
        s.enabled = false;
        assert!(!s.try_escape(0.0));
        assert!(s.is_active()); // still active
    }
}
