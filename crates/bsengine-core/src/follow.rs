use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// Causes this entity's `Transform` to move toward another entity's position
/// at the given `speed` (units/second), with a local `offset` applied to the
/// target position. The `FollowPlugin` reads this and updates `Transform`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Follow {
    pub target: Entity,
    /// World-space offset added to the target's position before interpolation.
    pub offset: Vec3,
    /// Maximum move speed (units/second). Use `f32::INFINITY` for instant snap.
    pub speed: f32,
}

impl Follow {
    pub fn new(target: Entity) -> Self {
        Self {
            target,
            offset: Vec3::ZERO,
            speed: f32::INFINITY,
        }
    }

    pub fn with_offset(mut self, offset: Vec3) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }
}

/// Causes this entity's `Transform` rotation to orient toward another entity's
/// position each frame. The `LookAtPlugin` reads this and updates `Transform`.
#[derive(Component, Debug, Clone, Copy)]
pub struct LookAt {
    pub target: Entity,
    /// World-space up vector used to compute the orientation.
    pub up: Vec3,
}

impl LookAt {
    pub fn new(target: Entity) -> Self {
        Self {
            target,
            up: Vec3::Y,
        }
    }

    pub fn with_up(mut self, up: Vec3) -> Self {
        self.up = up.normalize();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    fn dummy_entity() -> Entity {
        let mut world = World::new();
        world.spawn_empty().id()
    }

    #[test]
    fn follow_defaults_instant_snap() {
        let e = dummy_entity();
        let f = Follow::new(e);
        assert!(f.speed.is_infinite());
        assert_eq!(f.offset, Vec3::ZERO);
    }

    #[test]
    fn follow_builder_sets_speed_and_offset() {
        let e = dummy_entity();
        let f = Follow::new(e).with_speed(5.0).with_offset(Vec3::Y * 2.0);
        assert_eq!(f.speed, 5.0);
        assert_eq!(f.offset, Vec3::new(0.0, 2.0, 0.0));
    }

    #[test]
    fn look_at_defaults_y_up() {
        let e = dummy_entity();
        let l = LookAt::new(e);
        assert_eq!(l.up, Vec3::Y);
    }

    #[test]
    fn look_at_custom_up_normalized() {
        let e = dummy_entity();
        let l = LookAt::new(e).with_up(Vec3::new(0.0, 2.0, 0.0));
        assert!((l.up.length() - 1.0).abs() < 0.001);
    }
}
