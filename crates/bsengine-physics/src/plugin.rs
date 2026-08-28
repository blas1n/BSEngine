use std::sync::Mutex;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use rapier3d::geometry::{CollisionEvent as RapierCollisionEvent, ContactPair};
use rapier3d::pipeline::EventHandler;
use rapier3d::prelude::*;

use crate::{
    components::{
        CharacterBody, Collider, ColliderShape, CollisionEvent, PhysicsHandles, PhysicsInput,
        PhysicsTransform, RigidBody, RigidBodyType,
    },
    world::PhysicsWorld,
};

/// Bevy plugin that inserts a `PhysicsWorld` and steps spawn/simulate/sync systems each frame.
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PhysicsWorld::default());
        app.add_event::<CollisionEvent>();
        // Reflection registration lives here rather than in
        // `bsengine_scene::register_gameplay_reflect_types` because
        // `bsengine-scene` does not (and must not) depend on this crate, and
        // because `PhysicsPlugin` is in both hosts that matter -- the windowed
        // runtime and the headless `bsengine-runtime --test` app -- so the two
        // cannot drift the way they did before that function existed.
        // `RigidBodyType` is a field type, not a component; `register_type`
        // walks type dependencies already, but naming it is what keeps that
        // true if `RigidBody` ever stops being the only thing referring to it.
        app.register_type::<RigidBody>();
        app.register_type::<RigidBodyType>();
        app.register_type::<Collider>();
        app.register_type::<ColliderShape>();
        app.register_type::<PhysicsTransform>();
        app.register_type::<PhysicsInput>();
        app.register_type::<CharacterBody>();
        app.add_systems(
            Update,
            (
                sync_physics_input_from_transform_for_kinematic,
                spawn_bodies,
                step_world,
                sync_from_rapier,
                sync_transform_from_physics,
                // After the transform sync, so the ground ray is cast from
                // where the character actually ended up this step rather than
                // where it was before Rapier resolved the step.
                lock_character_rotation,
                update_grounded,
            )
                .chain()
                .run_if(
                    |paused: Option<bevy_ecs::prelude::Res<bsengine_core::PauseState>>| {
                        !paused.map(|p| p.paused).unwrap_or(false)
                    },
                ),
        );
    }
}

struct CollisionBuffer {
    events: Mutex<Vec<RapierCollisionEvent>>,
}

impl EventHandler for CollisionBuffer {
    fn handle_collision_event(
        &self,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        event: RapierCollisionEvent,
        _contact_pair: Option<&ContactPair>,
    ) {
        self.events.lock().unwrap().push(event);
    }

    fn handle_contact_force_event(
        &self,
        _dt: rapier3d::math::Real,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        _contact_pair: &ContactPair,
        _total_force_magnitude: rapier3d::math::Real,
    ) {
    }
}

fn to_rapier_vec(v: Vec3) -> Vector {
    Vector::new(v.x, v.y, v.z)
}

fn to_rapier_rot(q: Quat) -> Rotation {
    Rotation::from_xyzw(q.x, q.y, q.z, q.w)
}

fn from_rapier_vec(v: Vector) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

fn from_rapier_rot(r: Rotation) -> Quat {
    Quat::from_xyzw(r.x, r.y, r.z, r.w)
}

fn spawn_bodies(
    mut world: ResMut<PhysicsWorld>,
    mut commands: Commands,
    query: Query<(Entity, &RigidBody, &Collider, Option<&PhysicsInput>), Without<PhysicsHandles>>,
) {
    for (entity, rigid_body, collider, input) in query.iter() {
        let pos = input.map(|i| i.position.0).unwrap_or(Vec3::ZERO);
        let rot = input.map(|i| i.rotation.0).unwrap_or(Quat::IDENTITY);

        let pose = Pose::from_parts(to_rapier_vec(pos), to_rapier_rot(rot));

        let rb = match rigid_body.body_type {
            RigidBodyType::Dynamic => RigidBodyBuilder::dynamic()
                .pose(pose)
                .linear_damping(rigid_body.linear_damping)
                .angular_damping(rigid_body.angular_damping)
                .build(),
            RigidBodyType::Static => RigidBodyBuilder::fixed().pose(pose).build(),
            RigidBodyType::KinematicPosition => RigidBodyBuilder::kinematic_position_based()
                .pose(pose)
                .build(),
        };

        let body_handle = world.rigid_body_set.insert(rb);

        let shape = make_shape(&collider.shape);
        let coll = ColliderBuilder::new(shape)
            .restitution(collider.restitution)
            .friction(collider.friction)
            .density(collider.density)
            .sensor(collider.sensor)
            .active_events(ActiveEvents::COLLISION_EVENTS)
            .build();

        let collider_handle = world.add_collider(coll, body_handle);
        world.collider_entity_map.insert(collider_handle, entity);
        world.register_entity_body(entity, body_handle);

        commands.entity(entity).insert((
            PhysicsHandles {
                body_handle,
                collider_handle,
            },
            PhysicsTransform {
                position: pos.into(),
                rotation: rot.into(),
            },
        ));
    }
}

