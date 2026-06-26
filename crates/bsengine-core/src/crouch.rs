use bevy_ecs::prelude::Component;

/// Transition state of the crouch action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrouchPhase {
    Standing,
    CrouchingDown,
    Crouched,
    StandingUp,
}

/// Crouch and prone state for a character entity.
///
/// The movement system reads `is_crouched` / `phase` to choose the appropriate
/// capsule height and speed multiplier. Call `tick(dt)` each frame to advance
/// the crouch transition.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Crouch {
    pub phase: CrouchPhase,
    /// Input latch: true while the crouch button is held.
    pub wants_crouch: bool,
    /// Seconds for the full crouch-down or stand-up transition.
    pub transition_duration: f32,
    /// Progress through the current transition in [0, 1].
    pub transition_progress: f32,
    /// Multiplier applied to movement speed while crouched. Default 0.5.
    pub speed_multiplier: f32,
    /// Height reduction factor for the character capsule. Default 0.6 = 60% of standing height.
    pub height_scale: f32,
    /// If true, toggle on press rather than hold.
    pub toggle_mode: bool,
    pub enabled: bool,
}

impl Crouch {
    pub fn new() -> Self {
        Self {
            phase: CrouchPhase::Standing,
            wants_crouch: false,
            transition_duration: 0.15,
            transition_progress: 0.0,
            speed_multiplier: 0.5,
            height_scale: 0.6,
            toggle_mode: false,
            enabled: true,
        }
    }

    pub fn with_transition_duration(mut self, seconds: f32) -> Self {
        self.transition_duration = seconds.max(0.01);
        self
    }

    pub fn with_speed_multiplier(mut self, factor: f32) -> Self {
        self.speed_multiplier = factor.clamp(0.0, 1.0);
        self
    }

    pub fn with_height_scale(mut self, scale: f32) -> Self {
        self.height_scale = scale.clamp(0.1, 1.0);
        self
    }

    pub fn toggle(mut self) -> Self {
        self.toggle_mode = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Request crouch start.
    pub fn press(&mut self) {
        if !self.enabled {
            return;
        }
        if self.toggle_mode {
            self.wants_crouch = !self.wants_crouch;
        } else {
            self.wants_crouch = true;
        }
    }

    /// Release crouch (hold mode only; no-op in toggle mode).
    pub fn release(&mut self) {
        if !self.toggle_mode {
            self.wants_crouch = false;
        }
    }

    /// Advance the crouch transition. Returns `true` when the transition completes.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled {
            return false;
        }
        let step = if self.transition_duration > 0.0 {
            dt / self.transition_duration
        } else {
            1.0
        };

        // Start transitions first so progress advances in the same tick.
        if self.phase == CrouchPhase::Standing && self.wants_crouch {
            self.phase = CrouchPhase::CrouchingDown;
            self.transition_progress = 0.0;
        } else if self.phase == CrouchPhase::Crouched && !self.wants_crouch {
            self.phase = CrouchPhase::StandingUp;
            self.transition_progress = 1.0;
        }

        match self.phase {
            CrouchPhase::CrouchingDown => {
                self.transition_progress = (self.transition_progress + step).min(1.0);
                if self.transition_progress >= 1.0 {
                    self.phase = CrouchPhase::Crouched;
                    return true;
                }
                false
            }
            CrouchPhase::StandingUp => {
                self.transition_progress = (self.transition_progress - step).max(0.0);
                if self.transition_progress <= 0.0 {
                    self.phase = CrouchPhase::Standing;
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    pub fn is_crouched(&self) -> bool {
        matches!(
            self.phase,
            CrouchPhase::Crouched | CrouchPhase::CrouchingDown
        )
    }

    pub fn current_height_scale(&self) -> f32 {
        let t = self.transition_progress;
        1.0 - (1.0 - self.height_scale) * t
    }
}

impl Default for Crouch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crouch_transitions_to_crouched() {
        let mut c = Crouch::new().with_transition_duration(1.0);
        c.press();
        assert!(!c.tick(0.5));
        assert!(c.is_crouched());
        assert!(c.tick(0.6));
        assert_eq!(c.phase, CrouchPhase::Crouched);
    }

    #[test]
    fn release_transitions_to_standing() {
        let mut c = Crouch::new().with_transition_duration(0.01);
        c.press();
        c.tick(1.0);
        assert_eq!(c.phase, CrouchPhase::Crouched);
        c.release();
        c.tick(1.0);
        assert_eq!(c.phase, CrouchPhase::Standing);
    }

    #[test]
    fn height_scale_interpolates() {
        let mut c = Crouch::new()
            .with_transition_duration(1.0)
            .with_height_scale(0.5);
        c.press();
        c.tick(0.5);
        let h = c.current_height_scale();
        assert!(h > 0.5 && h < 1.0);
    }

    #[test]
    fn toggle_mode_flips_on_press() {
        let mut c = Crouch::new().toggle();
        c.press();
        assert!(c.wants_crouch);
        c.press();
        assert!(!c.wants_crouch);
    }

    #[test]
    fn disabled_cannot_crouch() {
        let mut c = Crouch::new().disabled();
        c.press();
        assert!(!c.tick(1.0));
        assert_eq!(c.phase, CrouchPhase::Standing);
    }
}
