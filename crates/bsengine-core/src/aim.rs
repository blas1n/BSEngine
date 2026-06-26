use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// How the aim direction is determined each frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AimMode {
    /// Aim follows input direction directly (player-controlled).
    Input,
    /// Aim tracks a target entity's position.
    Target,
    /// Aim is held at a fixed world-space direction.
    Fixed,
    /// Aim is controlled by an external system (AI, cutscene).
    Manual,
}

/// Aiming state for ranged characters, turrets, and cameras.
///
/// Systems that need the current aim direction read `direction`.
/// The aim system writes `direction` based on `mode` each frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Aim {
    pub mode: AimMode,
    /// Current normalized aim direction in world space.
    pub direction: Vec3,
    /// Target entity (used in `Target` mode).
    pub target: Option<Entity>,
    /// Optional world-space offset added to target entity position before computing direction.
    pub target_offset: Vec3,
    /// Maximum turn rate in radians per second. `None` = instant.
    pub turn_rate: Option<f32>,
    /// If `true`, the aim has line-of-sight to the target.
    pub has_los: bool,
    /// Maximum effective range of the weapon/ability using this aim.
    pub max_range: Option<f32>,
    pub enabled: bool,
}

impl Aim {
    pub fn new() -> Self {
        Self {
            mode: AimMode::Input,
            direction: Vec3::NEG_Z,
            target: None,
            target_offset: Vec3::ZERO,
            turn_rate: None,
            has_los: true,
            max_range: None,
            enabled: true,
        }
    }

    pub fn with_mode(mut self, mode: AimMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_target(mut self, entity: Entity) -> Self {
        self.mode = AimMode::Target;
        self.target = Some(entity);
        self
    }

    pub fn with_turn_rate(mut self, radians_per_second: f32) -> Self {
        self.turn_rate = Some(radians_per_second.max(0.001));
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

    /// Rotate `direction` toward `desired` at `turn_rate` over `dt`.
    /// If `turn_rate` is `None`, direction is set instantly.
    pub fn rotate_toward(&mut self, desired: Vec3, dt: f32) {
        let desired = match desired.try_normalize() {
            Some(v) => v,
            None => return,
        };
        match self.turn_rate {
            None => self.direction = desired,
            Some(rate) => {
                let angle = self.direction.angle_between(desired);
                let max_step = rate * dt;
                if angle <= max_step {
                    self.direction = desired;
                } else {
                    let t = max_step / angle;
                    self.direction = self.direction.lerp(desired, t).normalize_or(self.direction);
                }
            }
        }
    }

    /// Set direction instantly (ignores turn_rate).
    pub fn set_direction(&mut self, dir: Vec3) {
        if let Some(d) = dir.try_normalize() {
            self.direction = d;
        }
    }

    pub fn is_within_range(&self, distance: f32) -> bool {
        match self.max_range {
            None => true,
            Some(r) => distance <= r,
        }
    }
}

impl Default for Aim {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn rotate_toward_instant_without_rate() {
        let mut a = Aim::new();
        a.rotate_toward(Vec3::X, 0.016);
        assert!((a.direction - Vec3::X).length() < 0.001);
    }

    #[test]
    fn rotate_toward_clamped_by_rate() {
        let mut a = Aim::new().with_turn_rate(PI); // 180°/s
        a.direction = Vec3::NEG_Z;
        a.rotate_toward(Vec3::X, 0.25); // 45° step
        let angle = a.direction.angle_between(Vec3::NEG_Z);
        assert!(angle > 0.0 && angle <= PI * 0.25 + 0.01);
    }

    #[test]
    fn set_direction_normalizes() {
        let mut a = Aim::new();
        a.set_direction(Vec3::new(2.0, 0.0, 0.0));
        assert!((a.direction.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn is_within_range_checks_max() {
        let a = Aim::new().with_max_range(10.0);
        assert!(a.is_within_range(5.0));
        assert!(!a.is_within_range(15.0));
    }

    #[test]
    fn no_range_limit_always_in_range() {
        let a = Aim::new();
        assert!(a.is_within_range(999.0));
    }
}
