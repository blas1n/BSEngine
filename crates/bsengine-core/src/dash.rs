use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Current phase of a dash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashPhase {
    /// No dash in progress; cooldown may still be ticking.
    Ready,
    /// Actively dashing — entity moves at `speed` along `direction`.
    Active,
    /// Dash finished; in post-dash recovery (e.g., brief disable of air control).
    Recovery,
}

/// Short directional burst of movement.
///
/// The movement system reads `phase`, `direction`, and `speed` each frame.
/// Call `trigger(dir)` to start a dash; call `tick(dt)` to advance the state machine.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Dash {
    pub phase: DashPhase,
    /// Direction the dash travels (should be unit-length when active).
    pub direction: Vec3,
    /// Movement speed while `Active`, in units/second.
    pub speed: f32,
    /// How long the active dash lasts, in seconds.
    pub duration: f32,
    /// Time spent in the current phase.
    pub phase_timer: f32,
    /// Post-dash recovery window in seconds. 0.0 = no recovery phase.
    pub recovery_duration: f32,
    /// How long between dashes in seconds.
    pub cooldown: f32,
    /// Remaining cooldown before the next dash is available.
    pub cooldown_timer: f32,
    /// Maximum consecutive dashes before a full cooldown is required.
    pub max_charges: u32,
    pub charges: u32,
    pub enabled: bool,
    /// Whether the entity is invincible while dashing.
    pub invincible_during_dash: bool,
}

impl Dash {
    pub fn new(speed: f32, duration: f32) -> Self {
        Self {
            phase: DashPhase::Ready,
            direction: Vec3::ZERO,
            speed: speed.max(0.0),
            duration: duration.max(0.0),
            phase_timer: 0.0,
            recovery_duration: 0.0,
            cooldown: 1.0,
            cooldown_timer: 0.0,
            max_charges: 1,
            charges: 1,
            enabled: true,
            invincible_during_dash: false,
        }
    }

    pub fn with_cooldown(mut self, seconds: f32) -> Self {
        self.cooldown = seconds.max(0.0);
        self
    }

    pub fn with_recovery(mut self, seconds: f32) -> Self {
        self.recovery_duration = seconds.max(0.0);
        self
    }

    pub fn with_charges(mut self, charges: u32) -> Self {
        self.max_charges = charges.max(1);
        self.charges = self.max_charges;
        self
    }

    pub fn invincible(mut self) -> Self {
        self.invincible_during_dash = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Try to start a dash in the given direction.
    /// Returns `false` if on cooldown, no charges, already dashing, or disabled.
    pub fn trigger(&mut self, direction: Vec3) -> bool {
        if !self.enabled || self.charges == 0 || self.phase != DashPhase::Ready {
            return false;
        }
        self.direction = direction.normalize_or_zero();
        self.phase = DashPhase::Active;
        self.phase_timer = 0.0;
        self.charges -= 1;
        if self.charges == 0 {
            self.cooldown_timer = self.cooldown;
        }
        true
    }

    /// Advance the dash state machine. Returns `true` when a dash completes (exits Active).
    pub fn tick(&mut self, dt: f32) -> bool {
        // Recharge cooldown.
        if self.cooldown_timer > 0.0 {
            self.cooldown_timer -= dt;
            if self.cooldown_timer <= 0.0 {
                self.cooldown_timer = 0.0;
                self.charges = self.max_charges;
            }
        }

        match self.phase {
            DashPhase::Active => {
                self.phase_timer += dt;
                if self.phase_timer >= self.duration {
                    if self.recovery_duration > 0.0 {
                        self.phase = DashPhase::Recovery;
                        self.phase_timer = 0.0;
                    } else {
                        self.phase = DashPhase::Ready;
                        self.direction = Vec3::ZERO;
                    }
                    return true;
                }
            }
            DashPhase::Recovery => {
                self.phase_timer += dt;
                if self.phase_timer >= self.recovery_duration {
                    self.phase = DashPhase::Ready;
                    self.direction = Vec3::ZERO;
                    self.phase_timer = 0.0;
                }
            }
            DashPhase::Ready => {}
        }
        false
    }

    pub fn is_active(&self) -> bool {
        self.phase == DashPhase::Active
    }

    pub fn is_invincible(&self) -> bool {
        self.invincible_during_dash && self.phase == DashPhase::Active
    }

    pub fn can_dash(&self) -> bool {
        self.enabled && self.charges > 0 && self.phase == DashPhase::Ready
    }
}

impl Default for Dash {
    fn default() -> Self {
        Self::new(10.0, 0.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dash_triggers_and_becomes_active() {
        let mut d = Dash::new(10.0, 0.2);
        assert!(d.trigger(Vec3::X));
        assert_eq!(d.phase, DashPhase::Active);
        assert_eq!(d.direction, Vec3::X);
    }

    #[test]
    fn dash_completes_after_duration() {
        let mut d = Dash::new(10.0, 0.2);
        d.trigger(Vec3::X);
        let finished = d.tick(0.3);
        assert!(finished);
        assert_eq!(d.phase, DashPhase::Ready);
    }

    #[test]
    fn dash_enters_recovery_phase() {
        let mut d = Dash::new(10.0, 0.2).with_recovery(0.1);
        d.trigger(Vec3::X);
        let finished = d.tick(0.3);
        assert!(finished);
        assert_eq!(d.phase, DashPhase::Recovery);
        d.tick(0.2);
        assert_eq!(d.phase, DashPhase::Ready);
    }

    #[test]
    fn dash_cooldown_refills_charges() {
        let mut d = Dash::new(10.0, 0.1).with_cooldown(0.5).with_charges(1);
        d.trigger(Vec3::X);
        d.tick(0.2);
        assert_eq!(d.charges, 0);
        d.tick(0.5);
        assert_eq!(d.charges, 1);
    }

    #[test]
    fn dash_disabled_cannot_trigger() {
        let mut d = Dash::new(10.0, 0.2).disabled();
        assert!(!d.trigger(Vec3::X));
        assert_eq!(d.phase, DashPhase::Ready);
    }
}
