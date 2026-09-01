use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bsengine_gltf::SkinnedMesh;
use glam::{Quat, Vec3};
use rapier3d::geometry::{CollisionEvent as RapierCollisionEvent, ContactPair};
use rapier3d::pipeline::EventHandler;
use rapier3d::prelude::*;

use crate::{
    components::{
        CharacterBody, Collider, ColliderShape, CollisionEvent, Joint, PhysicsHandles,
        PhysicsInput, PhysicsTransform, Ragdoll, RagdollBone, RigidBody, RigidBodyType,
    },
    ragdoll::plan_bones,
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
                // Before `spawn_bodies`, so the bone entities it spawns get
                // their Rapier bodies on the same frame the ragdoll switched
                // on, and `sync_joints` below can link them that same frame
                // rather than leaving the skeleton loose for one step.
                sync_ragdolls,
                spawn_bodies,
                // After `spawn_bodies`, because the collider whose groups this
                // narrows does not exist until then.
                isolate_ragdoll_bones,
                // After `spawn_bodies`, so a joint authored alongside its two
                // bodies in the same scene is created on the frame those
                // bodies register rather than the frame after; before
                // `step_world`, so it constrains that very step.
                sync_joints,
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

/// Makes the simulation's joints match the [`Joint`] components in the world.
///
/// Mirrors [`spawn_bodies`]: a component says what the simulation should
/// contain, and this is the pass that makes it so. It retries every frame
/// rather than reacting once to insertion, because on the frame a scene's
/// `Joint` first exists neither end has a Rapier body yet -- `spawn_bodies`
/// creates those from the same scene's `RigidBody`/`Collider`, and a
/// create-once-on-insert design would silently drop every scene-authored
/// joint.
///
/// The removal half is the part with no immediate symptom: a joint whose
/// target has despawned keeps constraining a body nothing points at any more,
/// and nothing fails at the moment it happens.
fn sync_joints(
    mut world: ResMut<PhysicsWorld>,
    // What this pass has already built, and against which body -- the one
    // question neither the world nor the components can answer once something
    // has gone away. `Joint.body_b` stops naming the pair that was actually
    // inserted the moment the component is removed, its entity despawns, or
    // `body_b` is re-pointed somewhere else, and all three are exactly when
    // the joint has to come out.
    //
    // Deliberately a `Local` rather than a marker component: it is this
    // system's own bookkeeping and no part of any entity's public shape, and
    // an engine-wide component catalogue should not grow an entry for it.
    mut created: Local<HashMap<Entity, Entity>>,
    joints: Query<(Entity, &Joint)>,
    // Every entity, purely to ask whether one still exists. `PhysicsWorld`
    // cannot answer that: `entity_body_map` keeps a despawned entity's handle,
    // so `add_joint` would happily re-joint a ghost.
    alive: Query<Entity>,
) {
    // Drop what the components no longer ask for. A lookup that misses covers
    // both an entity that despawned and one that merely lost its `Joint` --
    // neither is in `joints` any more, and a despawned entity is in no query
    // at all, which is why this is driven from the bookkeeping rather than
    // from the world.
    created.retain(|&entity, &mut body_b| {
        let still_wanted =
            joints.get(entity).is_ok_and(|(_, j)| j.body_b == body_b) && alive.get(body_b).is_ok();
        if !still_wanted {
            world.remove_joint(entity, body_b);
        }
        still_wanted
    });

    for (entity, joint) in joints.iter() {
        if created.contains_key(&entity) || alive.get(joint.body_b).is_err() {
            continue;
        }
        // False means one of the two bodies is not registered with Rapier yet;
        // the next frame tries again.
        if world.add_joint(
            entity,
            joint.body_b,
            &joint.kind,
            joint.anchor_a.0,
            joint.anchor_b.0,
        ) {
            created.insert(entity, joint.body_b);
        }
    }
}

/// The collision group a ragdoll's own bone colliders belong to — and the one
/// they are filtered *out* of, so a character's bones never collide with each
/// other.
///
/// They have to be. Adjacent bone capsules share an endpoint by construction,
/// so every joint in the skeleton is also a contact at full penetration depth:
/// the solver shoves the two bones apart while the joint hauls them back, and
/// the ragdoll shakes itself to pieces. Self-collision between a character's
/// own bones is out of scope for this item, and this is what "out of scope"
/// has to mean mechanically.
///
/// Only the bones' *filter* drops the group, not their membership, so a bone
/// still collides with everything else in the world: Rapier requires both
/// sides to accept, and an ordinary collider's default filter is `ALL`.
const RAGDOLL_GROUP: Group = Group::GROUP_32;

/// What [`sync_ragdolls`] built for one ragdoll, so switching it off can take
/// away exactly what switching it on put there.
struct BuiltRagdoll {
    /// `(node index in `SkinnedMesh.nodes`, bone body entity)`.
    bones: Vec<(usize, Entity)>,
}

/// Builds a ragdoll's bone bodies and joints when it becomes active, and
/// removes them when it stops.
///
/// Mirrors [`sync_joints`]: the component says what the simulation should
/// contain and this is the pass that makes it so. The bone entities are
/// ordinary physics entities — `RigidBody`, `Collider`, `PhysicsInput`, and a
/// `Joint` naming the bone above — so [`spawn_bodies`] and [`sync_joints`]
/// bring them into Rapier and take their constraints away again, rather than
/// this growing a private copy of both.
///
/// A `Local` rather than a component holds the bookkeeping, for the reason
/// `sync_joints` gives for its own: which entities this pass created is its
/// business alone, and no part of any entity's authored shape.
fn sync_ragdolls(
    mut commands: Commands,
    mut world: ResMut<PhysicsWorld>,
    mut built: Local<HashMap<Entity, BuiltRagdoll>>,
    // Warned-about entities, so a misconfigured ragdoll says so once instead
    // of once per frame forever. Cleared again if it later gets a skeleton.
    mut warned: Local<HashSet<Entity>>,
    ragdolls: Query<(Entity, &Ragdoll, Option<&SkinnedMesh>)>,
) {
    // Tear down first, so a ragdoll switched off and on again in the same
    // frame is rebuilt rather than left with the old bodies. A lookup that
    // misses covers an entity that despawned and one that merely lost its
    // `Ragdoll`; neither is in `ragdolls` any more.
    let stale: Vec<Entity> = built
        .keys()
        .copied()
        .filter(|&owner| !ragdolls.get(owner).is_ok_and(|(_, r, _)| r.active))
        .collect();
    for owner in stale {
        for (_, bone) in built.remove(&owner).into_iter().flat_map(|b| b.bones) {
            // Despawning alone would leave the Rapier body simulating: nothing
            // removes it, and it would go on falling through the level and
            // reporting contacts under a collider handle that maps to nothing.
            world.remove_body(bone);
            commands.entity(bone).despawn();
        }
        warned.remove(&owner);
    }

    for (owner, ragdoll, skinned) in ragdolls.iter() {
        if !ragdoll.active || built.contains_key(&owner) {
            continue;
        }
        let Some(skinned) = skinned else {
            if warned.insert(owner) {
                tracing::warn!(
                    "ragdoll: {owner:?} has an active Ragdoll but no SkinnedMesh, so there \
                     is no skeleton to build bodies from; doing nothing"
                );
            }
            continue;
        };
        warned.remove(&owner);

        let radius = ragdoll.bone_radius.max(1.0e-3);
        let plans = plan_bones(&skinned.nodes, radius, ragdoll.total_mass);
        if plans.is_empty() {
            if warned.insert(owner) {
                tracing::warn!(
                    "ragdoll: {owner:?} has a SkinnedMesh whose {} node(s) form no bone \
                     (a bone is a node with a parent), so there is nothing to build",
                    skinned.nodes.len()
                );
            }
            continue;
        }

        // Ids first: a bone's `Joint` names its parent's entity, and half the
        // bones are planned before the bone they hang from.
        let entities: Vec<Entity> = plans.iter().map(|_| commands.spawn_empty().id()).collect();
        let by_node: HashMap<usize, usize> = plans
            .iter()
            .enumerate()
            .map(|(i, plan)| (plan.node, i))
            .collect();

        for (i, plan) in plans.iter().enumerate() {
            // Rapier takes a density and derives the mass from the shape, so
            // the mass the plan asked for has to be divided back out by the
            // capsule's volume. A non-positive result would give the body zero
            // mass, which Rapier reads as *infinite* -- a bone that anchors the
            // skeleton in mid-air instead of falling.
            let volume = plan.volume(radius);
            let density = if volume > 0.0 && plan.mass > 0.0 {
                plan.mass / volume
            } else {
                1.0
            };

            commands.entity(entities[i]).insert((
                RagdollBone,
                RigidBody::dynamic(),
                Collider {
                    shape: ColliderShape::Capsule {
                        half_height: plan.half_height,
                        radius,
                    },
                    restitution: 0.0,
                    friction: 0.5,
                    density,
                    sensor: false,
                },
                PhysicsInput {
                    position: plan.center.into(),
                    rotation: plan.rotation.into(),
                },
            ));

            let Some(parent_index) = plan.parent.and_then(|node| by_node.get(&node)).copied()
            else {
                continue;
            };
            let parent = &plans[parent_index];
            // The two bones meet at the shared node: this bone's head is that
            // point, and its parent's tail is the same point. Anchoring there
            // means the joint starts already satisfied instead of the solver
            // yanking the skeleton into shape on frame one.
            //
            // That shared node is also what names the joint. It is the knee,
            // not either of the two bones the knee connects -- see
            // `Ragdoll::joint_overrides`.
            let joint_node = skinned.nodes[plan.node].parent.unwrap_or(plan.node);
            commands.entity(entities[i]).insert(Joint {
                body_b: entities[parent_index],
                kind: ragdoll.joint_for_bone(&skinned.nodes[joint_node].name),
                anchor_a: plan.local_head().into(),
                anchor_b: parent.local_tail().into(),
            });
        }

        built.insert(
            owner,
            BuiltRagdoll {
                bones: plans
                    .iter()
                    .enumerate()
                    .map(|(i, plan)| (plan.node, entities[i]))
                    .collect(),
            },
        );
    }
}

/// Keeps every ragdoll bone collider out of [`RAGDOLL_GROUP`]'s own traffic.
///
/// Runs every frame rather than once at creation, for the reason
/// [`lock_character_rotation`] gives: a body is registered with Rapier by
/// `spawn_bodies`, so there is no single moment observable from here at which
/// "the collider now exists". Setting the same groups again is idempotent.
fn isolate_ragdoll_bones(
    mut world: ResMut<PhysicsWorld>,
    query: Query<Entity, (With<RagdollBone>, With<PhysicsHandles>)>,
) {
    let groups = InteractionGroups::all()
        .with_memberships(RAGDOLL_GROUP)
        .with_filter(Group::ALL.difference(RAGDOLL_GROUP));
    for entity in query.iter() {
        world.set_collision_groups(entity, groups);
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

    // ---- Joint sync (roadmap item 51) ------------------------------------

    /// A dynamic body at `at`, the shape a scene-authored jointed body has.
    fn spawn_jointable_body(app: &mut App, at: Vec3) -> Entity {
        app.world_mut()
            .spawn((
                Transform::from_position(at),
                RigidBody::dynamic(),
                Collider::ball(0.25),
                PhysicsInput {
                    position: at.into(),
                    rotation: Quat::IDENTITY.into(),
                },
            ))
            .id()
    }

    /// Two bodies a metre apart, jointed by a `Joint` component on the second
    /// one, stepped until `sync_joints` has had both Rapier bodies to work
    /// with. Returns them in `(anchor, hanging)` order.
    fn jointed_pair(app: &mut App) -> (Entity, Entity) {
        let anchor = spawn_jointable_body(app, Vec3::new(0.0, 0.0, 0.0));
        let hanging = spawn_jointable_body(app, Vec3::new(0.0, -1.0, 0.0));
        app.world_mut().entity_mut(hanging).insert(crate::Joint {
            body_b: anchor,
            kind: crate::JointKind::Spherical,
            anchor_a: Vec3::new(0.0, 0.5, 0.0).into(),
            anchor_b: Vec3::new(0.0, -0.5, 0.0).into(),
        });
        app.update();
        (anchor, hanging)
    }

    #[test]
    fn a_joint_component_becomes_a_real_joint_in_the_simulation() {
        // Without this the two despawn tests below are vacuous: "no joint
        // afterwards" is trivially true if one was never created.
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        let (anchor, hanging) = jointed_pair(&mut app);

        assert!(
            app.world()
                .resource::<PhysicsWorld>()
                .has_joint(hanging, anchor),
            "a Joint component on an entity whose two bodies both exist must \
             reach the simulation, the same way RigidBody/Collider do"
        );
    }

    #[test]
    fn a_joint_is_removed_when_its_target_despawns() {
        // Nothing fails at the moment this goes wrong, which is exactly why it
        // needs a test: the constraint simply stays, holding a body nothing
        // points at any more.
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        let (anchor, hanging) = jointed_pair(&mut app);
        assert!(app
            .world()
            .resource::<PhysicsWorld>()
            .has_joint(hanging, anchor));

        app.world_mut().despawn(anchor);
        app.update();

        assert!(
            !app.world()
                .resource::<PhysicsWorld>()
                .has_joint(hanging, anchor),
            "the joint's target despawned, so the joint must go with it"
        );
        assert!(
            app.world().get::<crate::Joint>(hanging).is_some(),
            "the surviving entity keeps its Joint component -- the component is \
             what the scene authored, and only the simulation's copy is stale"
        );
    }

    #[test]
    fn a_joint_is_removed_when_the_entity_holding_it_despawns() {
        // The other half of the same problem, and the half neither the
        // component nor the target can report: the despawned entity is in no
        // query at all afterwards.
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        let (anchor, hanging) = jointed_pair(&mut app);
        assert!(app
            .world()
            .resource::<PhysicsWorld>()
            .has_joint(hanging, anchor));

        app.world_mut().despawn(hanging);
        app.update();

        assert!(
            !app.world()
                .resource::<PhysicsWorld>()
                .has_joint(hanging, anchor),
            "the entity carrying the joint despawned, so the joint must go too"
        );
    }

    #[test]
    fn removing_the_joint_component_removes_the_joint() {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        let (anchor, hanging) = jointed_pair(&mut app);
        assert!(app
            .world()
            .resource::<PhysicsWorld>()
            .has_joint(hanging, anchor));

        app.world_mut().entity_mut(hanging).remove::<crate::Joint>();
        app.update();

        assert!(
            !app.world()
                .resource::<PhysicsWorld>()
                .has_joint(hanging, anchor),
            "the component is what says the constraint should exist; taking it \
             away must take the constraint away"
        );
    }

    #[test]
    fn a_joint_naming_a_body_that_never_spawns_is_simply_not_created() {
        // A `Joint` can outlive its target's body, or name an entity that has
        // none. Neither is a panic and neither is a joint -- and the retrying
        // sync pass must not spin itself into creating one anyway.
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        let bodyless = app.world_mut().spawn(Transform::default()).id();
        let hanging = spawn_jointable_body(&mut app, Vec3::ZERO);
        app.world_mut().entity_mut(hanging).insert(crate::Joint {
            body_b: bodyless,
            kind: crate::JointKind::Fixed,
            anchor_a: Vec3::ZERO.into(),
            anchor_b: Vec3::ZERO.into(),
        });

        for _ in 0..5 {
            app.update();
        }

        assert!(
            !app.world()
                .resource::<PhysicsWorld>()
                .has_joint(hanging, bodyless),
            "an entity with no rigid body cannot be one end of a joint"
        );
    }

    // ---- Ragdoll (roadmap item 52, sub-step 1/2) -------------------------

    /// A ragdoll-shaped skeleton standing 10 units up: hips at the root, a
    /// two-bone spine, and a three-bone left leg. Branching on purpose — a
    /// straight chain would not notice a root that fails to hold its children
    /// together.
    fn humanoid_skeleton() -> SkinnedMesh {
        let node =
            |name: &str, position: [f32; 3], parent: Option<usize>| bsengine_gltf::NodeTransform {
                name: name.to_string(),
                position,
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
                parent,
            };
        SkinnedMesh {
            mesh_id: 0,
            rest_vertices: Vec::new(),
            skin: Vec::new(),
            skin_data: Default::default(),
            nodes: vec![
                node("Hips", [0.0, 10.0, 0.0], None),
                node("Spine", [0.0, 0.45, 0.0], Some(0)),
                node("Head", [0.0, 0.40, 0.0], Some(1)),
                node("LeftUpLeg", [0.15, -0.10, 0.0], Some(0)),
                node("LeftLeg", [0.0, -0.45, 0.0], Some(3)),
                node("LeftFoot", [0.0, -0.42, 0.05], Some(4)),
            ],
        }
    }

    /// Every ragdoll bone body currently in the world, as
    /// `(entity, PhysicsTransform)`.
    fn bone_bodies(app: &mut App) -> Vec<(Entity, Vec3)> {
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &PhysicsTransform), With<RagdollBone>>();
        query
            .iter(app.world())
            .map(|(e, t)| (e, t.position.0))
            .collect()
    }

    #[derive(Clone, Default)]
    struct LogSink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for LogSink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl tracing_subscriber::fmt::MakeWriter<'_> for LogSink {
        type Writer = Self;
        fn make_writer(&self) -> Self {
            self.clone()
        }
    }

    /// Runs `body` with every `tracing` event on this thread captured.
    fn capture_logs<T>(body: impl FnOnce() -> T) -> (T, String) {
        let sink = LogSink::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(sink.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::WARN)
            .finish();
        let out = tracing::subscriber::with_default(subscriber, body);
        let logs = String::from_utf8_lossy(&sink.0.lock().unwrap()).into_owned();
        (out, logs)
    }

    #[test]
    fn an_activated_ragdoll_falls_without_the_skeleton_coming_apart() {
        // Both halves are needed, for the same reason item 51's chain test
        // needed both: "it fell" alone passes on a skeleton that scattered to
        // the four winds, and "it held together" alone passes on one that never
        // moved at all.
        //
        // There is a *floor*, and one bone is kicked, and neither is decoration.
        // In empty space every body accelerates identically, so the gaps between
        // them never change and the cohesion half passes with no joints in the
        // simulation at all -- the assertion would be about gravity, not about
        // this feature. Landing puts real load through the joints, and the kick
        // is the item-51 shape: something that would send one bone away on its
        // own has to drag the rest of the skeleton with it instead.
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        spawn_floor(&mut app);
        let skeleton = humanoid_skeleton();
        app.world_mut().spawn((
            Ragdoll {
                active: true,
                ..Default::default()
            },
            skeleton.clone(),
        ));

        app.update();
        let start: HashMap<Entity, Vec3> = bone_bodies(&mut app).into_iter().collect();
        assert_eq!(
            start.len(),
            skeleton.nodes.len(),
            "one bone body per node, the root included"
        );

        // The foot: the extremity furthest from the root, so an unconstrained
        // one has the whole chain to get away from.
        let foot = *start
            .iter()
            .min_by(|a, b| a.1.y.total_cmp(&b.1.y))
            .expect("bones exist")
            .0;
        app.world_mut()
            .resource_mut::<PhysicsWorld>()
            .apply_impulse(foot, Vec3::new(60.0, 0.0, 25.0));

        for _ in 0..300 {
            app.update();
        }
        let end: HashMap<Entity, Vec3> = bone_bodies(&mut app).into_iter().collect();

        // 1. It fell. It started 10 units up and the floor's surface is at 0.
        let mut min_drop = f32::INFINITY;
        for (entity, from) in &start {
            let to = end[entity];
            min_drop = min_drop.min(from.y - to.y);
        }
        println!("ragdoll collapse: smallest drop over 300 steps = {min_drop}");
        assert!(
            min_drop > 8.0,
            "every bone must have fallen to the floor 10 units below; the \
             least-moved one dropped only {min_drop} units"
        );

        // 2. It did not explode. A joint holds its two bones at a shared point,
        //    so the two capsule centres can never be further apart than the sum
        //    of their half-heights -- whatever angle the joint bends to.
        //
        // Which body is which bone is settled by where each one *started*: on
        // the first frame a bone body sits exactly on its plan's centre. Query
        // iteration order is an archetype detail and would be the wrong thing
        // to trust here.
        let plans = crate::ragdoll::plan_bones(&skeleton.nodes, 0.08, 70.0);
        let entity_of: HashMap<usize, Entity> = plans
            .iter()
            .map(|plan| {
                let (&entity, _) = start
                    .iter()
                    .find(|(_, &at)| at.abs_diff_eq(plan.center, 0.01))
                    .unwrap_or_else(|| {
                        panic!(
                            "no bone body spawned at node {}'s planned centre {:?}; \
                             bodies started at {:?}",
                            plan.node,
                            plan.center,
                            start.values().collect::<Vec<_>>()
                        )
                    });
                (plan.node, entity)
            })
            .collect();

        let mut separations: Vec<(usize, f32, f32)> = Vec::new();
        for plan in &plans {
            let Some(parent_node) = plan.parent else {
                continue;
            };
            let parent = plans.iter().find(|p| p.node == parent_node).unwrap();
            separations.push((
                plan.node,
                (end[&entity_of[&plan.node]] - end[&entity_of[&parent_node]]).length(),
                plan.half_height + parent.half_height,
            ));
        }
        println!("ragdoll cohesion (node, separation, joint distance): {separations:?}");
        let (node, separation, limit) = separations
            .iter()
            .copied()
            .max_by(|a, b| (a.1 - a.2).total_cmp(&(b.1 - b.2)))
            .expect("a branching skeleton has jointed pairs");
        assert!(
            separation <= limit + 0.05,
            "the skeleton came apart at node {node}: two jointed bones ended up \
             {separation} apart when the joint can only let them reach {limit}"
        );
    }

    #[test]
    fn an_inactive_ragdoll_creates_no_bodies_at_all() {
        // `active: false` must be completely inert -- a ragdoll component on a
        // normal character may not perturb it, and `Ragdoll::default()` is what
        // the Inspector's Add Component gives you.
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        app.world_mut()
            .spawn((Ragdoll::default(), humanoid_skeleton()));

        for _ in 0..30 {
            app.update();
        }

        assert!(
            bone_bodies(&mut app).is_empty(),
            "an inactive ragdoll must build nothing"
        );
    }

    #[test]
    fn switching_a_ragdoll_off_takes_its_bodies_out_of_the_simulation() {
        // Despawning the bone entities is not enough on its own: nothing
        // removes their Rapier bodies, so a "torn down" ragdoll would go on
        // falling through the level and reporting contacts under collider
        // handles that map to no entity. Nothing fails at the moment that
        // happens, which is exactly why it needs asserting.
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        let owner = app
            .world_mut()
            .spawn((
                Ragdoll {
                    active: true,
                    ..Default::default()
                },
                humanoid_skeleton(),
            ))
            .id();

        app.update();
        let bones = bone_bodies(&mut app);
        assert!(!bones.is_empty(), "the ragdoll should have built bodies");
        let a_bone = bones[0].0;
        assert!(app
            .world()
            .resource::<PhysicsWorld>()
            .get_linvel(a_bone)
            .is_some());

        app.world_mut().get_mut::<Ragdoll>(owner).unwrap().active = false;
        app.update();

        assert!(
            bone_bodies(&mut app).is_empty(),
            "the bone entities must be gone"
        );
        assert!(
            app.world()
                .resource::<PhysicsWorld>()
                .get_linvel(a_bone)
                .is_none(),
            "and so must their rigid bodies -- an entity-less body keeps simulating"
        );
    }

    #[test]
    fn a_ragdoll_without_a_skinned_mesh_warns_instead_of_panicking() {
        // The bone hierarchy comes from `SkinnedMesh.nodes`; with no skeleton
        // there is nothing to build. An authoring mistake is not a reason to
        // take the game down -- but it is a reason to say so, or it looks
        // exactly like a ragdoll that ran and did nothing.
        let (mut app, logs) = capture_logs(|| {
            let mut app = new_app();
            app.add_plugins(PhysicsPlugin);
            app.world_mut().spawn(Ragdoll {
                active: true,
                ..Default::default()
            });
            for _ in 0..5 {
                app.update();
            }
            app
        });

        assert!(bone_bodies(&mut app).is_empty(), "no skeleton, no bodies");
        assert!(
            logs.contains("SkinnedMesh"),
            "the warning must name what is missing; captured logs were: {logs:?}"
        );
        assert_eq!(
            logs.matches("no SkinnedMesh").count(),
            1,
            "and it must be said once, not once per frame forever: {logs:?}"
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
