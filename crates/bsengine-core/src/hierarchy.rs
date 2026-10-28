//! Parent and child: Bevy's own [`ChildOf`] / [`Children`] relationship,
//! re-exported from the crate root.
//!
//! This crate had its own `Parent(Entity)` until Bevy 0.16 brought the
//! relationship into `bevy_ecs`. What changed with it: a parent's
//! [`Children`] is kept by the relationship's hooks rather than found by
//! scanning every `Parent`; a `ChildOf` naming an entity that does not exist is
//! removed with a warning rather than left dangling; and despawning a parent
//! despawns its children, as Unity's `Destroy` and Godot's `queue_free` do
//! (Unreal's actor destroy detaches them instead -- the two engines whose model
//! of "an object and its children" this one follows are the first two).
//! [`crate::propagate_global_transforms`] and `Transform`'s insert hook read
//! `ChildOf` to place a child in its parent's space.

pub use bevy_ecs::hierarchy::{ChildOf, Children};

#[cfg(test)]
mod tests {
    use super::ChildOf;
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
                ChildOf(parent),
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
                ChildOf(grandparent),
            ))
            .id();

        let child = world
            .spawn((
                crate::Transform::from_position(glam::Vec3::new(0.0, 0.0, 1.0)),
                crate::GlobalTransform::default(),
                ChildOf(parent),
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
