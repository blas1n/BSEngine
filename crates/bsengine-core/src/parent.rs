use bevy_ecs::prelude::{Component, Entity};

#[derive(Component, Debug, Clone, Copy)]
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
                crate::Transform::from_translation(glam::Vec3::new(1.0, 0.0, 0.0)),
                crate::GlobalTransform::default(),
            ))
            .id();

        let child = world
            .spawn((
                crate::Transform::from_translation(glam::Vec3::new(0.0, 1.0, 0.0)),
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
}