fn step_world(
    mut world: ResMut<PhysicsWorld>,
    query: Query<(&PhysicsHandles, &PhysicsInput), With<RigidBody>>,
    mut collision_events: EventWriter<CollisionEvent>,
) {
    for (handles, input) in query.iter() {
        if let Some(body) = world.rigid_body_set.get_mut(handles.body_handle) {
            if body.is_kinematic() {
                body.set_next_kinematic_position(Pose::from_parts(
                    to_rapier_vec(input.position.0),
                    to_rapier_rot(input.rotation.0),
                ));
            }
        }
    }

    let buffer = CollisionBuffer {
        events: Mutex::new(Vec::new()),
    };
    world.step(&buffer);

    for event in buffer.events.into_inner().unwrap() {
        let (h1, h2, started) = match event {
            RapierCollisionEvent::Started(h1, h2, _) => (h1, h2, true),
            RapierCollisionEvent::Stopped(h1, h2, _) => (h1, h2, false),
        };
        if let (Some(&e1), Some(&e2)) = (
            world.collider_entity_map.get(&h1),
            world.collider_entity_map.get(&h2),
        ) {
            collision_events.send(CollisionEvent {
                entity_a: e1,
                entity_b: e2,
                started,
            });
        }
    }
}

fn sync_from_rapier(
    world: Res<PhysicsWorld>,
    mut query: Query<(&PhysicsHandles, &mut PhysicsTransform)>,
) {
    for (handles, mut transform) in query.iter_mut() {
        if let Some(body) = world.rigid_body_set.get(handles.body_handle) {
            transform.position = from_rapier_vec(body.translation()).into();
            transform.rotation = from_rapier_rot(*body.rotation()).into();
        }
    }
}

/// Copies simulated position/rotation from `PhysicsTransform` (written by
/// `sync_from_rapier`) into the generic `Transform` component that
/// rendering and scripts actually read — without this, physics-driven
/// bodies (falling, forces, impulses) simulate correctly internally but
/// never visibly move, since nothing outside this crate reads
/// `PhysicsTransform`.
///
/// Only applies to `Dynamic` bodies. For `Static`/`Kinematic` bodies,
/// `Transform` is authoritative (scene-authored or script-driven via
/// `Bsengine.setPosition`) and physics follows it, not the other way
/// around — see `sync_physics_input_from_transform_for_kinematic`.
fn sync_transform_from_physics(
    mut query: Query<(&RigidBody, &PhysicsTransform, &mut bsengine_core::Transform)>,
) {
    for (rigid_body, physics_transform, mut transform) in query.iter_mut() {
        if rigid_body.body_type == RigidBodyType::Dynamic {
            transform.position = physics_transform.position;
            transform.rotation = physics_transform.rotation;
        }
    }
}

/// For kinematic bodies, copies the script/scene-authoritative `Transform`
/// into `PhysicsInput` each frame, so `step_world` picks up script-driven
/// movement (e.g. a moving platform using `Bsengine.setPosition`) and
/// Rapier's collision resolution reflects where the body actually is.
/// Dynamic bodies don't need this — their `Transform` is physics-driven,
/// not the other way around.
/// Locks pitch and roll on every character body, leaving yaw free.
///
/// Runs every frame rather than once on insert because a body is registered
/// with Rapier by `spawn_bodies` a frame after the entity appears, so there is
/// no single moment at which "the body now exists" can be observed from here.
/// Setting the same flags repeatedly is idempotent and cheap.
fn lock_character_rotation(
    mut world: ResMut<PhysicsWorld>,
    query: Query<Entity, (With<CharacterBody>, With<PhysicsHandles>)>,
) {
    for entity in query.iter() {
        world.lock_rotations(entity, true, false, true);
    }
}

