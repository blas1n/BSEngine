use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Phase of the climb interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClimbPhase {
    /// Not on a climbable surface.
    None,
    /// Gripping and moving along the climbable surface.
    Climbing,
    /// Pulling up over the top edge.
    MountingTop,
    /// Stepping off the bottom.
    DismountingBottom,
}

/// Ladder / climbable surface locomotion component.
///
/// The movement system sets `surface_axis` and `surface_normal` when a climbable
/// contact is detected, then calls `attach()`. While `phase == Climbing`, it applies
/// `climb_speed * input_direction` along `surface_axis` and suppresses gravity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Climb {
    pub phase: ClimbPhase,
    /// Up-axis of the climbable surface (e.g. Vec3::Y for a vertical ladder).
    pub surface_axis: Vec3,
    /// Outward normal of the surface (used to offset the character).
    pub surface_normal: Vec3,
    /// Movement speed along the surface axis (m/s).
    pub climb_speed: f32,
    /// 1-D position along `surface_axis` (written by movement system).
    pub position_on_surface: f32,
    /// Surface position at which the top-mount animation triggers.
    pub top_threshold: f32,
    /// Surface position at which the bottom-dismount animation triggers.
    pub bottom_threshold: f32,
    /// True when the character is pressing the "grab" / "interact" input.
    pub wants_climb: bool,
    /// True when the character wants to jump off the ladder.
    pub wants_jump_off: bool,
    /// Optional max speed override for descending (0 = same as climb_speed).
    pub descend_speed: f32,
    pub enabled: bool,
}

impl Climb {
    pub fn new(climb_speed: f32) -> Self {
        Self {
            phase: ClimbPhase::None,
            surface_axis: Vec3::Y,
            surface_normal: Vec3::Z,
            climb_speed: climb_speed.max(0.0),
            position_on_surface: 0.0,
            top_threshold: 0.0,
            bottom_threshold: 0.0,
            wants_climb: false,
            wants_jump_off: false,
            descend_speed: 0.0,
            enabled: true,
        }
    }

    pub fn with_descend_speed(mut self, s: f32) -> Self {
        self.descend_speed = s.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Attach to a climbable surface. Returns true if accepted.
    pub fn attach(&mut self, axis: Vec3, normal: Vec3, top: f32, bottom: f32) -> bool {
        if !self.enabled || self.phase == ClimbPhase::MountingTop {
            return false;
        }
        self.surface_axis = axis.normalize_or_zero();
        self.surface_normal = normal.normalize_or_zero();
        self.top_threshold = top;
        self.bottom_threshold = bottom;
        self.phase = ClimbPhase::Climbing;
        true
    }

    /// Detach from the surface (jumped, grabbed a ledge, fell off).
    pub fn detach(&mut self) {
        self.phase = ClimbPhase::None;
    }

    /// Advance state based on position along the surface. Call every frame while climbing.
    pub fn tick(&mut self, pos: f32) {
        if !self.enabled || self.phase == ClimbPhase::None {
            return;
        }
        self.position_on_surface = pos;

        if self.phase == ClimbPhase::Climbing {
            if pos >= self.top_threshold && self.top_threshold > self.bottom_threshold {
                self.phase = ClimbPhase::MountingTop;
            } else if pos <= self.bottom_threshold {
                self.phase = ClimbPhase::DismountingBottom;
            }
        }

        // Mounting / dismounting are cleared by the movement system calling detach().
    }

    pub fn is_climbing(&self) -> bool {
        self.phase == ClimbPhase::Climbing
    }

    pub fn is_attached(&self) -> bool {
        self.phase != ClimbPhase::None
    }

    /// Effective upward speed for the current phase (positive).
    pub fn ascent_speed(&self) -> f32 {
        self.climb_speed
    }

    /// Effective downward speed for the current phase (positive).
    pub fn descent_speed(&self) -> f32 {
        if self.descend_speed > 0.0 {
            self.descend_speed
        } else {
            self.climb_speed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_sets_climbing_phase() {
        let mut c = Climb::new(4.0);
        let ok = c.attach(Vec3::Y, Vec3::Z, 10.0, 0.0);
        assert!(ok);
        assert!(c.is_climbing());
    }

    #[test]
    fn tick_transitions_to_mounting_top() {
        let mut c = Climb::new(4.0);
        c.attach(Vec3::Y, Vec3::Z, 10.0, 0.0);
        c.tick(11.0); // past top threshold
        assert_eq!(c.phase, ClimbPhase::MountingTop);
    }

    #[test]
    fn tick_transitions_to_dismounting_bottom() {
        let mut c = Climb::new(4.0);
        c.attach(Vec3::Y, Vec3::Z, 10.0, 1.0);
        c.tick(0.5); // below bottom threshold
        assert_eq!(c.phase, ClimbPhase::DismountingBottom);
    }

    #[test]
    fn detach_clears_phase() {
        let mut c = Climb::new(4.0);
        c.attach(Vec3::Y, Vec3::Z, 10.0, 0.0);
        c.detach();
        assert!(!c.is_attached());
    }

    #[test]
    fn descend_speed_falls_back_to_climb_speed() {
        let c = Climb::new(4.0);
        assert_eq!(c.descent_speed(), 4.0);
    }

    #[test]
    fn custom_descend_speed_used_when_set() {
        let c = Climb::new(4.0).with_descend_speed(2.0);
        assert_eq!(c.descent_speed(), 2.0);
        assert_eq!(c.ascent_speed(), 4.0);
    }
}
