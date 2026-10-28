use crate::{ReflectQuat, ReflectVec3};
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::{Mat4, Quat, Vec3};

/// Local position, rotation, and scale of an entity, relative to its parent (if any).
///
/// Requires [`crate::GlobalTransform`]: everything that asks where an entity is
/// reads the world-space one, and propagation only fills in one that exists, so
/// a Transform without it was an entity at the origin to every system. Unity's
/// `Transform` and Godot's `Node3D` carry both by construction; Bevy's own
/// `Transform` has required its `GlobalTransform` since 0.15.
///
/// Inserting one also writes that `GlobalTransform` (see
/// [`fill_global_transform`]), so an entity is where it was put from the moment
/// it exists rather than from the next PostUpdate.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
#[require(crate::GlobalTransform)]
#[component(on_insert = fill_global_transform)]
pub struct Transform {
    /// Local-space position.
    pub position: ReflectVec3,
    /// Local-space rotation.
    pub rotation: ReflectQuat,
    /// Local-space scale.
    pub scale: ReflectVec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO.into(),
            rotation: Quat::IDENTITY.into(),
            scale: Vec3::ONE.into(),
        }
    }
}

impl Transform {
    /// Creates a transform at the given position with identity rotation and unit scale.
    pub fn from_position(position: Vec3) -> Self {
        Self {
            position: position.into(),
            ..Default::default()
        }
    }

    /// Rotates this transform so it faces `target`, using `up` to determine the roll axis.
    pub fn looking_at(mut self, target: Vec3, up: Vec3) -> Self {
        let dir = (target - self.position.0).normalize();
        let right = up.cross(dir).normalize();
        let up = dir.cross(right);
        self.rotation = Quat::from_mat3(&glam::Mat3::from_cols(right, up, dir)).into();
        self
    }

    /// Builds the local-to-world transform matrix (scale, then rotate, then translate).
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale.0, self.rotation.0, self.position.0)
    }

    /// Builds the world-to-view matrix, i.e. the inverse of `to_matrix()`.
    pub fn view_matrix(&self) -> Mat4 {
        self.to_matrix().inverse()
    }
}

