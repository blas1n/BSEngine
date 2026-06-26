use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// How the homing guidance adjusts its course each frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomingMode {
    /// Steers toward current target position at a fixed turn rate.
    Pursuit,
    /// Leads the target by predicting its position based on relative velocity.
    PredictiveLead,
    /// Locks flight direction on acquisition; does not adjust after launch.
    LockedOn,
}

/// Homing guidance for a projectile or missile entity.
///
/// The movement system reads `target`, `speed`, and `turn_rate` each frame to
/// steer the entity. Call `tick(own_pos, target_pos, dt)` to get the updated
/// desired heading; apply it to the entity's velocity externally.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Homing {
    pub target: Option<Entity>,
    pub mode: HomingMode,
    /// Flight speed in units/second.
    pub speed: f32,
    /// Maximum angular turn rate in radians/second.
    pub turn_rate: f32,
    /// Current heading (unit vector). Updated by `tick`.
    pub heading: Vec3,
    /// How far the target can be before lock is broken. `None` = unlimited.
    pub max_range: Option<f32>,
    /// Time-to-live; projectile expires when it reaches 0.
    pub lifetime: f32,
    pub enabled: bool,
}

impl Homing {
    pub fn new(speed: f32, turn_rate: f32, lifetime: f32) -> Self {
        Self {
            target: None,
            mode: HomingMode::Pursuit,
            speed: speed.max(0.0),
            turn_rate: turn_rate.max(0.0),
            heading: Vec3::Z,
            max_range: None,
            lifetime: lifetime.max(0.0),
            enabled: true,
        }
    }

    pub fn with_mode(mut self, mode: HomingMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_initial_heading(mut self, heading: Vec3) -> Self {
        self.heading = heading.normalize_or_zero();
        self
    }

    pub fn with_max_range(mut self, range: f32) -> Self {
        self.max_range = Some(range.max(0.0));
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Acquire a target and optionally lock heading (for `LockedOn` mode).
    pub fn acquire(&mut self, target: Entity, initial_dir: Vec3) {
        self.target = Some(target);
        if self.mode == HomingMode::LockedOn {
            self.heading = initial_dir.normalize_or_zero();
        }
    }

    /// Compute the desired heading toward `target_pos` from `own_pos`, then
    /// rotate `self.heading` toward it at `turn_rate` rad/s.
    /// Also decrements `lifetime` by `dt`.
    /// Returns the velocity to apply this frame (heading * speed).
    pub fn tick(&mut self, own_pos: Vec3, target_pos: Vec3, dt: f32) -> Vec3 {
        self.lifetime = (self.lifetime - dt).max(0.0);

        if !self.enabled || self.target.is_none() {
            return self.heading * self.speed;
        }

        // Range check.
        let to_target = target_pos - own_pos;
        if let Some(max_r) = self.max_range {
            if to_target.length() > max_r {
                self.target = None;
                return self.heading * self.speed;
            }
        }

        if self.mode == HomingMode::LockedOn {
            return self.heading * self.speed;
        }

        let desired = to_target.normalize_or_zero();
        // Rotate current heading toward desired at turn_rate rad/s.
        let angle = self.heading.angle_between(desired);
        let max_step = self.turn_rate * dt;
        if angle <= max_step || angle == 0.0 {
            self.heading = desired;
        } else {
            // Slerp by clamped fraction.
            let t = max_step / angle;
            self.heading = self.heading.lerp(desired, t).normalize_or_zero();
        }

        self.heading * self.speed
    }

    pub fn is_alive(&self) -> bool {
        self.lifetime > 0.0
    }

    pub fn has_target(&self) -> bool {
        self.target.is_some()
    }
}

impl Default for Homing {
    fn default() -> Self {
        Self::new(20.0, std::f32::consts::PI, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::World;

    fn spawn() -> Entity {
        World::new().spawn_empty().id()
    }

    #[test]
    fn homing_steers_toward_target() {
        let e = spawn();
        let mut h = Homing::new(10.0, std::f32::consts::PI * 2.0, 5.0);
        h.heading = Vec3::Z;
        h.acquire(e, Vec3::Z);
        // Target is directly to the right; should steer that way.
        let vel = h.tick(Vec3::ZERO, Vec3::X * 5.0, 1.0);
        assert!(vel.x > 0.0, "should steer toward +X");
    }

    #[test]
    fn locked_on_ignores_target_movement() {
        let e = spawn();
        let mut h = Homing::new(10.0, std::f32::consts::PI, 5.0).with_mode(HomingMode::LockedOn);
        h.acquire(e, Vec3::Z);
        let vel = h.tick(Vec3::ZERO, Vec3::X * 100.0, 0.1);
        // Heading stays locked to initial Z direction.
        assert!(vel.z > 0.0);
    }

    #[test]
    fn lifetime_decrements() {
        let mut h = Homing::new(10.0, 1.0, 2.0);
        h.tick(Vec3::ZERO, Vec3::X, 1.0);
        assert!((h.lifetime - 1.0).abs() < 0.001);
        assert!(h.is_alive());
        h.tick(Vec3::ZERO, Vec3::X, 1.5);
        assert!(!h.is_alive());
    }

    #[test]
    fn range_break_clears_target() {
        let e = spawn();
        let mut h = Homing::new(10.0, 1.0, 10.0).with_max_range(5.0);
        h.target = Some(e);
        h.tick(Vec3::ZERO, Vec3::X * 10.0, 0.1);
        assert!(h.target.is_none());
    }

    #[test]
    fn disabled_homing_does_not_steer() {
        let e = spawn();
        let mut h = Homing::new(10.0, std::f32::consts::PI * 2.0, 5.0).disabled();
        h.heading = Vec3::Z;
        h.acquire(e, Vec3::Z);
        let vel = h.tick(Vec3::ZERO, Vec3::X * 5.0, 1.0);
        // Disabled → heading unchanged (Z direction).
        assert!(vel.z > 0.0);
    }
}
