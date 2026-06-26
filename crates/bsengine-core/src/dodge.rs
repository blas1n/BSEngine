use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Phase of the dodge/evasion roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DodgePhase {
    /// No active dodge; ready to receive input.
    Idle,
    /// Actively rolling — movement applied and i-frames active.
    Rolling,
    /// Roll finished; waiting for cooldown before next dodge is available.
    Cooldown,
}

/// Dodge-roll / evasion component — short directional dash with invincibility frames.
///
/// The movement system reads `direction` and `speed` while `phase == Rolling` to apply
/// movement, and disables hitbox collision when `invincible == true`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Dodge {
    pub phase: DodgePhase,
    /// World-space roll direction (normalised by the movement system).
    pub direction: Vec3,
    /// Movement speed during the roll (m/s).
    pub speed: f32,
    /// Total duration of the active roll (seconds).
    pub duration: f32,
    /// Time remaining in the current phase (seconds).
    pub timer: f32,
    /// True during the active roll — movement system disables collision damage.
    pub invincible: bool,
    /// Seconds the character must wait before dodging again.
    pub cooldown: f32,
    /// True when the player is holding/pressing the dodge input.
    pub wants_dodge: bool,
    /// Whether the character can dodge while airborne.
    pub allow_airborne: bool,
    /// Consecutive dodge count this chain (reset on grounded idle).
    pub chain_count: u32,
    /// Maximum consecutive dodges before a forced cooldown (0 = unlimited).
    pub max_chain: u32,
    pub enabled: bool,
}

impl Dodge {
    pub fn new(speed: f32, duration: f32, cooldown: f32) -> Self {
        Self {
            phase: DodgePhase::Idle,
            direction: Vec3::ZERO,
            speed: speed.max(0.0),
            duration: duration.max(0.0),
            timer: 0.0,
            invincible: false,
            cooldown: cooldown.max(0.0),
            wants_dodge: false,
            allow_airborne: false,
            chain_count: 0,
            max_chain: 0,
            enabled: true,
        }
    }

    pub fn with_airborne(mut self) -> Self {
        self.allow_airborne = true;
        self
    }

    pub fn with_max_chain(mut self, n: u32) -> Self {
        self.max_chain = n;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Start a dodge in the given direction. Returns true if the dodge was accepted.
    pub fn start(&mut self, dir: Vec3) -> bool {
        if !self.enabled || self.phase != DodgePhase::Idle {
            return false;
        }
        if self.max_chain > 0 && self.chain_count >= self.max_chain {
            return false;
        }
        self.direction = dir.normalize_or_zero();
        self.phase = DodgePhase::Rolling;
        self.timer = self.duration;
        self.invincible = true;
        self.chain_count += 1;
        true
    }

    /// Advance the dodge state machine. Call every frame.
    pub fn tick(&mut self, dt: f32, is_grounded: bool) {
        if !self.enabled {
            return;
        }

        match self.phase {
            DodgePhase::Idle => {
                if is_grounded {
                    self.chain_count = 0;
                }
            }
            DodgePhase::Rolling => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.invincible = false;
                    self.phase = DodgePhase::Cooldown;
                    self.timer = self.cooldown;
                }
            }
            DodgePhase::Cooldown => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.timer = 0.0;
                    self.phase = DodgePhase::Idle;
                    if is_grounded {
                        self.chain_count = 0;
                    }
                }
            }
        }
    }

    pub fn is_rolling(&self) -> bool {
        self.phase == DodgePhase::Rolling
    }

    pub fn is_on_cooldown(&self) -> bool {
        self.phase == DodgePhase::Cooldown
    }

    pub fn is_available(&self) -> bool {
        self.enabled
            && self.phase == DodgePhase::Idle
            && (self.max_chain == 0 || self.chain_count < self.max_chain)
    }

    /// 0..1 cooldown progress (1 = fully recovered).
    pub fn cooldown_fraction(&self) -> f32 {
        if self.phase != DodgePhase::Cooldown || self.cooldown <= 0.0 {
            return 1.0;
        }
        1.0 - (self.timer / self.cooldown).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_transitions_to_rolling() {
        let mut d = Dodge::new(8.0, 0.3, 0.5);
        let ok = d.start(Vec3::X);
        assert!(ok);
        assert!(d.is_rolling());
        assert!(d.invincible);
    }

    #[test]
    fn tick_ends_roll_and_starts_cooldown() {
        let mut d = Dodge::new(8.0, 0.3, 0.5);
        d.start(Vec3::X);
        d.tick(0.4, true); // past roll duration
        assert!(d.is_on_cooldown());
        assert!(!d.invincible);
    }

    #[test]
    fn tick_ends_cooldown_and_returns_to_idle() {
        let mut d = Dodge::new(8.0, 0.3, 0.5);
        d.start(Vec3::X);
        d.tick(0.4, true); // into cooldown
        d.tick(0.6, true); // past cooldown
        assert_eq!(d.phase, DodgePhase::Idle);
        assert!(d.is_available());
    }

    #[test]
    fn start_rejected_while_rolling() {
        let mut d = Dodge::new(8.0, 0.3, 0.5);
        d.start(Vec3::X);
        let ok = d.start(Vec3::Y);
        assert!(!ok);
    }

    #[test]
    fn max_chain_blocks_extra_dodges() {
        let mut d = Dodge::new(8.0, 0.1, 0.1).with_max_chain(1);
        d.start(Vec3::X);
        d.tick(0.15, false); // cooldown
        d.tick(0.15, false); // idle but chain_count=1, not grounded
        assert!(!d.is_available());
    }

    #[test]
    fn grounded_idle_resets_chain() {
        let mut d = Dodge::new(8.0, 0.1, 0.1).with_max_chain(1);
        d.start(Vec3::X);
        d.tick(0.15, true); // cooldown, grounded
        d.tick(0.15, true); // idle, grounded → chain reset
        assert!(d.is_available());
    }
}
