use bevy_ecs::prelude::{Component, Entity, ReflectComponent};
use bevy_reflect::Reflect;

/// Reference to this entity's parent, used to build transform hierarchies
/// consumed by [`crate::propagate_global_transforms`].
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Parent(pub Entity);

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::World;

    #[test]
    fn child_inherits_parent_translation() {
        let mut world = World::new();

        let parent = world
            .spawn((
                crate::Transform::from_position(glam::Vec3::new(1.0, 0.0, 0.0)),
                crate::GlobalTransform::default(),
            ))
            .id();

        let child = world
            .spawn((
                crate::Transform::from_position(glam::Vec3::new(0.0, 1.0, 0.0)),
                crate::GlobalTransform::default(),
                Parent(parent),
            ))
            .id();

        crate::propagate_global_transforms(&mut world);

        let child_gt = world.get::<crate::GlobalTransform>(child).unwrap();
        let pos = child_gt.0.w_axis.truncate();
        assert!((pos.x - 1.0).abs() < 1e-5, "x={}", pos.x);
        assert!((pos.y - 1.0).abs() < 1e-5, "y={}", pos.y);
        assert!(pos.z.abs() < 1e-5, "z={}", pos.z);
    }

    #[test]
    fn grandchild_inherits_grandparent_and_parent_translation() {
        let mut world = World::new();

        let grandparent = world
            .spawn((
                crate::Transform::from_position(glam::Vec3::new(10.0, 0.0, 0.0)),
                crate::GlobalTransform::default(),
            ))
            .id();

        let parent = world
            .spawn((
                crate::Transform::from_position(glam::Vec3::new(0.0, 1.0, 0.0)),
                crate::GlobalTransform::default(),
                Parent(grandparent),
            ))
            .id();

        let child = world
            .spawn((
                crate::Transform::from_position(glam::Vec3::new(0.0, 0.0, 1.0)),
                crate::GlobalTransform::default(),
                Parent(parent),
            ))
            .id();

        crate::propagate_global_transforms(&mut world);

        let child_gt = world.get::<crate::GlobalTransform>(child).unwrap();
        let pos = child_gt.0.w_axis.truncate();
        // grandparent (10,0,0) + parent-local (0,1,0) + child-local (0,0,1)
        assert!((pos.x - 10.0).abs() < 1e-5, "x={}", pos.x);
        assert!((pos.y - 1.0).abs() < 1e-5, "y={}", pos.y);
        assert!((pos.z - 1.0).abs() < 1e-5, "z={}", pos.z);
    }
}
