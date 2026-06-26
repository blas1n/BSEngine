use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Current wall-interaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallState {
    /// Not touching a wall.
    None,
    /// Running along a wall surface.
    Running,
    /// Sliding down a wall (gravity partially reduced).
    Sliding,
    /// Jumping off a wall this frame.
    Jumping,
}

/// Wall-run / wall-slide / wall-jump component for platformer characters.
///
/// The movement system detects wall contacts, writes `wall_normal` and `wall_entity`,
/// then calls the appropriate method. Physics reads `gravity_scale` while attached.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wall {
    pub state: WallState,
    /// Outward normal of the wall surface (written by the physics/movement system).
    pub wall_normal: Vec3,
    /// Movement speed while running along the wall (m/s).
    pub run_speed: f32,
    /// Downward slide speed while attached but not running (m/s).
    pub slide_speed: f32,
    /// Fraction of normal gravity applied while wall-running (0 = none, 1 = full).
    pub gravity_scale: f32,
    /// Max time a continuous wall-run can last before the character falls off (0 = unlimited).
    pub max_run_duration: f32,
    /// Time remaining in the current wall-run.
    pub run_timer: f32,
    /// Jump impulse magnitude when jumping off the wall.
    pub wall_jump_force: f32,
    /// Mix of away-from-wall (x) and upward (y) impulse directions [0..1].
    pub wall_jump_lateral_fraction: f32,
    /// Cooldown before the same wall can be grabbed again (prevents infinite wall-hopping).
    pub reattach_cooldown: f32,
    /// Remaining reattach cooldown timer.
    pub reattach_timer: f32,
    pub enabled: bool,
}

impl Wall {
    pub fn new(run_speed: f32, slide_speed: f32, wall_jump_force: f32) -> Self {
        Self {
            state: WallState::None,
            wall_normal: Vec3::ZERO,
            run_speed: run_speed.max(0.0),
            slide_speed: slide_speed.max(0.0),
            gravity_scale: 0.1,
            max_run_duration: 0.0,
            run_timer: 0.0,
            wall_jump_force: wall_jump_force.max(0.0),
            wall_jump_lateral_fraction: 0.5,
            reattach_cooldown: 0.5,
            reattach_timer: 0.0,
            enabled: true,
        }
    }

    pub fn with_gravity_scale(mut self, s: f32) -> Self {
        self.gravity_scale = s.clamp(0.0, 1.0);
        self
    }

    pub fn with_max_run_duration(mut self, secs: f32) -> Self {
        self.max_run_duration = secs.max(0.0);
        self
    }

    pub fn with_reattach_cooldown(mut self, secs: f32) -> Self {
        self.reattach_cooldown = secs.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Attach to a wall for running. Returns true if accepted.
    pub fn attach(&mut self, normal: Vec3) -> bool {
        if !self.enabled || self.reattach_timer > 0.0 {
            return false;
        }
        self.wall_normal = normal.normalize_or_zero();
        self.state = WallState::Running;
        self.run_timer = self.max_run_duration;
        true
    }

    /// Transition from running to sliding (input released or run expired).
    pub fn begin_slide(&mut self) {
        if self.state == WallState::Running {
            self.state = WallState::Sliding;
        }
    }

    /// Compute the wall-jump impulse vector and detach.
    /// Returns the impulse; movement system adds it to the rigidbody.
    pub fn jump(&mut self) -> Vec3 {
        if !self.enabled || (self.state != WallState::Running && self.state != WallState::Sliding) {
            return Vec3::ZERO;
        }
        let lat = self.wall_normal * self.wall_jump_lateral_fraction;
        let up = Vec3::Y * (1.0 - self.wall_jump_lateral_fraction);
        let impulse = (lat + up).normalize_or_zero() * self.wall_jump_force;
        self.state = WallState::Jumping;
        self.reattach_timer = self.reattach_cooldown;
        impulse
    }

    /// Advance timers and handle run expiry. Call every frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }

        if self.reattach_timer > 0.0 {
            self.reattach_timer = (self.reattach_timer - dt).max(0.0);
        }

        if self.state == WallState::Running && self.max_run_duration > 0.0 {
            self.run_timer -= dt;
            if self.run_timer <= 0.0 {
                self.begin_slide();
            }
        }
    }

    /// Detach from the wall (landed or moved away).
    pub fn detach(&mut self) {
        self.state = WallState::None;
        self.wall_normal = Vec3::ZERO;
    }

    pub fn is_attached(&self) -> bool {
        matches!(self.state, WallState::Running | WallState::Sliding)
    }

    /// Fraction of run time remaining (1 = full, 0 = expired). Returns 1 if unlimited.
    pub fn run_fraction(&self) -> f32 {
        if self.max_run_duration <= 0.0 {
            return 1.0;
        }
        (self.run_timer / self.max_run_duration).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_sets_running_state() {
        let mut w = Wall::new(8.0, 2.0, 12.0);
        let ok = w.attach(-Vec3::X);
        assert!(ok);
        assert_eq!(w.state, WallState::Running);
    }

    #[test]
    fn begin_slide_transitions_from_running() {
        let mut w = Wall::new(8.0, 2.0, 12.0);
        w.attach(-Vec3::X);
        w.begin_slide();
        assert_eq!(w.state, WallState::Sliding);
    }

    #[test]
    fn jump_returns_impulse_and_detaches() {
        let mut w = Wall::new(8.0, 2.0, 12.0);
        w.attach(-Vec3::X);
        let impulse = w.jump();
        assert!(impulse.length() > 0.0);
        assert_eq!(w.state, WallState::Jumping);
        assert!(w.reattach_timer > 0.0);
    }

    #[test]
    fn run_expires_after_max_duration() {
        let mut w = Wall::new(8.0, 2.0, 12.0).with_max_run_duration(0.3);
        w.attach(-Vec3::X);
        w.tick(0.4);
        assert_eq!(w.state, WallState::Sliding);
    }

    #[test]
    fn reattach_blocked_during_cooldown() {
        let mut w = Wall::new(8.0, 2.0, 12.0).with_reattach_cooldown(0.5);
        w.attach(-Vec3::X);
        w.jump();
        let ok = w.attach(-Vec3::X); // should be blocked
        assert!(!ok);
    }

    #[test]
    fn reattach_allowed_after_cooldown_expires() {
        let mut w = Wall::new(8.0, 2.0, 12.0).with_reattach_cooldown(0.2);
        w.attach(-Vec3::X);
        w.jump();
        w.tick(0.3);
        let ok = w.attach(-Vec3::X);
        assert!(ok);
    }
}