/// `on_insert` hook of [`Transform`]: writes the entity's `GlobalTransform`
/// from it, through the parent's `GlobalTransform` when it has a [`crate::Parent`].
///
/// [`crate::propagate_global_transforms`] runs in PostUpdate and everything that
/// moves things runs before it. Before the `GlobalTransform` was required, an
/// entity spawned this frame had none, and readers such as cloth and the
/// behaviour tree's `MoveTo` fell back to its `Transform`; a required one left at
/// identity until PostUpdate would instead put it at the origin for that frame.
/// Unity's `transform.position` and Godot's `global_transform` are right as soon
/// as the object exists, and so is this. A parent's own `GlobalTransform` may be
/// a frame old, which propagation corrects as it always has.
fn fill_global_transform(
    mut world: bevy_ecs::world::DeferredWorld,
    context: bevy_ecs::component::HookContext,
) {
    let entity = context.entity;
    let Some(local) = world.get::<Transform>(entity).map(Transform::to_matrix) else {
        return;
    };
    let parent_world = world
        .get::<crate::ChildOf>(entity)
        .and_then(|p| world.get::<crate::GlobalTransform>(p.0))
        .map(|g| g.0 .0);
    let matrix = parent_world.map_or(local, |p| p * local);
    if let Some(mut global) = world.get_mut::<crate::GlobalTransform>(entity) {
        global.0 = matrix.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transform_alone_gets_a_world_position() {
        // Every reader of where an entity *is* -- the renderer, physics,
        // audio, picking -- reads GlobalTransform, and propagation only writes
        // one that exists. So a Transform spawned without it sat at the origin
        // for all of them, and every spawn site had to remember the pair.
        let mut world = bevy_ecs::world::World::new();
        let e = world
            .spawn(Transform::from_position(Vec3::new(2.0, 3.0, 4.0)))
            .id();
        assert!(
            world.get::<crate::GlobalTransform>(e).is_some(),
            "a Transform brings its GlobalTransform"
        );
        crate::propagate_global_transforms(&mut world);
        let pos = world
            .get::<crate::GlobalTransform>(e)
            .unwrap()
            .0
            .w_axis
            .truncate();
        assert!(
            (pos - Vec3::new(2.0, 3.0, 4.0)).length() < 1e-5,
            "and propagation fills it in: pos={pos}"
        );
    }

    #[test]
    fn a_spawned_transform_is_in_the_world_before_propagation_runs() {
        // Propagation runs in PostUpdate, and gameplay -- a behaviour tree's
        // MoveTo, cloth reading where it hangs -- runs before it. Before the
        // GlobalTransform was required those readers fell back to the
        // Transform of an entity that had none; a required one that sat at
        // identity until PostUpdate would send them to the origin for the
        // frame it was spawned in. So insertion fills it in, from the parent's
        // world transform when there is one.
        let mut world = bevy_ecs::world::World::new();
        let parent = world
            .spawn(Transform::from_position(Vec3::new(10.0, 0.0, 0.0)))
            .id();
        let root = world
            .get::<crate::GlobalTransform>(parent)
            .unwrap()
            .0
            .w_axis
            .truncate();
        assert!(
            (root - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-5,
            "root: {root}"
        );

        let child = world
            .spawn((
                Transform::from_position(Vec3::new(0.0, 1.0, 0.0)),
                crate::ChildOf(parent),
            ))
            .id();
        let pos = world
            .get::<crate::GlobalTransform>(child)
            .unwrap()
            .0
            .w_axis
            .truncate();
        assert!(
            (pos - Vec3::new(10.0, 1.0, 0.0)).length() < 1e-5,
            "child: {pos}"
        );

        // And a replaced Transform is a moved entity, not a stale one.
        world
            .entity_mut(parent)
            .insert(Transform::from_position(Vec3::new(-4.0, 0.0, 0.0)));
        let moved = world
            .get::<crate::GlobalTransform>(parent)
            .unwrap()
            .0
            .w_axis
            .truncate();
        assert!(
            (moved - Vec3::new(-4.0, 0.0, 0.0)).length() < 1e-5,
            "moved: {moved}"
        );
    }

    #[test]
    fn every_spatial_component_brings_a_transform() {
        // The things placed in space -- the set Bevy 0.15 made require its own
        // Transform (cameras, meshes, lights, light probes, decals). Each is
        // read by the renderer only through a query with a mandatory
        // `&Transform`, so one spawned bare was skipped without a word; Unity's
        // GameObject and Godot's Node3D cannot exist without a transform at
        // all. `MeshRenderer` lives in bsengine-render and is covered there.
        fn check<C: bevy_ecs::component::Component + Default>(missing: &mut Vec<&'static str>) {
            let mut world = bevy_ecs::world::World::new();
            let e = world.spawn(C::default()).id();
            if world.get::<Transform>(e).is_none()
                || world.get::<crate::GlobalTransform>(e).is_none()
            {
                missing.push(std::any::type_name::<C>());
            }
        }
        let mut missing = Vec::new();
        check::<crate::Camera>(&mut missing);
        check::<crate::DirectionalLight>(&mut missing);
        check::<crate::PointLight>(&mut missing);
        check::<crate::SpotLight>(&mut missing);
        check::<crate::RectLight>(&mut missing);
        check::<crate::ReflectionProbe>(&mut missing);
        check::<crate::LightProbeVolume>(&mut missing);
        check::<crate::Decal>(&mut missing);
        assert!(
            missing.is_empty(),
            "spawned without a Transform: {missing:?}"
        );
    }

    #[test]
    fn a_required_transform_does_not_replace_an_authored_one() {
        // The other side: `#[require]` fills in only what is absent, so a
        // light spawned *with* its Transform keeps the one it was given.
        let mut world = bevy_ecs::world::World::new();
        let e = world
            .spawn((
                crate::PointLight::default(),
                Transform::from_position(Vec3::new(1.0, 2.0, 3.0)),
            ))
            .id();
        assert_eq!(
            world.get::<Transform>(e).unwrap().position,
            Vec3::new(1.0, 2.0, 3.0).into()
        );
    }

    #[test]
    fn default_is_identity() {
        let t = Transform::default();
        assert_eq!(t.position, Vec3::ZERO.into());
        assert_eq!(t.rotation, Quat::IDENTITY.into());
        assert_eq!(t.scale, Vec3::ONE.into());
    }

    #[test]
    fn to_matrix_identity_for_default() {
        let t = Transform::default();
        assert!(t.to_matrix().abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }

    #[test]
    fn from_translation_sets_position() {
        let t = Transform::from_position(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(t.position, Vec3::new(1.0, 2.0, 3.0).into());
        assert_eq!(t.scale, Vec3::ONE.into());
    }

    #[test]
    fn view_matrix_is_inverse_of_model() {
        let t = Transform::from_position(Vec3::new(0.0, 0.0, 5.0));
        let model = t.to_matrix();
        let view = t.view_matrix();
        let product = model * view;
        assert!(product.abs_diff_eq(Mat4::IDENTITY, 1e-5));
    }
}
