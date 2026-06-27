use bevy_ecs::prelude::Component;

/// Allows an entity to temporarily pass through solid colliders (phase-shift).
///
/// When `is_phased` is true, the physics/collision system should ignore
/// solid-collision responses for this entity. Use it for abilities like
/// "walk through walls for 2 seconds" or boss invulnerability windows.
///
/// The phase has an optional `cooldown` that prevents re-activation immediately
/// after the effect ends. `tick(dt)` advances both timers and clears flags.
///
/// Unlike `Ghost` (which also handles visibility/spectator state), `Phase` is
/// purely about collision bypass with timed activation.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Phase {
    /// True while the entity is phased through solid colliders.
    pub is_phased: bool,
    /// How long a single phase lasts.
    pub duration: f32,
    /// Remaining phase time.
    pub timer: f32,
    /// Time the entity must wait before phasing again (0 = no cooldown).
    pub cooldown: f32,
    /// Remaining cooldown time.
    pub cooldown_timer: f32,
    pub just_phased: bool,
    pub just_unphased: bool,
    pub enabled: bool,
}

impl Phase {
    pub fn new(duration: f32) -> Self {
        Self {
            is_phased: false,
            duration: duration.max(0.0),
            timer: 0.0,
            cooldown: 0.0,
            cooldown_timer: 0.0,
            just_phased: false,
            just_unphased: false,
            enabled: true,
        }
    }

    pub fn with_cooldown(mut self, cooldown: f32) -> Self {
        self.cooldown = cooldown.max(0.0);
        self
    }

    /// Attempt to activate phase. Returns true if successful.
    /// Fails if disabled, already phased, or still on cooldown.
    pub fn activate(&mut self) -> bool {
        if !self.enabled || self.is_phased || self.cooldown_timer > 0.0 {
            return false;
        }

        self.is_phased = true;
        self.timer = self.duration;
        self.just_phased = true;
        true
    }

    /// Force-end a phase early.
    pub fn cancel(&mut self) {
        if self.is_phased {
            self.is_phased = false;
            self.timer = 0.0;
            self.cooldown_timer = self.cooldown;
            self.just_unphased = true;
        }
    }

    /// Advance timers. Expires the phase and starts the cooldown when the timer runs out.
    pub fn tick(&mut self, dt: f32) {
        self.just_phased = false;
        self.just_unphased = false;

        if self.is_phased {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.is_phased = false;
                self.timer = 0.0;
                self.cooldown_timer = self.cooldown;
                self.just_unphased = true;
            }
        } else if self.cooldown_timer > 0.0 {
            self.cooldown_timer -= dt;
            if self.cooldown_timer < 0.0 {
                self.cooldown_timer = 0.0;
            }
        }
    }

    pub fn is_ready(&self) -> bool {
        !self.is_phased && self.cooldown_timer <= 0.0 && self.enabled
    }

    /// Fraction of the current phase remaining [1.0 = just started, 0.0 = expired].
    pub fn phase_fraction(&self) -> f32 {
        if self.duration <= 0.0 || !self.is_phased {
            return 0.0;
        }
        self.timer / self.duration
    }

    /// Fraction of the cooldown remaining [1.0 = just entered cooldown, 0.0 = ready].
    pub fn cooldown_fraction(&self) -> f32 {
        if self.cooldown <= 0.0 {
            return 0.0;
        }
        self.cooldown_timer / self.cooldown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_starts_phase() {
        let mut p = Phase::new(2.0);
        let ok = p.activate();
        assert!(ok);
        assert!(p.is_phased);
        assert!(p.just_phased);
    }

    #[test]
    fn activate_fails_when_already_phased() {
        let mut p = Phase::new(2.0);
        p.activate();
        let ok = p.activate();
        assert!(!ok);
    }

    #[test]
    fn tick_expires_phase() {
        let mut p = Phase::new(1.0);
        p.activate();
        p.tick(1.1);
        assert!(!p.is_phased);
        assert!(p.just_unphased);
    }

    #[test]
    fn tick_starts_cooldown_after_expiry() {
        let mut p = Phase::new(1.0).with_cooldown(2.0);
        p.activate();
        p.tick(1.1); // phase expires → cooldown_timer set to 2.0
        assert!(p.cooldown_timer > 0.0);
        assert!(!p.is_ready());
    }

    #[test]
    fn activate_blocked_during_cooldown() {
        let mut p = Phase::new(1.0).with_cooldown(2.0);
        p.activate();
        p.tick(1.1); // expires, cooldown starts
        let ok = p.activate();
        assert!(!ok);
    }

    #[test]
    fn cooldown_ticks_down() {
        let mut p = Phase::new(1.0).with_cooldown(2.0);
        p.activate();
        p.tick(1.1); // expires
        p.tick(2.1); // cooldown done
        assert!(p.is_ready());
    }

    #[test]
    fn cancel_ends_phase_early() {
        let mut p = Phase::new(5.0);
        p.activate();
        p.cancel();
        assert!(!p.is_phased);
        assert!(p.just_unphased);
    }

    #[test]
    fn phase_fraction_during_phase() {
        let mut p = Phase::new(2.0);
        p.activate();
        p.tick(1.0); // half elapsed
        let frac = p.phase_fraction();
        assert!((frac - 0.5).abs() < 1e-4);
    }

    #[test]
    fn disabled_activate_no_op() {
        let mut p = Phase::new(2.0);
        p.enabled = false;
        let ok = p.activate();
        assert!(!ok);
        assert!(!p.is_phased);
    }
}
