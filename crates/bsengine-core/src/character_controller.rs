use bevy_ecs::prelude::Component;

/// Kinematic character controller — moves an entity without exposing it to external forces.
/// The movement system sweeps a capsule collider and resolves collisions each frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct CharacterController {
    /// Radius of the capsule collider in world units.
    pub radius: f32,
    /// Total height (tip to tip) of the capsule collider in world units.
    pub height: f32,
    /// Maximum slope angle in degrees that the character can walk up.
    pub slope_limit: f32,
    /// Maximum height of a step the character automatically climbs.
    pub step_offset: f32,
    /// Thin shell around the collider used to prevent interpenetration.
    pub skin_width: f32,
    /// Whether the movement system applies gravity to this controller.
    pub apply_gravity: bool,
    /// Whether the controller is grounded this frame (set by the movement system).
    pub is_grounded: bool,
}

impl CharacterController {
    pub fn new(radius: f32, height: f32) -> Self {
        let radius = radius.max(0.0);
        let height = height.max(radius * 2.0); // height must cover the capsule
        Self {
            radius,
            height,
            slope_limit: 45.0,
            step_offset: 0.3,
            skin_width: 0.01,
            apply_gravity: true,
            is_grounded: false,
        }
    }

    pub fn with_slope_limit(mut self, degrees: f32) -> Self {
        self.slope_limit = degrees.clamp(0.0, 90.0);
        self
    }

    pub fn with_step_offset(mut self, offset: f32) -> Self {
        self.step_offset = offset.max(0.0);
        self
    }

    pub fn with_skin_width(mut self, width: f32) -> Self {
        self.skin_width = width.max(0.0);
        self
    }

    pub fn without_gravity(mut self) -> Self {
        self.apply_gravity = false;
        self
    }

    pub fn capsule_half_height(&self) -> f32 {
        self.height / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_controller_defaults() {
        let cc = CharacterController::new(0.3, 1.8);
        assert!((cc.radius - 0.3).abs() < 0.001);
        assert!((cc.height - 1.8).abs() < 0.001);
        assert!((cc.slope_limit - 45.0).abs() < 0.001);
        assert!((cc.step_offset - 0.3).abs() < 0.001);
        assert!(cc.apply_gravity);
        assert!(!cc.is_grounded);
    }

    #[test]
    fn height_clamped_to_capsule_minimum() {
        let cc = CharacterController::new(0.5, 0.1); // height < 2*radius
        assert!(cc.height >= cc.radius * 2.0);
    }

    #[test]
    fn slope_limit_clamped() {
        let cc = CharacterController::new(0.3, 1.8).with_slope_limit(120.0);
        assert!((cc.slope_limit - 90.0).abs() < 0.001);
    }

    #[test]
    fn without_gravity() {
        let cc = CharacterController::new(0.3, 1.8).without_gravity();
        assert!(!cc.apply_gravity);
    }

    #[test]
    fn capsule_half_height() {
        let cc = CharacterController::new(0.3, 2.0);
        assert!((cc.capsule_half_height() - 1.0).abs() < 0.001);
    }
}
