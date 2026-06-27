use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Marks an entity as kinematically controlled: velocity is set by gameplay
/// systems (animation, AI, scripts) rather than the physics solver.
///
/// The physics system still resolves collisions against kinematic bodies but
/// does NOT apply forces or gravity to them. Set `desired_velocity` each frame;
/// after the physics step, `actual_velocity` and `is_grounded` reflect the
/// result.
///
/// Unlike `CharacterController` (which is a complete capsule-mover), `Kinematic`
/// is a low-level tag that lets any collider shape participate in kinematic
/// movement. Pair with `Collider` for collision.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Kinematic {
    /// Velocity the entity wants to move at this frame (world space, units/s).
    pub desired_velocity: Vec3,
    /// Velocity actually achieved after collision resolution (written by physics).
    pub actual_velocity: Vec3,
    /// Angular velocity around the Y axis (radians/s).
    pub angular_velocity: f32,
    /// True if the physics step detected ground contact this frame.
    pub is_grounded: bool,
    /// True if the physics step detected a ceiling contact this frame.
    pub is_ceiling: bool,
    /// True if the physics step detected a wall contact this frame.
    pub is_wall: bool,
    /// Surface normal of the most recent ground contact.
    pub ground_normal: Vec3,
    pub enabled: bool,
}

impl Kinematic {
    pub fn new() -> Self {
        Self {
            desired_velocity: Vec3::ZERO,
            actual_velocity: Vec3::ZERO,
            angular_velocity: 0.0,
            is_grounded: false,
            is_ceiling: false,
            is_wall: false,
            ground_normal: Vec3::Y,
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set desired velocity and return self (builder helper).
    pub fn with_velocity(mut self, v: Vec3) -> Self {
        self.desired_velocity = v;
        self
    }

    /// True when colliding with anything this frame.
    pub fn is_colliding(&self) -> bool {
        self.is_grounded || self.is_ceiling || self.is_wall
    }

    /// Signed speed along the vertical axis (positive = moving up).
    pub fn vertical_speed(&self) -> f32 {
        self.actual_velocity.y
    }

    /// Horizontal speed (XZ plane).
    pub fn horizontal_speed(&self) -> f32 {
        let h = Vec3::new(self.actual_velocity.x, 0.0, self.actual_velocity.z);
        h.length()
    }

    /// Clear per-frame physics outputs (call before the physics step).
    pub fn reset_contacts(&mut self) {
        self.is_grounded = false;
        self.is_ceiling = false;
        self.is_wall = false;
        self.ground_normal = Vec3::Y;
    }
}

impl Default for Kinematic {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_with_zero_velocity() {
        let k = Kinematic::new();
        assert_eq!(k.desired_velocity, Vec3::ZERO);
        assert_eq!(k.actual_velocity, Vec3::ZERO);
    }

    #[test]
    fn is_colliding_when_grounded() {
        let mut k = Kinematic::new();
        k.is_grounded = true;
        assert!(k.is_colliding());
    }

    #[test]
    fn horizontal_speed_ignores_vertical() {
        let mut k = Kinematic::new();
        k.actual_velocity = Vec3::new(3.0, 10.0, 4.0);
        assert!((k.horizontal_speed() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn vertical_speed_returns_y() {
        let mut k = Kinematic::new();
        k.actual_velocity = Vec3::new(1.0, -5.0, 0.0);
        assert!((k.vertical_speed() - (-5.0)).abs() < 1e-5);
    }

    #[test]
    fn reset_contacts_clears_flags() {
        let mut k = Kinematic::new();
        k.is_grounded = true;
        k.is_ceiling = true;
        k.is_wall = true;
        k.reset_contacts();
        assert!(!k.is_colliding());
    }

    #[test]
    fn disabled_flag_accessible() {
        let k = Kinematic::new().disabled();
        assert!(!k.enabled);
    }
}
