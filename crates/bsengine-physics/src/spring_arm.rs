//! A camera boom that pulls in when something is in the way.

use bevy_ecs::prelude::*;
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use bsengine_core::{Parent, Transform};
use glam::{Mat4, Vec3};

use crate::PhysicsWorld;

/// Holds this entity's children `length` behind it -- along its local +Z --
/// and pulls them in when the world is in the way: the third-person camera
/// rig Unreal's `SpringArmComponent` and Godot's `SpringArm3D` both provide.
///
/// Put it on a pivot entity parented to the character, turn the pivot to
/// aim, and make the camera a child of the pivot. Each frame the space from
/// the pivot back to `length` is swept with a sphere of `probe_radius`
/// (a ray at 0); on a hit the children move to just short of it, `margin`
/// in front, so the camera stays on this side of the wall instead of seeing
/// through it. The pivot's own body and its ancestors' -- the character the
/// arm hangs off -- are not obstacles, as Unreal ignores the owning actor.
///
/// Only the children's local **position** is set (to `(0, 0, length)` or
/// shorter); their rotation stays what it is, which for a camera child left
/// at identity means looking back along the arm at the pivot.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct SpringArm {
    /// How far behind the pivot the children sit when nothing is in the way.
    pub length: f32,
    /// Radius of the sphere swept along the arm; 0 sweeps a ray.
    pub probe_radius: f32,
    /// How far in front of an obstacle the children stop.
    pub margin: f32,
    /// Whether the world can push the arm in at all. Off holds it at
    /// `length` regardless (Unreal's `bDoCollisionTest`).
    pub collision: bool,
    /// The length the arm had this frame: `length`, or less where something
    /// was in the way. Written by the arm, read by a script that wants to
    /// fade the character when the camera is close (Godot's
    /// `get_hit_length`).
    #[reflect(ignore)]
    pub current_length: f32,
}

impl Default for SpringArm {
    fn default() -> Self {
        Self {
            // Unreal's 300 cm, in this engine's metres; Godot's 1 m is a
            // shoulder camera rather than a follow camera.
            length: 3.0,
            // Unreal's `ProbeSize`: 12 cm.
            probe_radius: 0.12,
            // Godot's default margin.
            margin: 0.01,
            collision: true,
            current_length: 3.0,
        }
    }
}

/// This entity's world matrix, from the `Transform`s up its `Parent` chain as
/// they are now -- not `GlobalTransform`, which is last frame's until this
/// frame's propagation runs after the arm.
fn world_matrix(entity: Entity, transforms: &Query<(&Transform, Option<&Parent>)>) -> Mat4 {
    let mut matrix = Mat4::IDENTITY;
    let mut current = Some(entity);
    // Bounded, as propagation is, so a parent cycle cannot hang the frame.
    for _ in 0..32 {
        let Some(e) = current else { break };
        let Ok((transform, parent)) = transforms.get(e) else {
            break;
        };
        matrix = transform.to_matrix() * matrix;
        current = parent.map(|p| p.0);
    }
    matrix
}

/// The entity and its ancestors: what the arm must not collide with.
fn with_ancestors(
    entity: Entity,
    transforms: &Query<(&Transform, Option<&Parent>)>,
) -> Vec<Entity> {
    let mut out = vec![entity];
    let mut current = transforms
        .get(entity)
        .ok()
        .and_then(|(_, p)| p.map(|p| p.0));
    while let Some(e) = current {
        if out.contains(&e) || out.len() > 32 {
            break;
        }
        out.push(e);
        current = transforms.get(e).ok().and_then(|(_, p)| p.map(|p| p.0));
    }
    out
}

