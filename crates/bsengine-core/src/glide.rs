use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Current phase of the glide movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlidePhase {
    /// Not gliding; no forces applied.
    Idle,
    /// Actively gliding — lift and drag are applied by the movement system.
    Gliding,
    /// Stall: airspeed fell below `stall_speed`; the entity falls with increased drag.
    Stalling,
}

/// Winged-suit / hang-glider movement component.
///
/// The movement system queries `phase` and `glide_direction` each frame to apply lift.
/// Call `start()` when the entity jumps and the glide input is held; `tick(vel, dt)` each frame;
/// `end()` on landing or input release.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Glide {
    pub phase: GlidePhase,
    /// Normalised horizontal glide direction (set by input each frame).
    pub glide_direction: Vec3,
    /// Maximum descent speed while gliding (m/s downward, positive = downward).
    pub max_descent_speed: f32,
    /// Lift force applied per second to counteract gravity while gliding.
    pub lift: f32,
    /// Forward speed below which the glider stalls.
    pub stall_speed: f32,
    /// Current measured horizontal airspeed (written by movement system).
    pub airspeed: f32,
    /// Maximum horizontal speed while gliding.
    pub max_speed: f32,
    /// Whether the glide input is currently held.
    pub wants_glide: bool,
    /// Total time spent gliding this session (for stamina/fuel deduction).
    pub elapsed: f32,
    pub enabled: bool,
}

impl Glide {
    pub fn new(lift: f32, max_speed: f32) -> Self {
        Self {
            phase: GlidePhase::Idle,
            glide_direction: Vec3::NEG_Z,
            max_descent_speed: 3.0,
            lift: lift.max(0.0),
            stall_speed: 2.0,
            airspeed: 0.0,
            max_speed: max_speed.max(0.0),
            wants_glide: false,
            elapsed: 0.0,
            enabled: true,
        }
    }

    pub fn with_max_descent(mut self, speed: f32) -> Self {
        self.max_descent_speed = speed.max(0.0);
        self
    }

    pub fn with_stall_speed(mut self, speed: f32) -> Self {
        self.stall_speed = speed.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Begin gliding. No-op if disabled or already gliding.
    pub fn start(&mut self) {
        if self.enabled && self.phase == GlidePhase::Idle {
            self.phase = GlidePhase::Gliding;
            self.elapsed = 0.0;
        }
    }

    /// End the glide and return to Idle.
    pub fn end(&mut self) {
        self.phase = GlidePhase::Idle;
    }

    /// Advance the glide timer and update stall state.
    /// `horizontal_speed` is the current horizontal velocity magnitude.
    /// Returns `true` if just transitioned to Stalling.
    pub fn tick(&mut self, horizontal_speed: f32, dt: f32) -> bool {
        if !self.enabled || self.phase == GlidePhase::Idle {
            return false;
        }
        self.airspeed = horizontal_speed;
        self.elapsed += dt;
        let was_stalling = self.phase == GlidePhase::Stalling;
        self.phase = if horizontal_speed < self.stall_speed {
            GlidePhase::Stalling
        } else {
            GlidePhase::Gliding
        };
        !was_stalling && self.phase == GlidePhase::Stalling
    }

    pub fn is_active(&self) -> bool {
        self.phase != GlidePhase::Idle
    }

    pub fn is_stalling(&self) -> bool {
        self.phase == GlidePhase::Stalling
    }

    /// Effective lift this frame — zero when stalling.
    pub fn effective_lift(&self) -> f32 {
        if self.phase == GlidePhase::Gliding {
            self.lift
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_enters_gliding_phase() {
        let mut g = Glide::new(9.8, 15.0);
        g.start();
        assert_eq!(g.phase, GlidePhase::Gliding);
    }

    #[test]
    fn end_returns_to_idle() {
        let mut g = Glide::new(9.8, 15.0);
        g.start();
        g.end();
        assert!(!g.is_active());
    }

    #[test]
    fn tick_transitions_to_stalling() {
        let mut g = Glide::new(9.8, 15.0).with_stall_speed(5.0);
        g.start();
        let stalled = g.tick(3.0, 0.016);
        assert!(stalled);
        assert!(g.is_stalling());
    }

    #[test]
    fn effective_lift_zero_when_stalling() {
        let mut g = Glide::new(9.8, 15.0).with_stall_speed(5.0);
        g.start();
        g.tick(1.0, 0.016);
        assert!((g.effective_lift()).abs() < 0.001);
    }

    #[test]
    fn effective_lift_nonzero_when_gliding() {
        let mut g = Glide::new(9.8, 15.0).with_stall_speed(5.0);
        g.start();
        g.tick(10.0, 0.016);
        assert!((g.effective_lift() - 9.8).abs() < 0.001);
    }
}