/// Writes [`CharacterBody::grounded`] from a downward ray under each character.
///
/// The ray starts at the character's origin and is told to ignore that
/// character's own body, so it reports the first *other* surface underneath.
/// A hit steeper than `max_slope_deg` does not count: a character pressed
/// against a wall is touching something, but it is not standing on it.
fn update_grounded(
    world: Res<PhysicsWorld>,
    mut query: Query<(
        Entity,
        &bsengine_core::Transform,
        &Collider,
        &mut CharacterBody,
    )>,
) {
    for (entity, transform, collider, mut character) in query.iter_mut() {
        let origin = transform.position.0;
        // Distance from the body's origin down to the bottom of its shape.
        let half_extent = match &collider.shape {
            ColliderShape::Box { half_extents } => half_extents.y,
            ColliderShape::Sphere { radius } => *radius,
            ColliderShape::Capsule {
                half_height,
                radius,
            } => half_height + radius,
            // Terrain-only shape; a `CharacterBody` is never expected to carry
            // one. No sensible vertical half-extent exists for a height grid,
            // so this contributes no offset rather than guessing at one.
            ColliderShape::Heightfield { .. } => 0.0,
        };
        let reach = half_extent + character.ground_check_distance;

        character.grounded = match world.cast_ray_excluding(origin, Vec3::NEG_Y, reach, entity) {
            Some(hit) => {
                let cos_limit = character.max_slope_deg.to_radians().cos();
                hit.normal.normalize_or_zero().dot(Vec3::Y).abs() >= cos_limit
            }
            None => false,
        };
    }
}

fn sync_physics_input_from_transform_for_kinematic(
    mut query: Query<(&RigidBody, &bsengine_core::Transform, &mut PhysicsInput)>,
) {
    for (rigid_body, transform, mut input) in query.iter_mut() {
        if rigid_body.body_type == RigidBodyType::KinematicPosition {
            input.position = transform.position;
            input.rotation = transform.rotation;
        }
    }
}