/// Sets every spring arm's length for this frame and moves its children to
/// the end of it. Runs before transforms are propagated, so the camera's
/// world position this frame is the arm's.
#[allow(clippy::type_complexity)] // a ParamSet of two queries is the type
pub fn update_spring_arms(
    physics: Option<Res<PhysicsWorld>>,
    mut arms: Query<(Entity, &mut SpringArm)>,
    children: Query<(Entity, &Parent)>,
    mut transforms: ParamSet<(Query<(&Transform, Option<&Parent>)>, Query<&mut Transform>)>,
) {
    let mut placed: Vec<(Entity, f32)> = Vec::new();
    for (arm_entity, mut arm) in arms.iter_mut() {
        let length = arm.length.max(0.0);
        let reach = if arm.collision {
            let read = transforms.p0();
            let world = world_matrix(arm_entity, &read);
            let origin = world.transform_point3(Vec3::ZERO);
            let dir = world.transform_vector3(Vec3::Z).normalize_or_zero();
            let mut exclude = with_ancestors(arm_entity, &read);
            exclude.extend(
                children
                    .iter()
                    .filter(|(_, p)| p.0 == arm_entity)
                    .map(|(e, _)| e),
            );
            physics
                .as_deref()
                .filter(|_| dir != Vec3::ZERO)
                .and_then(|world| {
                    world.cast_sphere_excluding(origin, dir, length, arm.probe_radius, &exclude)
                })
                .map_or(length, |hit| (hit - arm.margin).clamp(0.0, length))
        } else {
            length
        };
        arm.current_length = reach;
        placed.push((arm_entity, reach));
    }
    let mut write = transforms.p1();
    for (child, parent) in children.iter() {
        if let Some(&(_, reach)) = placed.iter().find(|(arm, _)| *arm == parent.0) {
            if let Ok(mut transform) = write.get_mut(child) {
                transform.position = Vec3::new(0.0, 0.0, reach).into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Collider, PhysicsInput, PhysicsPlugin, RigidBody};
    use bsengine_app::new_app;
    use glam::Quat;

    /// A fixed physics box at `at` with these half-extents.
    fn wall(app: &mut bevy_app::App, at: Vec3, half: Vec3) -> Entity {
        app.world_mut()
            .spawn((
                Transform::from_position(at),
                RigidBody::fixed(),
                Collider::cuboid(half.x, half.y, half.z),
                PhysicsInput {
                    position: at.into(),
                    rotation: Quat::IDENTITY.into(),
                },
            ))
            .id()
    }

    /// A character with a collider of its own at `at`, a pivot 1.5 up on it
    /// carrying `arm`, and a camera on the pivot. Returns (character, pivot,
    /// camera).
    fn rig(app: &mut bevy_app::App, at: Vec3, arm: SpringArm) -> (Entity, Entity, Entity) {
        let character = app
            .world_mut()
            .spawn((
                Transform::from_position(at),
                RigidBody::fixed(),
                // Big enough to enclose the pivot 1.5 up, so the arm starts
                // inside its own character -- which it must not count.
                Collider::ball(2.0),
                PhysicsInput {
                    position: at.into(),
                    rotation: Quat::IDENTITY.into(),
                },
            ))
            .id();
        let pivot = app
            .world_mut()
            .spawn((
                Transform::from_position(Vec3::new(0.0, 1.5, 0.0)),
                Parent(character),
                arm,
            ))
            .id();
        let camera = app
            .world_mut()
            .spawn((Transform::default(), Parent(pivot)))
            .id();
        (character, pivot, camera)
    }

    fn app() -> bevy_app::App {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        app
    }

    fn camera_z(app: &bevy_app::App, camera: Entity) -> f32 {
        app.world().get::<Transform>(camera).unwrap().position.0.z
    }

    fn reach(app: &bevy_app::App, pivot: Entity) -> f32 {
        app.world().get::<SpringArm>(pivot).unwrap().current_length
    }

    /// Nothing behind: the camera sits the full length back. The character's
    /// own collider, which the arm starts inside, is not an obstacle -- the
    /// premise checks that it would have been one.
    #[test]
    fn with_nothing_behind_the_arm_is_its_full_length() {
        let mut app = app();
        let (_, pivot, camera) = rig(&mut app, Vec3::ZERO, SpringArm::default());
        app.update();
        app.update();
        assert!(
            app.world()
                .resource::<PhysicsWorld>()
                .cast_sphere_excluding(Vec3::new(0.0, 1.5, 0.0), Vec3::Z, 3.0, 0.12, &[])
                .is_some(),
            "premise: unexcluded, the character's own collider stops the sweep"
        );
        assert_eq!(reach(&app, pivot), 3.0);
        assert_eq!(camera_z(&app, camera), 3.0);
    }

    /// A wall behind the character pulls the camera in to just short of it:
    /// the wall's face, less the probe's radius and the margin.
    #[test]
    fn a_wall_behind_pulls_the_camera_in() {
        let mut app = app();
        wall(&mut app, Vec3::new(0.0, 1.5, 2.1), Vec3::new(5.0, 5.0, 0.1));
        let (_, pivot, camera) = rig(&mut app, Vec3::ZERO, SpringArm::default());
        app.update();
        app.update();
        let expected = 2.0 - 0.12 - 0.01;
        assert!(
            (reach(&app, pivot) - expected).abs() < 0.005,
            "stopped at the wall: {} (expected about {expected})",
            reach(&app, pivot)
        );
        assert_eq!(
            camera_z(&app, camera),
            reach(&app, pivot),
            "the camera is where the arm ends"
        );
    }

    /// The arm points along the pivot's own +Z: turned a quarter about Y it
    /// points along world +X, where the wall now is.
    #[test]
    fn the_arm_follows_the_pivots_rotation() {
        let mut app = app();
        wall(&mut app, Vec3::new(2.1, 1.5, 0.0), Vec3::new(0.1, 5.0, 5.0));
        let (_, pivot, _) = rig(&mut app, Vec3::ZERO, SpringArm::default());
        app.update();
        assert_eq!(
            reach(&app, pivot),
            3.0,
            "premise: facing +Z, the +X wall is not behind"
        );
        app.world_mut()
            .get_mut::<Transform>(pivot)
            .unwrap()
            .rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).into();
        app.update();
        assert!(
            reach(&app, pivot) < 2.0,
            "turned towards it: {}",
            reach(&app, pivot)
        );
    }

    /// With collision off the wall is ignored.
    #[test]
    fn collision_off_ignores_the_world() {
        let mut app = app();
        wall(&mut app, Vec3::new(0.0, 1.5, 2.1), Vec3::new(5.0, 5.0, 0.1));
        let (_, pivot, camera) = rig(
            &mut app,
            Vec3::ZERO,
            SpringArm {
                collision: false,
                ..Default::default()
            },
        );
        app.update();
        app.update();
        assert_eq!(reach(&app, pivot), 3.0);
        assert_eq!(camera_z(&app, camera), 3.0);
    }

    /// The probe's radius is what keeps the camera out of a gap a ray slips
    /// through: two walls with a 10 cm slit along the arm stop the 12 cm
    /// sphere, and not a ray.
    #[test]
    fn the_probe_sphere_catches_what_a_ray_slips_past() {
        // One rig per world: two at one spot would each find the other's
        // character in the way.
        let reach_with = |probe_radius: f32| {
            let mut app = app();
            // Left and right of the arm's line (x = 0), 5 cm clear each side.
            wall(
                &mut app,
                Vec3::new(-1.05, 1.5, 2.5),
                Vec3::new(1.0, 5.0, 0.1),
            );
            wall(
                &mut app,
                Vec3::new(1.05, 1.5, 2.5),
                Vec3::new(1.0, 5.0, 0.1),
            );
            let (_, pivot, _) = rig(
                &mut app,
                Vec3::ZERO,
                SpringArm {
                    probe_radius,
                    ..Default::default()
                },
            );
            app.update();
            app.update();
            reach(&app, pivot)
        };
        assert_eq!(reach_with(0.0), 3.0, "the ray passes through the slit");
        let sphere = reach_with(0.12);
        assert!(sphere < 2.5, "the sphere does not: {sphere}");
    }

    /// The camera's world position this frame is where the arm put it this
    /// frame: the arm runs before transforms are propagated. Checked with the
    /// propagation registered both before and after the physics plugin -- a
    /// pair of systems with no order between them still runs in a fixed one,
    /// so one registration order alone could pass by luck.
    #[test]
    fn the_camera_is_drawn_where_the_arm_put_it_this_frame() {
        for propagate_first in [true, false] {
            let mut app = new_app();
            if propagate_first {
                app.add_systems(
                    bevy_app::PostUpdate,
                    bsengine_core::propagate_global_transforms,
                );
                app.add_plugins(PhysicsPlugin);
            } else {
                app.add_plugins(PhysicsPlugin);
                app.add_systems(
                    bevy_app::PostUpdate,
                    bsengine_core::propagate_global_transforms,
                );
            }
            wall(&mut app, Vec3::new(0.0, 1.5, 2.1), Vec3::new(5.0, 5.0, 0.1));
            let (_, pivot, camera) = rig(&mut app, Vec3::ZERO, SpringArm::default());
            app.world_mut()
                .entity_mut(camera)
                .insert(bsengine_core::GlobalTransform::default());
            app.update();
            let world_z = app
                .world()
                .get::<bsengine_core::GlobalTransform>(camera)
                .unwrap()
                .0
                 .0
                .w_axis
                .z;
            assert!(
                reach(&app, pivot) < 2.0,
                "premise: the wall pulled the arm in on this first frame"
            );
            assert!(
                (world_z - reach(&app, pivot)).abs() < 1e-4,
                "propagate first = {propagate_first}: the camera is at {world_z},                  the arm ends at {}",
                reach(&app, pivot)
            );
        }
    }

    /// Where the character is *this* frame decides the arm: moved next to a
    /// wall, the arm is short on the very next update, not one late.
    #[test]
    fn the_arm_uses_where_the_character_is_now() {
        let mut app = app();
        // A wall only around x = 10.
        wall(
            &mut app,
            Vec3::new(10.0, 1.5, 2.1),
            Vec3::new(1.0, 5.0, 0.1),
        );
        let (character, pivot, _) = rig(&mut app, Vec3::ZERO, SpringArm::default());
        app.update();
        assert_eq!(reach(&app, pivot), 3.0, "premise: away from the wall");
        app.world_mut()
            .get_mut::<Transform>(character)
            .unwrap()
            .position = Vec3::new(10.0, 0.0, 0.0).into();
        app.update();
        assert!(
            reach(&app, pivot) < 2.0,
            "short at once: {}",
            reach(&app, pivot)
        );
    }
}
