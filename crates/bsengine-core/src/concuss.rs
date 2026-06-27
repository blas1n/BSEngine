use bevy_ecs::prelude::Component;

/// Head-trauma debuff that adds random aim deviation and intermittently
/// suppresses ability activation.
///
/// `check_suppress(rng_value, dt)` returns `true` when the entity's ability
/// attempt should fail this frame: `rng_value < ability_suppress_chance * dt`.
/// Pass a value in [0.0, 1.0) sampled by the ability system before activation.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_cleared` on expiry. `clear()` removes the debuff immediately.
///
/// Distinct from `Daze` (pure aim penalty, no ability disruption) and `Stun`
/// (full CC that blocks all actions): Concuss represents head trauma where the
/// entity can still act but both aim and concentration are impaired.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Concuss {
    pub duration: f32,
    pub timer: f32,
    /// Maximum random angular aim deviation in radians while concussed.
    pub aim_deviation_rad: f32,
    /// Expected ability failures per second while concussed. Values > 1.0 are
    /// allowed (multiple expected failures per second).
    pub ability_suppress_chance: f32,
    pub just_concussed: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Concuss {
    pub fn new(aim_deviation_rad: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            aim_deviation_rad: aim_deviation_rad.max(0.0),
            ability_suppress_chance: 0.0,
            just_concussed: false,
            just_cleared: false,
            enabled: true,
        }
    }

    pub fn with_suppress_chance(mut self, chance: f32) -> Self {
        self.ability_suppress_chance = chance.max(0.0);
        self
    }

    /// Apply or extend the concuss for `duration` seconds. High-watermark:
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
                self.just_concussed = true;
            }
        }
    }

    /// Remove the concuss immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_cleared = true;
        }
    }

    /// Advance the timer; sets `just_cleared` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_concussed = false;
        self.just_cleared = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_cleared = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `true` when the ability attempt should be suppressed this frame.
    /// Caller passes a value in [0.0, 1.0) from their RNG.
    /// Always returns `false` when the debuff is inactive.
    pub fn check_suppress(&self, rng_value: f32, dt: f32) -> bool {
        self.is_active() && rng_value < self.ability_suppress_chance * dt
    }

    /// Fraction of the debuff duration remaining [1.0 = just applied, 0.0 = cleared].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Concuss {
    fn default() -> Self {
        Self::new(std::f32::consts::FRAC_PI_8).with_suppress_chance(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_concuss() {
        let mut c = Concuss::new(0.5);
        c.apply(3.0);
        assert!(c.is_active());
        assert!(c.just_concussed);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut c = Concuss::new(0.5);
        c.apply(2.0);
        c.tick(0.016);
        c.apply(5.0);
        assert!((c.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut c = Concuss::new(0.5);
        c.apply(5.0);
        c.apply(2.0);
        assert!((c.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_concuss() {
        let mut c = Concuss::new(0.5);
        c.apply(1.0);
        c.tick(1.1);
        assert!(!c.is_active());
        assert!(c.just_cleared);
    }

    #[test]
    fn clear_ends_early() {
        let mut c = Concuss::new(0.5);
        c.apply(5.0);
        c.clear();
        assert!(!c.is_active());
        assert!(c.just_cleared);
    }

    #[test]
    fn check_suppress_triggers_when_rng_below_threshold() {
        let mut c = Concuss::new(0.5).with_suppress_chance(10.0);
        c.apply(3.0);
        // rng=0.0 < 10.0 * 0.016 = 0.16 → should suppress
        assert!(c.check_suppress(0.0, 0.016));
    }

    #[test]
    fn check_suppress_no_trigger_when_rng_above() {
        let mut c = Concuss::new(0.5).with_suppress_chance(1.0);
        c.apply(3.0);
        // rng=0.99 > 1.0 * 0.016 = 0.016 → no suppress
        assert!(!c.check_suppress(0.99, 0.016));
    }

    #[test]
    fn check_suppress_false_when_inactive() {
        let c = Concuss::new(0.5).with_suppress_chance(100.0);
        assert!(!c.check_suppress(0.0, 1.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut c = Concuss::new(0.5);
        c.apply(2.0);
        c.tick(1.0);
        assert!((c.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut c = Concuss::new(0.5);
        c.enabled = false;
        c.apply(5.0);
        assert!(!c.is_active());
    }

    #[test]
    fn tick_clears_just_concussed() {
        let mut c = Concuss::new(0.5);
        c.apply(3.0);
        c.tick(0.016);
        assert!(!c.just_concussed);
    }
}
