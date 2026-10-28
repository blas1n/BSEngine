//! A camera boom that pulls in when something is in the way.

use bevy_ecs::prelude::*;
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use bsengine_core::{ChildOf, Transform};
use glam::{Mat4, Quat, Vec3};

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
    /// How quickly the arm catches up when the pivot moves: each frame it
    /// closes `lag_speed * dt` of the gap (Unreal's `CameraLagSpeed`, whose
    /// default once enabled is 10). 0 is no lag -- the arm moves rigidly with
    /// the pivot, as it always has. Godot's `SpringArm3D` has no lag;
    /// Unity puts it in Cinemachine's damping.
    pub lag_speed: f32,
    /// The same for turning (`CameraRotationLagSpeed`): the arm's direction,
    /// and its children's rotation with it, follows the pivot's. 0 is no lag.
    pub rotation_lag_speed: f32,
    /// The furthest the lagging arm may fall behind the pivot
    /// (`CameraLagMaxDistance`), so a teleport carries the camera along
    /// instead of sweeping it across the level. 0 is no limit.
    pub lag_max_distance: f32,
    /// Where the lag has got to; `None` until an arm with lag has run. Not
    /// authored -- per-frame state, like `current_length`.
    #[reflect(ignore)]
    pub lag: Option<SpringArmLag>,
}

/// A lagging arm's state between frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringArmLag {
    /// The lagged pivot position, in world space.
    pub origin: Vec3,
    /// The lagged pivot rotation, in world space.
    pub rotation: Quat,
    /// The rotation last applied to the children on top of their own: the
    /// lagged rotation relative to the pivot's. Taken back out before the
    /// next frame's is put in, so a child's authored rotation survives.
    pub applied: Quat,
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
            lag_speed: 0.0,
            rotation_lag_speed: 0.0,
            lag_max_distance: 0.0,
            lag: None,
        }
    }
}

