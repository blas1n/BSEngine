use bevy_ecs::prelude::Component;

/// Temporal decay debuff: the entity's effectiveness diminishes linearly from
/// full potency to `min_potency` over the wane's duration.
///
/// `current_potency()` starts at `1.0` when `start(duration)` is called and
/// falls toward `min_potency` as the timer runs down. Systems multiply their
/// computed stats by `current_potency()` to apply the fade.
///
/// `stop()` ends the wane early and resets potency. `tick(dt)` counts down
/// and sets `just_expired` when the duration elapses naturally.
///
/// Distinct from `Weaken` (fixed outgoing-damage multiplier), `Wither` (max-
/// health ceiling reduction), and `Demoralize` (attack-speed penalty): Wane
/// is a _continuous_ linear decay — it models a buff that runs out of energy,
/// a fire dying down, or a shield failing over time.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wane {
    pub duration: f32,
    pub timer: f32,
    /// Potency floor the wane decays toward. Clamped to [0.0, 1.0].
    pub min_potency: f32,
    pub just_waning: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Wane {
    pub fn new(min_potency: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            min_potency: min_potency.clamp(0.0, 1.0),
            just_waning: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Begin the wane effect for `duration` seconds, resetting any previous
    /// wane. No-op when disabled.
    pub fn start(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_waning = true;
    }

    /// End the wane immediately, restoring full potency.
    pub fn stop(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_expired = true;
        }
    }

    /// Advance the timer; sets `just_expired` when the wane runs out.
    pub fn tick(&mut self, dt: f32) {
        self.just_waning = false;
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

    /// Current potency in [min_potency, 1.0]. Returns `1.0` when not active.
    /// Decays linearly from `1.0` at full duration to `min_potency` at expiry.
    pub fn current_potency(&self) -> f32 {
        if !self.is_active() || self.duration <= 0.0 {
            return 1.0;
        }
        let frac = (self.timer / self.duration).clamp(0.0, 1.0);
        self.min_potency + frac * (1.0 - self.min_potency)
    }

    /// Fraction of the wane duration remaining [1.0 = just started, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Wane {
    fn default() -> Self {
        Self::new(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_activates_wane() {
        let mut w = Wane::new(0.0);
        w.start(3.0);
        assert!(w.is_active());
        assert!(w.just_waning);
    }

    #[test]
    fn potency_at_full_when_just_started() {
        let mut w = Wane::new(0.0);
        w.start(4.0);
        assert!((w.current_potency() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn potency_at_half_when_midway() {
        let mut w = Wane::new(0.0);
        w.start(4.0);
        w.tick(2.0);
        assert!((w.current_potency() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn potency_reaches_min_at_expiry_edge() {
        let mut w = Wane::new(0.2);
        w.start(1.0);
        w.tick(0.9999);
        assert!(w.current_potency() < 0.25); // very close to min 0.2
    }

    #[test]
    fn potency_one_when_not_active() {
        let w = Wane::new(0.0);
        assert!((w.current_potency() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_expires_wane() {
        let mut w = Wane::new(0.0);
        w.start(1.0);
        w.tick(1.1);
        assert!(!w.is_active());
        assert!(w.just_expired);
    }

    #[test]
    fn stop_ends_early() {
        let mut w = Wane::new(0.0);
        w.start(5.0);
        w.stop();
        assert!(!w.is_active());
        assert!(w.just_expired);
        assert!((w.current_potency() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn min_potency_is_floor() {
        let mut w = Wane::new(0.4);
        w.start(1.0);
        // The potency should always be >= min_potency while active
        w.tick(0.5);
        assert!(w.current_potency() >= 0.4 - 1e-3);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut w = Wane::new(0.0);
        w.start(2.0);
        w.tick(1.0);
        assert!((w.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_start_no_op() {
        let mut w = Wane::new(0.0);
        w.enabled = false;
        w.start(5.0);
        assert!(!w.is_active());
    }

    #[test]
    fn tick_clears_just_waning() {
        let mut w = Wane::new(0.0);
        w.start(3.0);
        w.tick(0.016);
        assert!(!w.just_waning);
    }
}
