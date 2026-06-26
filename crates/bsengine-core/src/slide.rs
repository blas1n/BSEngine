use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Phase of the character slide movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlidePhase {
    /// Not sliding.
    Idle,
    /// Building up and moving at full slide speed.
    Sliding,
    /// Deceleration at the end of the slide.
    Braking,
}

/// Character slide / prone-dash movement component.
///
/// The character controller checks `wants_slide` each frame.
/// Call `start(direction)` to begin; `tick(dt)` advances the timer and
/// transitions through Sliding → Braking → Idle automatically.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Slide {
    pub phase: SlidePhase,
    /// Direction the slide was initiated in (normalised).
    pub direction: Vec3,
    /// Total slide duration in seconds.
    pub duration: f32,
    /// Time at which the slide transitions to Braking.
    pub brake_start: f32,
    /// Time elapsed in the current slide.
    pub elapsed: f32,
    /// Speed multiplier applied to the character during Sliding phase.
    pub slide_speed: f32,
    /// Whether the input requested a slide this frame.
    pub wants_slide: bool,
    /// Height scale while sliding (for hitbox reduction).
    pub crouch_scale: f32,
    pub enabled: bool,
}

impl Slide {
    pub fn new(duration: f32, slide_speed: f32) -> Self {
        Self {
            phase: SlidePhase::Idle,
            direction: Vec3::NEG_Z,
            duration: duration.max(0.01),
            brake_start: (duration * 0.7).max(0.01),
            elapsed: 0.0,
            slide_speed: slide_speed.max(0.0),
            wants_slide: false,
            crouch_scale: 0.5,
            enabled: true,
        }
    }

    pub fn with_crouch_scale(mut self, scale: f32) -> Self {
        self.crouch_scale = scale.clamp(0.1, 1.0);
        self
    }

    pub fn with_brake_start(mut self, t: f32) -> Self {
        self.brake_start = t.clamp(0.0, self.duration);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Begin a slide in `direction`. No-op if already sliding or disabled.
    pub fn start(&mut self, direction: Vec3) {
        if !self.enabled || self.phase != SlidePhase::Idle {
            return;
        }
        self.direction = direction.normalize_or_zero();
        self.elapsed = 0.0;
        self.phase = SlidePhase::Sliding;
    }

    /// Cancel a slide and return to Idle immediately.
    pub fn cancel(&mut self) {
        self.phase = SlidePhase::Idle;
        self.elapsed = 0.0;
    }

    /// Advance the slide timer. Returns `true` when the slide ends this tick.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled || self.phase == SlidePhase::Idle {
            return false;
        }
        self.elapsed += dt;
        if self.phase == SlidePhase::Sliding && self.elapsed >= self.brake_start {
            self.phase = SlidePhase::Braking;
        }
        if self.elapsed >= self.duration {
            self.phase = SlidePhase::Idle;
            self.elapsed = 0.0;
            return true;
        }
        false
    }

    pub fn is_active(&self) -> bool {
        self.phase != SlidePhase::Idle
    }

    /// Speed fraction [0, 1] — full during Sliding, linearly decreasing during Braking.
    pub fn speed_fraction(&self) -> f32 {
        match self.phase {
            SlidePhase::Idle => 0.0,
            SlidePhase::Sliding => 1.0,
            SlidePhase::Braking => {
                let brake_duration = self.duration - self.brake_start;
                if brake_duration <= 0.0 {
                    return 0.0;
                }
                let progress = (self.elapsed - self.brake_start) / brake_duration;
                (1.0 - progress).clamp(0.0, 1.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_enters_sliding_phase() {
        let mut s = Slide::new(1.0, 5.0);
        s.start(Vec3::X);
        assert_eq!(s.phase, SlidePhase::Sliding);
    }

    #[test]
    fn tick_transitions_to_braking_then_idle() {
        let mut s = Slide::new(1.0, 5.0).with_brake_start(0.6);
        s.start(Vec3::X);
        s.tick(0.7);
        assert_eq!(s.phase, SlidePhase::Braking);
        let ended = s.tick(0.4);
        assert!(ended);
        assert_eq!(s.phase, SlidePhase::Idle);
    }

    #[test]
    fn cancel_returns_to_idle() {
        let mut s = Slide::new(1.0, 5.0);
        s.start(Vec3::X);
        s.cancel();
        assert!(!s.is_active());
    }

    #[test]
    fn speed_fraction_full_while_sliding() {
        let mut s = Slide::new(1.0, 5.0);
        s.start(Vec3::X);
        assert!((s.speed_fraction() - 1.0).abs() < 0.001);
    }

    #[test]
    fn speed_fraction_zero_when_idle() {
        let s = Slide::new(1.0, 5.0);
        assert!((s.speed_fraction()).abs() < 0.001);
    }
}