/// This entity's world matrix, from the `Transform`s up its `Parent` chain as
/// they are now -- not `GlobalTransform`, which is last frame's until this
/// frame's propagation runs after the arm.
fn world_matrix(entity: Entity, transforms: &Query<(&Transform, Option<&ChildOf>)>) -> Mat4 {
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
    transforms: &Query<(&Transform, Option<&ChildOf>)>,
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

/// Where one arm's children go this frame.
enum Placement {
    /// No lag: `(0, 0, reach)` in the pivot's own space, as the arm always
    /// placed them; `undo` takes back a rotation a lag that was just switched
    /// off had applied to them.
    Rigid { reach: f32, undo: Option<Quat> },
    /// Lagging: a point in the pivot's space that is not on its +Z axis, and
    /// the rotation the lag turns them by, replacing last frame's.
    Lagged {
        position: Vec3,
        applied: Quat,
        previous: Quat,
    },
}

/// The sweep from `origin` along `dir` for `span`: how far the children can
/// go before the world is in the way, `margin` short of it.
fn swept_reach(
    physics: Option<&PhysicsWorld>,
    arm: &SpringArm,
    origin: Vec3,
    dir: Vec3,
    span: f32,
    exclude: &[Entity],
) -> f32 {
    if !arm.collision {
        return span;
    }
    physics
        .filter(|_| dir != Vec3::ZERO)
        .and_then(|world| world.cast_sphere_excluding(origin, dir, span, arm.probe_radius, exclude))
        .map_or(span, |hit| (hit - arm.margin).clamp(0.0, span))
}

/// Sets every spring arm's length for this frame and moves its children to
/// the end of it. Runs before transforms are propagated, so the camera's
/// world position this frame is the arm's.
#[allow(clippy::type_complexity)] // a ParamSet of two queries is the type
pub fn update_spring_arms(
    physics: Option<Res<PhysicsWorld>>,
    time: Option<Res<bsengine_core::Time>>,
    mut arms: Query<(Entity, &mut SpringArm)>,
    children: Query<(Entity, &ChildOf)>,
    mut transforms: ParamSet<(Query<(&Transform, Option<&ChildOf>)>, Query<&mut Transform>)>,
) {
    let dt = time.as_ref().map_or(0.0, |t| t.delta_seconds);
    let mut placed: Vec<(Entity, Placement)> = Vec::new();
    for (arm_entity, mut arm) in arms.iter_mut() {
        let length = arm.length.max(0.0);
        let read = transforms.p0();
        let world = world_matrix(arm_entity, &read);
        let origin = world.transform_point3(Vec3::ZERO);
        let mut exclude = with_ancestors(arm_entity, &read);
        exclude.extend(
            children
                .iter()
                .filter(|(_, p)| p.0 == arm_entity)
                .map(|(e, _)| e),
        );

        if arm.lag_speed <= 0.0 && arm.rotation_lag_speed <= 0.0 {
            let dir = world.transform_vector3(Vec3::Z).normalize_or_zero();
            let reach = swept_reach(physics.as_deref(), &arm, origin, dir, length, &exclude);
            arm.current_length = reach;
            let undo = arm.lag.take().map(|lag| lag.applied.inverse());
            placed.push((arm_entity, Placement::Rigid { reach, undo }));
            continue;
        }

        // Unreal's `UpdateDesiredArmLocation`: the arm's rotation and its
        // origin each close a share of the gap to the pivot's, and the arm
        // hangs back from the lagged origin along the lagged rotation. The
        // first lagging frame starts where the pivot is, not at the world
        // origin.
        let (_, pivot_rotation, _) = world.to_scale_rotation_translation();
        let previous = arm.lag.unwrap_or(SpringArmLag {
            origin,
            rotation: pivot_rotation,
            applied: Quat::IDENTITY,
        });
        let rotation = if arm.rotation_lag_speed > 0.0 {
            previous.rotation.slerp(
                pivot_rotation,
                (dt * arm.rotation_lag_speed).clamp(0.0, 1.0),
            )
        } else {
            pivot_rotation
        };
        let mut lagged_origin = if arm.lag_speed > 0.0 {
            previous
                .origin
                .lerp(origin, (dt * arm.lag_speed).clamp(0.0, 1.0))
        } else {
            origin
        };
        let behind = lagged_origin - origin;
        if arm.lag_max_distance > 0.0 && behind.length() > arm.lag_max_distance {
            lagged_origin = origin + behind.normalize() * arm.lag_max_distance;
        }
        let end = lagged_origin + rotation * Vec3::Z * length;

        // Swept from where the pivot *is*, not from the lagged origin: the
        // lag is the camera's, and the space between the character and the
        // camera is what must stay clear. Pulling in is never lagged --
        // Unreal does the same -- or the camera would sit inside a wall for
        // as long as it took to catch up.
        let to_end = end - origin;
        let span = to_end.length();
        let dir = to_end.normalize_or_zero();
        let reach = swept_reach(physics.as_deref(), &arm, origin, dir, span, &exclude);
        arm.current_length = reach;

        let applied = pivot_rotation.inverse() * rotation;
        placed.push((
            arm_entity,
            Placement::Lagged {
                position: world.inverse().transform_point3(origin + dir * reach),
                applied,
                previous: previous.applied,
            },
        ));
        arm.lag = Some(SpringArmLag {
            origin: lagged_origin,
            rotation,
            applied,
        });
    }
    let mut write = transforms.p1();
    for (child, parent) in children.iter() {
        let Some((_, placement)) = placed.iter().find(|(arm, _)| *arm == parent.0) else {
            continue;
        };
        let Ok(mut transform) = write.get_mut(child) else {
            continue;
        };
        match *placement {
            Placement::Rigid { reach, undo } => {
                transform.position = Vec3::new(0.0, 0.0, reach).into();
                if let Some(undo) = undo {
                    transform.rotation = (undo * transform.rotation.0).normalize().into();
                }
            }
            Placement::Lagged {
                position,
                applied,
                previous,
            } => {
                transform.position = position.into();
                // The child's own rotation, with last frame's lag taken back
                // out and this frame's put in: a camera tilted down by its
                // author stays tilted down, turning with the lagged arm.
                // Normalised, because this is composed onto itself every
                // frame: a product of unit quaternions drifts off unit length
                // by rounding, and a long session would give the camera a
                // scale.
                transform.rotation = (applied * previous.inverse() * transform.rotation.0)
                    .normalize()
                    .into();
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
                ChildOf(character),
                arm,
            ))
            .id();
        let camera = app
            .world_mut()
            .spawn((Transform::default(), ChildOf(pivot)))
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

    // --- lag ---

    /// An app whose clock advances exactly 0.1 s a frame, so a lag speed of 5
    /// closes exactly half the gap each frame.
    fn lag_app() -> bevy_app::App {
        let mut app = app();
        app.add_plugins(bsengine_app::TimePlugin);
        app.insert_resource(bsengine_core::Time::fixed(0.1));
        app
    }

    /// A rig with no colliders of its own: a character, a pivot 1.5 up on it
    /// carrying `arm`, and a camera on the pivot turned by `camera_rotation`.
    fn bare_rig(
        app: &mut bevy_app::App,
        arm: SpringArm,
        camera_rotation: Quat,
    ) -> (Entity, Entity, Entity) {
        let character = app.world_mut().spawn(Transform::default()).id();
        let pivot = app
            .world_mut()
            .spawn((
                Transform::from_position(Vec3::new(0.0, 1.5, 0.0)),
                ChildOf(character),
                arm,
            ))
            .id();
        let camera = app
            .world_mut()
            .spawn((
                Transform {
                    rotation: camera_rotation.into(),
                    ..Default::default()
                },
                ChildOf(pivot),
            ))
            .id();
        (character, pivot, camera)
    }

    /// Where `entity` is in the world, through its parents as they are now.
    fn world_of(app: &mut bevy_app::App, entity: Entity) -> (Vec3, Quat) {
        let world = app.world();
        let mut m = Mat4::IDENTITY;
        let mut current = Some(entity);
        while let Some(e) = current {
            m = world.get::<Transform>(e).unwrap().to_matrix() * m;
            current = world.get::<ChildOf>(e).map(|p| p.0);
        }
        let (_, rotation, position) = m.to_scale_rotation_translation();
        (position, rotation)
    }

    fn move_to(app: &mut bevy_app::App, entity: Entity, at: Vec3) {
        app.world_mut()
            .get_mut::<Transform>(entity)
            .unwrap()
            .position = at.into();
    }

    /// With lag, the camera closes its share of the gap each frame -- half at
    /// speed 5 and 0.1 s -- instead of jumping with the character; without
    /// it, the premise, it jumps.
    #[test]
    fn position_lag_closes_its_share_of_the_gap_each_frame() {
        let mut app = lag_app();
        let lagging = SpringArm {
            lag_speed: 5.0,
            ..SpringArm::default()
        };
        let (character, _, camera) = bare_rig(&mut app, lagging, Quat::IDENTITY);
        let (rigid_character, _, rigid_camera) =
            bare_rig(&mut app, SpringArm::default(), Quat::IDENTITY);
        app.update();
        app.update();
        let start = world_of(&mut app, camera).0;
        assert!(
            start.distance(Vec3::new(0.0, 1.5, 3.0)) < 1e-4,
            "settled behind: {start}"
        );

        move_to(&mut app, character, Vec3::new(10.0, 0.0, 0.0));
        move_to(&mut app, rigid_character, Vec3::new(10.0, 0.0, 0.0));
        app.update();
        assert!(
            (world_of(&mut app, rigid_camera).0.x - 10.0).abs() < 1e-4,
            "premise: without lag the camera jumps with the character"
        );
        let x = world_of(&mut app, camera).0.x;
        assert!((x - 5.0).abs() < 1e-3, "half the gap after one frame: {x}");
        app.update();
        let x = world_of(&mut app, camera).0.x;
        assert!((x - 7.5).abs() < 1e-3, "and half the rest after two: {x}");
    }

    /// A teleport moves the camera with it, no further than
    /// `lag_max_distance` behind.
    #[test]
    fn lag_max_distance_keeps_the_camera_close() {
        let mut app = lag_app();
        let arm = SpringArm {
            lag_speed: 5.0,
            lag_max_distance: 1.0,
            ..SpringArm::default()
        };
        let (character, _, camera) = bare_rig(&mut app, arm, Quat::IDENTITY);
        app.update();
        move_to(&mut app, character, Vec3::new(10.0, 0.0, 0.0));
        app.update();
        let x = world_of(&mut app, camera).0.x;
        assert!((x - 9.0).abs() < 1e-3, "one unit behind, not five: {x}");
    }

    /// Rotation lag turns the arm part way -- 45 degrees of a quarter turn
    /// after one frame at half -- and the camera with it, keeping the tilt
    /// its author gave it; caught up, the camera's own rotation is that tilt
    /// again.
    #[test]
    fn rotation_lag_turns_the_arm_and_the_camera_part_way() {
        let mut app = lag_app();
        let arm = SpringArm {
            rotation_lag_speed: 5.0,
            ..SpringArm::default()
        };
        let tilt = Quat::from_rotation_x(-0.3);
        let (_, pivot, camera) = bare_rig(&mut app, arm, tilt);
        app.update();
        let quarter = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        app.world_mut()
            .get_mut::<Transform>(pivot)
            .unwrap()
            .rotation = quarter.into();
        app.update();

        let half = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        let (at, facing) = world_of(&mut app, camera);
        let expected = Vec3::new(0.0, 1.5, 0.0) + half * Vec3::Z * 3.0;
        assert!(
            at.distance(expected) < 1e-3,
            "half way round: {at} (expected {expected})"
        );
        assert!(
            facing.angle_between(half * tilt) < 1e-3,
            "turned with the arm, tilt kept"
        );

        for _ in 0..60 {
            app.update();
        }
        let local = app.world().get::<Transform>(camera).unwrap().rotation.0;
        assert!(
            local.angle_between(tilt) < 1e-3,
            "caught up, the camera is its own tilt again: {local} vs {tilt}"
        );
    }

    /// Switching the lag off puts the children back on the arm at once, with
    /// the rotation the lag had added taken back out.
    #[test]
    fn switching_lag_off_returns_the_camera_to_the_arm() {
        let mut app = lag_app();
        let arm = SpringArm {
            rotation_lag_speed: 5.0,
            ..SpringArm::default()
        };
        let tilt = Quat::from_rotation_x(-0.3);
        let (_, pivot, camera) = bare_rig(&mut app, arm, tilt);
        app.update();
        app.world_mut()
            .get_mut::<Transform>(pivot)
            .unwrap()
            .rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).into();
        app.update();
        let mid = app.world().get::<Transform>(camera).unwrap().rotation.0;
        assert!(
            mid.angle_between(tilt) > 0.5,
            "premise: mid-lag the camera is turned"
        );

        app.world_mut()
            .get_mut::<SpringArm>(pivot)
            .unwrap()
            .rotation_lag_speed = 0.0;
        app.update();
        let t = app.world().get::<Transform>(camera).unwrap();
        assert!(
            t.rotation.0.angle_between(tilt) < 1e-4,
            "the tilt alone again: {} vs {tilt}",
            t.rotation.0
        );
        assert!(
            t.position.0.distance(Vec3::new(0.0, 0.0, 3.0)) < 1e-4,
            "on the arm again"
        );
    }

    /// The lag is the camera's; the space between it and the character is
    /// swept now. A character that has just come back past a wall leaves its
    /// lagged origin on the far side of it -- the sweep from the character
    /// stops the camera at the wall instead. Sweeping from the lagged origin
    /// would leave the camera behind the wall.
    #[test]
    fn a_lagging_camera_is_not_left_behind_a_wall() {
        let mut app = lag_app();
        // A wall across x = 3: between the character, back at x = 0, and its
        // lagged origin one frame later, half way back at x = 5.
        wall(
            &mut app,
            Vec3::new(3.0, 1.5, 0.0),
            Vec3::new(0.1, 5.0, 10.0),
        );
        let arm = SpringArm {
            lag_speed: 5.0,
            ..SpringArm::default()
        };
        let (character, pivot, camera) = bare_rig(&mut app, arm, Quat::IDENTITY);
        move_to(&mut app, character, Vec3::new(10.0, 0.0, 0.0));
        app.update();
        app.update();
        app.update();
        // Settled at x = 10, past the wall; now back to x = 0 in one step.
        move_to(&mut app, character, Vec3::ZERO);
        app.update();
        let lag = app.world().get::<SpringArm>(pivot).unwrap().lag.unwrap();
        assert!(
            lag.origin.x > 3.1,
            "premise: the lagged origin is beyond the wall ({})",
            lag.origin
        );
        let x = world_of(&mut app, camera).0.x;
        assert!(
            x < 2.9,
            "the camera stays on the character's side of the wall: {x}"
        );
    }
}