pub(crate) fn make_shape(shape: &ColliderShape) -> SharedShape {
    match shape {
        ColliderShape::Box { half_extents } => {
            SharedShape::cuboid(half_extents.x, half_extents.y, half_extents.z)
        }
        ColliderShape::Sphere { radius } => SharedShape::ball(*radius),
        ColliderShape::Capsule {
            half_height,
            radius,
        } => SharedShape::capsule_y(*half_height, *radius),
        ColliderShape::Heightfield {
            heights,
            rows,
            cols,
            scale,
        } => {
            // `rapier3d`'s heightfield takes a `parry3d::utils::Array2`, not a
            // `nalgebra::DMatrix` -- verified against this workspace's pinned
            // rapier3d 0.33.0 / parry3d 0.28.0 by reading the vendored source
            // (no `DMatrix` overload exists for the "dim3" build this crate
            // uses). `Array2` stores column-major (`flat_index(i, j) = i + j *
            // nrows`), so building it via `from_fn` -- rather than handing our
            // row-major `Vec<f32>` to `Array2::new` directly -- is what keeps
            // row/column indices from ending up transposed.
            let grid = rapier3d::parry::utils::Array2::from_fn(*rows, *cols, |i, j| {
                heights[i * (*cols) + j]
            });
            SharedShape::heightfield(grid, Vector::new(scale.x, scale.y, scale.z))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Collider, PhysicsInput, RigidBody};
    use bsengine_app::new_app;
    use bsengine_core::Transform;

    #[test]
    fn dynamic_body_falls_and_updates_transform_component() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);

        let start = Vec3::new(0.0, 5.0, 0.0);
        app.world_mut().spawn((
            Transform::from_position(start),
            RigidBody::dynamic(),
            Collider::ball(0.5),
            PhysicsInput {
                position: start.into(),
                rotation: Quat::IDENTITY.into(),
            },
        ));

        for _ in 0..30 {
            app.update();
        }

        let mut query = app.world_mut().query::<&Transform>();
        let transform = query.iter(app.world()).next().unwrap();
        assert!(
            transform.position.0.y < start.y,
            "expected the dynamic body to fall under gravity and for Transform to \
             reflect it via PhysicsTransform sync, got y={}",
            transform.position.0.y
        );
    }

    #[test]
    fn dynamic_body_does_not_fall_while_paused() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        app.insert_resource(bsengine_core::PauseState { paused: true });

        let start = Vec3::new(0.0, 5.0, 0.0);
        app.world_mut().spawn((
            Transform::from_position(start),
            RigidBody::dynamic(),
            Collider::ball(0.5),
            PhysicsInput {
                position: start.into(),
                rotation: Quat::IDENTITY.into(),
            },
        ));

        for _ in 0..30 {
            app.update();
        }

        let mut query = app.world_mut().query::<&Transform>();
        let transform = query.iter(app.world()).next().unwrap();
        assert_eq!(
            transform.position.0.y, start.y,
            "expected the dynamic body to stay in place while paused, got y={}",
            transform.position.0.y
        );
    }

    #[test]
    fn kinematic_body_transform_is_authoritative_not_overwritten_by_physics() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);

        let start = Vec3::new(0.0, 0.0, 0.0);
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_position(start),
                RigidBody::kinematic(),
                Collider::cuboid(1.0, 0.25, 1.0),
                PhysicsInput {
                    position: start.into(),
                    rotation: Quat::IDENTITY.into(),
                },
            ))
            .id();

        app.update();

        // Simulate a script moving the platform via Bsengine.setPosition.
        let moved = Vec3::new(5.0, 0.0, 0.0);
        app.world_mut()
            .get_mut::<Transform>(entity)
            .unwrap()
            .position = moved.into();

        app.update();
        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert_eq!(
            transform.position.0, moved,
            "kinematic body's script-driven Transform should not be reverted by physics sync"
        );
    }

    #[test]
    fn kinematic_body_physics_input_tracks_transform_each_frame() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);

        let start = Vec3::new(0.0, 0.0, 0.0);
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_position(start),
                RigidBody::kinematic(),
                Collider::cuboid(1.0, 0.25, 1.0),
                PhysicsInput {
                    position: start.into(),
                    rotation: Quat::IDENTITY.into(),
                },
            ))
            .id();

        app.update();

        let moved = Vec3::new(5.0, 0.0, 0.0);
        app.world_mut()
            .get_mut::<Transform>(entity)
            .unwrap()
            .position = moved.into();

        app.update();

        let input = app.world().get::<PhysicsInput>(entity).unwrap();
        assert_eq!(
            input.position.0, moved,
            "kinematic body's PhysicsInput should track its script-driven Transform"
        );
    }

    // ---- CharacterBody (roadmap item 27) ---------------------------------

    /// Spawns a static floor slab centred at the origin, top surface at y = 0.
    fn spawn_floor(app: &mut bevy_app::App) {
        app.world_mut().spawn((
            Transform::from_position(Vec3::new(0.0, -0.5, 0.0)),
            RigidBody::fixed(),
            Collider::cuboid(10.0, 0.5, 10.0),
            PhysicsInput {
                position: Vec3::new(0.0, -0.5, 0.0).into(),
                rotation: Quat::IDENTITY.into(),
            },
            PhysicsTransform::default(),
        ));
    }

    fn spawn_character(app: &mut bevy_app::App, at: Vec3) -> bevy_ecs::entity::Entity {
        app.world_mut()
            .spawn((
                Transform::from_position(at),
                RigidBody::dynamic(),
                Collider::capsule(0.5, 0.3),
                CharacterBody::default(),
                PhysicsInput {
                    position: at.into(),
                    rotation: Quat::IDENTITY.into(),
                },
                PhysicsTransform::default(),
            ))
            .id()
    }

    #[test]
    fn a_character_standing_on_the_floor_is_grounded() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        spawn_floor(&mut app);
        // Capsule half-height 0.5 + radius 0.3, so its base sits at y - 0.8.
        let character = spawn_character(&mut app, Vec3::new(0.0, 0.8, 0.0));

        for _ in 0..30 {
            app.update();
        }

        assert!(
            app.world()
                .get::<CharacterBody>(character)
                .unwrap()
                .grounded,
            "a character resting on the floor should report grounded"
        );
    }

    #[test]
    fn a_character_in_the_air_is_not_grounded() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        spawn_floor(&mut app);
        let character = spawn_character(&mut app, Vec3::new(0.0, 20.0, 0.0));

        // One step: far above the floor and barely moved, so nothing is under it.
        app.update();

        assert!(
            !app.world()
                .get::<CharacterBody>(character)
                .unwrap()
                .grounded,
            "a character 20 units up should not report grounded"
        );
    }

    #[test]
    fn the_ground_ray_does_not_hit_the_character_itself() {
        // The failure this guards is silent: a ray that starts inside the
        // character's own capsule hits it immediately, and every character
        // reports grounded forever -- including ones falling through empty
        // space. The air test above only catches it if the exclusion works,
        // so this asserts the mechanism directly with no floor present at all.
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        let character = spawn_character(&mut app, Vec3::new(0.0, 5.0, 0.0));

        app.update();

        assert!(
            !app.world()
                .get::<CharacterBody>(character)
                .unwrap()
                .grounded,
            "with no floor in the world at all, nothing can be underfoot"
        );
    }

    #[test]
    fn a_character_capsule_does_not_tip_over() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        spawn_floor(&mut app);
        let character = spawn_character(&mut app, Vec3::new(0.0, 0.8, 0.0));

        // One step first: `spawn_bodies` runs in `Update`, so until it has, the
        // entity has no Rapier body and any impulse aimed at it is silently
        // dropped. An earlier version of this test applied the impulse before
        // this update and passed with the rotation lock removed -- it was
        // asserting that nothing happens to a body that was never pushed.
        app.update();

        // A torque impulse about Z, which is exactly what the lock refuses. A
        // *linear* impulse would prove nothing either: `apply_impulse` acts at
        // the centre of mass and generates no torque at all.
        app.world_mut()
            .resource_mut::<PhysicsWorld>()
            .apply_torque_impulse(character, Vec3::new(0.0, 0.0, 30.0));

        for _ in 0..60 {
            app.update();
        }

        let rotation = app.world().get::<Transform>(character).unwrap().rotation.0;
        let up = rotation * Vec3::Y;
        assert!(
            up.dot(Vec3::Y) > 0.99,
            "character should still be upright; up vector is {up:?}"
        );
    }

    /// Spawns a dynamic ball with nothing else in the world, one step in, so it
    /// already has a Rapier body. Force is applied along X, where gravity is
    /// not, so the numbers below are the force's doing alone.
    fn spawn_free_body(app: &mut App) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_position(Vec3::ZERO),
                RigidBody::dynamic(),
                Collider::ball(0.5),
                PhysicsInput {
                    position: Vec3::ZERO.into(),
                    rotation: Quat::IDENTITY.into(),
                },
            ))
            .id();
        app.update();
        entity
    }

    #[test]
    fn the_same_force_every_step_produces_the_same_acceleration_every_step() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        let body = spawn_free_body(&mut app);

        // Hold a constant force for four steps and record what each step adds.
        let mut deltas = Vec::new();
        let mut previous = 0.0;
        for _ in 0..4 {
            app.world_mut()
                .resource_mut::<PhysicsWorld>()
                .apply_force(body, Vec3::X * 10.0);
            app.update();
            let vx = app
                .world()
                .resource::<PhysicsWorld>()
                .get_linvel(body)
                .unwrap()
                .x;
            deltas.push(vx - previous);
            previous = vx;
        }

        // F = ma with a constant F is a constant a, so every step adds the same
        // velocity. Rapier does not reset forces itself: without the reset in
        // `PhysicsWorld::step`, step n carries the sum of steps 1..n and these
        // deltas come out 1:2:3:4 -- the last one four times the first.
        let first = deltas[0];
        assert!(first > 0.0, "the force should have moved the body at all");
        for (i, delta) in deltas.iter().enumerate() {
            assert!(
                (delta - first).abs() < first * 0.01,
                "step {} added {delta} but step 1 added {first}; a constant force \
                 must not accelerate harder the longer it is held. All deltas: {deltas:?}",
                i + 1
            );
        }
    }

    #[test]
    fn the_same_torque_every_step_produces_the_same_angular_acceleration() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        let body = spawn_free_body(&mut app);

        let mut deltas = Vec::new();
        let mut previous = 0.0;
        for _ in 0..4 {
            app.world_mut()
                .resource_mut::<PhysicsWorld>()
                .add_torque(body, Vec3::Y * 10.0);
            app.update();
            let wy = app
                .world()
                .resource::<PhysicsWorld>()
                .get_angvel(body)
                .unwrap()
                .y;
            deltas.push(wy - previous);
            previous = wy;
        }

        let first = deltas[0];
        assert!(first > 0.0, "the torque should have spun the body at all");
        for (i, delta) in deltas.iter().enumerate() {
            assert!(
                (delta - first).abs() < first * 0.01,
                "step {} added {delta} but step 1 added {first}; torque accumulates \
                 the same way force does. All deltas: {deltas:?}",
                i + 1
            );
        }
    }

    #[test]
    fn a_force_applied_once_acts_once() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        let body = spawn_free_body(&mut app);

        app.world_mut()
            .resource_mut::<PhysicsWorld>()
            .apply_force(body, Vec3::X * 10.0);
        app.update();
        let after_one = app
            .world()
            .resource::<PhysicsWorld>()
            .get_linvel(body)
            .unwrap()
            .x;

        // Nothing applies force now. A body coasting with no damping keeps its
        // velocity; a body still being pushed by last frame's force gains more.
        for _ in 0..10 {
            app.update();
        }
        let after_eleven = app
            .world()
            .resource::<PhysicsWorld>()
            .get_linvel(body)
            .unwrap()
            .x;

        assert!(
            (after_eleven - after_one).abs() < after_one * 0.01,
            "one call to apply_force should push for one step, but velocity went \
             from {after_one} to {after_eleven} over ten further steps with no \
             force applied"
        );
    }

    // ---- Heightfield collider (roadmap item 44, terrain core) ------------

    /// Spawns a static heightfield body: flat at height 2.0 everywhere, over
    /// a 10x10 world-space horizontal extent centred on the origin. Two rows
    /// and two columns is the smallest grid rapier's heightfield accepts (one
    /// quad), which is all a flat surface needs.
    fn spawn_flat_heightfield(app: &mut App) {
        app.world_mut().spawn((
            Transform::from_position(Vec3::ZERO),
            RigidBody::fixed(),
            Collider {
                shape: ColliderShape::Heightfield {
                    heights: vec![2.0, 2.0, 2.0, 2.0],
                    rows: 2,
                    cols: 2,
                    scale: Vec3::new(10.0, 1.0, 10.0).into(),
                },
                restitution: 0.0,
                friction: 0.5,
                density: 1.0,
                sensor: false,
            },
            PhysicsInput {
                position: Vec3::ZERO.into(),
                rotation: Quat::IDENTITY.into(),
            },
            PhysicsTransform::default(),
        ));
    }

    #[test]
    fn heightfield_collider_supports_a_dynamic_body_at_the_expected_height() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        spawn_flat_heightfield(&mut app);

        let radius = 0.5;
        let start = Vec3::new(0.0, 6.0, 0.0);
        let sphere = app
            .world_mut()
            .spawn((
                Transform::from_position(start),
                RigidBody::dynamic(),
                Collider::ball(radius),
                PhysicsInput {
                    position: start.into(),
                    rotation: Quat::IDENTITY.into(),
                },
            ))
            .id();

        for _ in 0..150 {
            app.update();
        }

        let y = app.world().get::<Transform>(sphere).unwrap().position.0.y;
        let expected = 2.0 + radius;
        assert!(
            (y - expected).abs() < 0.05,
            "expected the dynamic sphere to come to rest on the flat heightfield \
             (all heights = 2.0) at y ~= {expected}, but it settled at y={y} -- \
             either it fell through the collider entirely or the shape is not \
             where the height data says it should be"
        );
    }
}
