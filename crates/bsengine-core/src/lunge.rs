use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Phase of a forward lunge attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LungePhase {
    /// Ready to execute. Accepts input.
    Idle,
    /// Flying forward toward the target point.
    Thrusting,
    /// Post-lunge freeze before the character can act again.
    Recovery,
    /// Waiting before the next lunge is permitted.
    Cooldown,
}

/// Forward-thrust attack component — closes the gap to a target rapidly.
///
/// Unlike `Dash` (which is purely evasive), `Lunge` is an offensive gap-closer
/// that moves the character toward a designated world-space point. The combat
/// system populates `target_point` and calls `begin()`. Each frame it calls
/// `tick(dt)` and moves the entity along `direction` by `speed * dt` until
/// `traveled >= range`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Lunge {
    pub phase: LungePhase,
    /// Normalised direction of the lunge (set by `begin()`).
    pub direction: Vec3,
    /// World-space point to lunge toward (set by caller before `begin()`).
    pub target_point: Vec3,
    /// Movement speed during the thrust (m/s).
    pub speed: f32,
    /// Maximum lunge distance (metres). Lunge ends when `traveled >= range`.
    pub range: f32,
    /// Distance covered so far this lunge.
    pub traveled: f32,
    /// Duration of the recovery phase after a lunge completes (seconds).
    pub recovery_time: f32,
    pub recovery_timer: f32,
    /// How long before the next lunge can begin (seconds).
    pub cooldown: f32,
    pub cooldown_timer: f32,
    /// If true, `begin()` is rejected when the character is airborne.
    pub ground_only: bool,
    /// Set to true by `begin()`; cleared by `tick()` the next frame.
    /// Combat system can read this to spawn a hit-check on the leading frame.
    pub just_lunged: bool,
    /// True when at least one hit has been registered for this lunge.
    pub hit_registered: bool,
    pub enabled: bool,
}

impl Lunge {
    pub fn new(speed: f32, range: f32, recovery_time: f32, cooldown: f32) -> Self {
        Self {
            phase: LungePhase::Idle,
            direction: Vec3::Z,
            target_point: Vec3::ZERO,
            speed: speed.max(0.0),
            range: range.max(0.0),
            traveled: 0.0,
            recovery_time: recovery_time.max(0.0),
            recovery_timer: 0.0,
            cooldown: cooldown.max(0.0),
            cooldown_timer: 0.0,
            ground_only: false,
            just_lunged: false,
            hit_registered: false,
            enabled: true,
        }
    }

    pub fn ground_only(mut self) -> Self {
        self.ground_only = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Begin a lunge toward `target`. Returns true if accepted.
    pub fn begin(&mut self, origin: Vec3, target: Vec3, is_grounded: bool) -> bool {
        if !self.enabled {
            return false;
        }
        if self.phase != LungePhase::Idle {
            return false;
        }
        if self.ground_only && !is_grounded {
            return false;
        }
        let delta = target - origin;
        if delta.length_squared() < 1e-6 {
            return false;
        }
        self.target_point = target;
        self.direction = delta.normalize();
        self.phase = LungePhase::Thrusting;
        self.traveled = 0.0;
        self.hit_registered = false;
        self.just_lunged = true;
        true
    }

    /// Advance the lunge state machine. `dt` is frame delta-time in seconds.
    /// Returns the displacement vector the caller should apply this frame.
    pub fn tick(&mut self, dt: f32) -> Vec3 {
        self.just_lunged = false;

        if !self.enabled {
            return Vec3::ZERO;
        }

        match self.phase {
            LungePhase::Idle => Vec3::ZERO,
            LungePhase::Thrusting => {
                let step = self.speed * dt;
                let remaining = self.range - self.traveled;
                let actual = step.min(remaining);
                self.traveled += actual;
                let displacement = self.direction * actual;
                if self.traveled >= self.range {
                    self.phase = LungePhase::Recovery;
                    self.recovery_timer = self.recovery_time;
                }
                displacement
            }
            LungePhase::Recovery => {
                self.recovery_timer -= dt;
                if self.recovery_timer <= 0.0 {
                    self.phase = LungePhase::Cooldown;
                    self.cooldown_timer = self.cooldown;
                }
                Vec3::ZERO
            }
            LungePhase::Cooldown => {
                self.cooldown_timer -= dt;
                if self.cooldown_timer <= 0.0 {
                    self.phase = LungePhase::Idle;
                }
                Vec3::ZERO
            }
        }
    }

    pub fn is_thrusting(&self) -> bool {
        self.phase == LungePhase::Thrusting
    }

    pub fn is_available(&self) -> bool {
        self.phase == LungePhase::Idle
    }

    /// Fraction of the lunge distance traveled (0.0–1.0).
    pub fn progress(&self) -> f32 {
        if self.range > 0.0 {
            (self.traveled / self.range).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn cooldown_fraction(&self) -> f32 {
        if self.cooldown > 0.0 {
            (self.cooldown_timer / self.cooldown).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_sets_thrusting_phase() {
        let mut l = Lunge::new(10.0, 5.0, 0.2, 0.5);
        let ok = l.begin(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), true);
        assert!(ok);
        assert_eq!(l.phase, LungePhase::Thrusting);
    }

    #[test]
    fn tick_advances_position_and_completes() {
        let mut l = Lunge::new(10.0, 5.0, 0.0, 0.5);
        l.begin(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), true);
        let d = l.tick(0.5); // 5.0 m in 0.5 s = range reached; enters Recovery
        assert!(d.z > 0.0);
        l.tick(0.0); // 0-duration recovery_timer (0.0 - 0.0 <= 0) → Cooldown
        assert_eq!(l.phase, LungePhase::Cooldown);
    }

    #[test]
    fn begin_rejected_when_not_idle() {
        let mut l = Lunge::new(10.0, 5.0, 0.2, 0.5);
        l.begin(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), true);
        let ok = l.begin(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), true);
        assert!(!ok);
    }

    #[test]
    fn progress_tracks_distance() {
        let mut l = Lunge::new(10.0, 10.0, 0.0, 0.0);
        l.begin(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0), true);
        l.tick(0.5); // 5 m = 50%
        assert!((l.progress() - 0.5).abs() < 0.01);
    }

    #[test]
    fn ground_only_rejects_airborne() {
        let mut l = Lunge::new(10.0, 5.0, 0.2, 0.5).ground_only();
        let ok = l.begin(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), false);
        assert!(!ok);
    }

    #[test]
    fn full_cycle_returns_to_idle() {
        let mut l = Lunge::new(10.0, 1.0, 0.1, 0.1);
        l.begin(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), true);
        l.tick(0.2); // completes thrust + enters recovery
        l.tick(0.15); // finishes recovery, enters cooldown
        l.tick(0.15); // finishes cooldown
        assert_eq!(l.phase, LungePhase::Idle);
    }
}
