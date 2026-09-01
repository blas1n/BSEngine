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
    ragdoll::{plan_bones, pose_from_bones},
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
                // After `sync_from_rapier`, which is what puts this step's
                // simulated pose into `PhysicsTransform`. Read before that and
                // the skinned mesh would trail the bodies by a frame.
                publish_ragdoll_pose,
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
                RagdollBone {
                    owner,
                    node: plan.node,
                },
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

/// Publishes the pose the bone bodies imply into the skinned mesh, so the
/// character on screen follows the simulation instead of its animation clips.
///
/// **This is the half of the ragdoll that fails silently.** Everything up to
/// here can be perfect — capsules in the right places, joints holding, the
/// skeleton collapsing exactly as it should — and if this pass is missing, the
/// character goes on playing its walk cycle over the top of it and looks
/// completely normal. Nothing about the bodies would show it; the only
/// observable difference is in the joint matrices the skinning ends up with.
///
/// The direction of travel is what keeps the two crates apart:
/// `bsengine-gltf` never names a physics type, it just honours a
/// `pose_override` written by whoever put one there. See that field, and the
/// `bsengine-gltf` dependency note in this crate's `Cargo.toml`.
///
/// Clearing the override again matters as much as writing it: while one is in
/// place the clips are not read at all, so a ragdoll switched off without this
/// leaves the character frozen in the pose it died in.
fn publish_ragdoll_pose(
    bones: Query<(&RagdollBone, &PhysicsTransform)>,
    mut owners: Query<(Entity, &Ragdoll, &mut SkinnedMesh)>,
) {
    let mut by_owner: HashMap<Entity, HashMap<usize, (Vec3, Quat)>> = HashMap::new();
    for (bone, transform) in bones.iter() {
        by_owner
            .entry(bone.owner)
            .or_default()
            .insert(bone.node, (transform.position.0, transform.rotation.0));
    }

    for (owner, ragdoll, mut skinned) in owners.iter_mut() {
        let poses = by_owner.get(&owner).filter(|_| ragdoll.active);
        let Some(poses) = poses else {
            // `is_empty` first, not an unconditional `clear`: taking `&mut` out
            // of the `Mut` marks the component changed, and every skinned mesh
            // in the level that has a ragdoll it has never used would report a
            // change every single frame.
            if !skinned.pose_override.is_empty() {
                skinned.pose_override.clear();
            }
            continue;
        };

        let radius = ragdoll.bone_radius.max(1.0e-3);
        let plans = plan_bones(&skinned.nodes, radius, ragdoll.total_mass);
        let bone_poses: Vec<Option<(Vec3, Quat)>> = plans
            .iter()
            .map(|plan| poses.get(&plan.node).copied())
            .collect();
        let pose = pose_from_bones(&skinned.nodes, &plans, &bone_poses);
        skinned.pose_override = pose;
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
        let nodes = vec![
            node("Hips", [0.0, 10.0, 0.0], None),
            node("Spine", [0.0, 0.45, 0.0], Some(0)),
            node("Head", [0.0, 0.40, 0.0], Some(1)),
            node("LeftUpLeg", [0.15, -0.10, 0.0], Some(0)),
            node("LeftLeg", [0.0, -0.45, 0.0], Some(3)),
            node("LeftFoot", [0.0, -0.42, 0.05], Some(4)),
        ];
        // Every node is a joint, and every inverse bind matrix is the identity.
        // That is not what a real exporter writes, and it is chosen so a joint
        // matrix reads back as the node's *global transform* with nothing to
        // undo: `joint_matrices[j].transform_point3(ZERO)` is simply where node
        // `j` is. `REST_POSITIONS` states independently where that should be.
        SkinnedMesh {
            mesh_id: 0,
            rest_vertices: Vec::new(),
            skin: Vec::new(),
            skin_data: bsengine_gltf::SkinData {
                joint_node_indices: (0..nodes.len()).collect(),
                inverse_bind_matrices: vec![glam::Mat4::IDENTITY.to_cols_array_2d(); nodes.len()],
            },
            nodes,
            pose_override: Vec::new(),
            joint_matrices: Vec::new(),
        }
    }

    /// Where [`humanoid_skeleton`]'s six nodes sit in model space at rest,
    /// written out rather than accumulated. Re-deriving them with the same
    /// parent-chain walk the code under test uses would let a broken walk agree
    /// with itself.
    const REST_POSITIONS: [Vec3; 6] = [
        Vec3::new(0.0, 10.0, 0.0),   // Hips
        Vec3::new(0.0, 10.45, 0.0),  // Spine
        Vec3::new(0.0, 10.85, 0.0),  // Head
        Vec3::new(0.15, 9.90, 0.0),  // LeftUpLeg
        Vec3::new(0.15, 9.45, 0.0),  // LeftLeg
        Vec3::new(0.15, 9.03, 0.05), // LeftFoot
    ];

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

    // ---- Per-bone joint overrides reach the simulation --------------------

    /// A root and two collinear bones hanging straight down from it, so that
    /// consecutive bones share a rest rotation.
    ///
    /// That collinearity is not cosmetic. A revolute joint's axis is given in
    /// *both* bodies' local frames, so two bones that rest at different
    /// orientations start with the constraint already violated and the solver
    /// spends the first steps hauling them into agreement — which is motion the
    /// test would then have to tell apart from the motion it is measuring.
    fn straight_leg() -> SkinnedMesh {
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
                node("Hips", [0.0, 5.0, 0.0], None),
                node("Knee", [0.0, -1.0, 0.0], Some(0)),
                node("Ankle", [0.0, -1.0, 0.0], Some(1)),
            ],
            pose_override: Vec::new(),
            joint_matrices: Vec::new(),
        }
    }

    /// Which entity is which bone, by the node the bone's child end sits on.
    fn bone_entities_by_node(app: &mut App) -> HashMap<usize, Entity> {
        let mut query = app.world_mut().query::<(Entity, &RagdollBone)>();
        query
            .iter(app.world())
            .map(|(entity, bone)| (bone.node, entity))
            .collect()
    }

    fn rotation_of(app: &App, entity: Entity) -> Quat {
        app.world()
            .get::<PhysicsTransform>(entity)
            .expect("a bone body has a simulated transform")
            .rotation
            .0
    }

    #[test]
    fn a_bone_overridden_to_revolute_is_constrained_off_axis() {
        // Task 1 proved `joint_for_bone` RETURNS the override. This proves the
        // construction path actually USES it: without it the map could be read
        // and then dropped on the floor, every ragdoll would be spherical
        // throughout, and everything else in this file would still pass.
        //
        // The measurement is the hinge axis as each of the joint's two bodies
        // sees it. A revolute joint's whole content is that those two stay
        // pointing the same way; a spherical joint says nothing about them. So
        // the same off-axis torque, applied to the same bone of the same
        // skeleton, must open that angle in one and not in the other.

        /// The hinge axis, in each bone body's own local space.
        const HINGE: Vec3 = Vec3::X;

        use crate::JointKind;

        /// Torques the shin about `local_torque_axis`, expressed in the shin's
        /// own frame, and reports `(off-axis tilt, total bend)`:
        ///
        /// * how far the hinge axis as the shin sees it has drifted from the
        ///   hinge axis as the thigh sees it — the whole content of a revolute
        ///   constraint, and the number that must stay at zero;
        /// * how far the two bones have turned relative to each other at all —
        ///   which is what separates "a hinge refused an off-axis torque" from
        ///   "the joint welded the skeleton solid" and from "the torque never
        ///   landed".
        fn torque_the_shin(
            overrides: HashMap<String, JointKind>,
            local_torque_axis: Vec3,
        ) -> (f32, f32) {
            let mut app = new_app();
            app.add_plugins(PhysicsPlugin);
            // No gravity and no floor: the only thing that can move a bone is
            // the test's own torque, so a difference between the runs has one
            // explanation rather than three.
            app.world_mut()
                .resource_mut::<PhysicsWorld>()
                .set_gravity(0.0);
            app.world_mut().spawn((
                Ragdoll {
                    active: true,
                    joint_overrides: overrides,
                    ..Default::default()
                },
                straight_leg(),
            ));
            app.update();

            let bones = bone_entities_by_node(&mut app);
            let shin = bones[&2];
            let thigh = bones[&1];

            let at_rest =
                (rotation_of(&app, shin) * HINGE).angle_between(rotation_of(&app, thigh) * HINGE);
            assert!(
                at_rest < 0.01,
                "the two bones start collinear, so the hinge axis must start \
                 agreed; it is already {at_rest} rad apart and the measurement \
                 below would be of the solver settling, not of the constraint"
            );

            let torque = rotation_of(&app, shin) * local_torque_axis;
            app.world_mut()
                .resource_mut::<PhysicsWorld>()
                .apply_torque_impulse(shin, torque * 5.0);
            for _ in 0..60 {
                app.update();
            }

            let (shin_rot, thigh_rot) = (rotation_of(&app, shin), rotation_of(&app, thigh));
            (
                (shin_rot * HINGE).angle_between(thigh_rot * HINGE),
                (thigh_rot.inverse() * shin_rot).to_axis_angle().1,
            )
        }

        /// The joint sits on the node the two bones share, which is the knee —
        /// not on either of the bones it joins. See `Ragdoll::joint_overrides`.
        fn knee_hinge() -> HashMap<String, JointKind> {
            HashMap::from([(
                "Knee".to_string(),
                JointKind::Revolute {
                    axis: HINGE.into(),
                    limits: None,
                },
            )])
        }

        // Off the hinge axis: a ball joint allows it, a hinge must not.
        let (spherical, spherical_bend) = torque_the_shin(HashMap::new(), Vec3::Z);
        let (revolute, revolute_bend) = torque_the_shin(knee_hinge(), Vec3::Z);
        // Along it: proves the override produced a *hinge* and not a weld, so
        // that the zero above is the axis being held and not the whole
        // skeleton being frozen.
        let (_, along_axis_bend) = torque_the_shin(knee_hinge(), HINGE);
        println!(
            "off-axis torque: spherical tilted {spherical} rad (bend \
             {spherical_bend}), overridden tilted {revolute} rad (bend \
             {revolute_bend}); on-axis torque bent the override by \
             {along_axis_bend} rad"
        );

        assert!(
            spherical > 0.3,
            "the default ball joint must let the bone turn off-axis, or the \
             comparison is between two motionless skeletons; got {spherical} rad"
        );
        assert!(
            revolute < 0.05,
            "with the knee overridden to a hinge, the same torque must not tilt \
             it off its axis -- an override that is read and then discarded \
             leaves this at the spherical {spherical} rad; got {revolute}"
        );
        assert!(
            along_axis_bend > 0.3,
            "...and the override has to be a hinge rather than a weld, or the \
             {revolute} rad above is a skeleton that cannot move at all rather \
             than one held to its axis; torquing along the axis bent it only \
             {along_axis_bend} rad"
        );
    }

    // ---- Skinning follows the ragdoll (item 52 sub-step 1/2, task 3) -----
    //
    // The task the whole feature lives or dies on, and it fails quietly. An
    // implementation that builds the bodies and lets them fall while skinning
    // goes on reading the animation clips produces a character that looks
    // completely normal on screen with a full ragdoll simulating underneath it
    // -- and every test above passes in that state, because they all inspect
    // the physics bodies, and the bodies are exactly what a broken version also
    // gets right. So these assert on the JOINT MATRICES, which are the only
    // place the difference is observable.
    //
    // They live in this crate rather than in `bsengine-gltf` because this is
    // the only one that can see both ends: `bsengine-gltf` must not know what
    // physics is (see the dependency note in Cargo.toml), so it can be asked
    // whether it honours a `pose_override` -- which it is, over there -- but
    // not whether a real ragdoll is what fills one.

    /// Where a clip holds the skeleton's root: 50 units along +X of where it
    /// rests. Constant over the clip's whole timeline, so nothing here depends
    /// on how far an `AnimationPlayer` has been ticked.
    const CLIP_ROOT: Vec3 = Vec3::new(50.0, 10.0, 0.0);

    fn shifted_clip_library() -> bsengine_gltf::AnimationClipLibrary {
        bsengine_gltf::AnimationClipLibrary::from_clips(vec![bsengine_gltf::AnimationClip {
            name: "pose".to_string(),
            duration: 1.0,
            channels: vec![bsengine_gltf::AnimationChannel {
                node_index: 0,
                times: vec![0.0, 1.0],
                values: bsengine_gltf::KeyframeValues::Translations(vec![
                    CLIP_ROOT.to_array(),
                    CLIP_ROOT.to_array(),
                ]),
                interpolation: bsengine_gltf::Interpolation::Linear,
            }],
        }])
    }

    /// A skinned character playing [`shifted_clip_library`]'s clip, physics and
    /// skinning both running, optionally carrying a `Ragdoll`.
    fn skinned_character(ragdoll: Option<Ragdoll>) -> (App, Entity) {
        let mut app = new_app();
        app.add_plugins(PhysicsPlugin);
        app.add_plugins(bsengine_gltf::SkinnedMeshPlugin);
        let mut entity = app.world_mut().spawn((
            humanoid_skeleton(),
            shifted_clip_library(),
            bsengine_core::AnimationPlayer::new("pose").with_duration(1.0),
        ));
        if let Some(ragdoll) = ragdoll {
            entity.insert(ragdoll);
        }
        let owner = entity.id();
        (app, owner)
    }

    /// Where each node currently is, according to the joint matrices the
    /// skinning system last computed. The inverse bind matrices are the
    /// identity (see [`humanoid_skeleton`]), so a joint matrix applied to the
    /// origin is the node's global position and nothing else.
    fn skinned_node_positions(app: &App, owner: Entity) -> Vec<Vec3> {
        app.world()
            .get::<SkinnedMesh>(owner)
            .expect("the character keeps its skinned mesh")
            .joint_matrices
            .iter()
            .map(|m| m.transform_point3(Vec3::ZERO))
            .collect()
    }

    /// Each ragdoll bone body's position, keyed by the node it belongs to.
    fn bone_positions_by_node(app: &mut App) -> HashMap<usize, Vec3> {
        let mut query = app.world_mut().query::<(&RagdollBone, &PhysicsTransform)>();
        query
            .iter(app.world())
            .map(|(bone, transform)| (bone.node, transform.position.0))
            .collect()
    }

    #[test]
    fn an_active_ragdoll_makes_the_joint_matrices_follow_physics_not_the_clip() {
        let (mut app, owner) = skinned_character(Some(Ragdoll {
            active: true,
            ..Default::default()
        }));

        // One frame: the bodies exist and are on their plans' centres, so the
        // pose is still the rest pose -- which is already the whole assertion
        // against the quiet failure, because the clip would put the skeleton
        // 50 units away along +X.
        app.update();
        let before = skinned_node_positions(&app, owner);
        assert_eq!(before.len(), REST_POSITIONS.len(), "one matrix per joint");
        for (node, rest) in REST_POSITIONS.iter().enumerate() {
            assert!(
                before[node].abs_diff_eq(*rest, 0.05),
                "on the frame the ragdoll switched on, node {node} should still \
                 be at its rest position {rest:?}; it is at {:?}. The clip puts \
                 the skeleton at x = {}, so a skinning path still reading the \
                 clip lands there instead",
                before[node],
                rest.x + CLIP_ROOT.x
            );
        }

        // No floor: in free fall every bone accelerates identically and the
        // joints stay satisfied, so the skeleton translates rigidly and "the
        // mesh followed the bodies" is an exact claim rather than an
        // approximate one.
        let bones_before = bone_positions_by_node(&mut app);
        for _ in 0..120 {
            app.update();
        }
        let bones_after = bone_positions_by_node(&mut app);
        let travel = bones_after[&0] - bones_before[&0];
        println!("ragdoll skinning: the bone bodies travelled {travel:?}");
        assert!(
            travel.y < -1.0,
            "the ragdoll has to have actually moved, or 'the mesh followed it' \
             is a claim about nothing; it travelled {travel:?}"
        );

        let after = skinned_node_positions(&app, owner);
        for (node, rest) in REST_POSITIONS.iter().enumerate() {
            let moved = after[node] - before[node];
            assert!(
                moved.abs_diff_eq(travel, 0.05),
                "node {node} moved {moved:?} while its bone bodies moved \
                 {travel:?} -- the joint matrices are not following the physics"
            );
            assert!(
                (after[node].x - (rest.x + CLIP_ROOT.x)).abs() > 10.0,
                "...and they must not be following the clip either: node {node} \
                 ended at {:?}, and the clip's pose is x = {}",
                after[node],
                rest.x + CLIP_ROOT.x
            );
        }
    }

    #[test]
    fn an_inactive_ragdoll_leaves_the_joint_matrices_byte_identical() {
        // The opposite direction, and the one every already-shipped character
        // depends on: attaching a `Ragdoll` and never switching it on must
        // leave the animation path producing bit-for-bit what it produced
        // before this feature existed -- which is what an entity with no
        // `Ragdoll` component at all still gets.
        let (mut with, with_owner) = skinned_character(Some(Ragdoll::default()));
        let (mut without, without_owner) = skinned_character(None);
        for _ in 0..30 {
            with.update();
            without.update();
        }

        let a = with.world().get::<SkinnedMesh>(with_owner).unwrap();
        let b = without.world().get::<SkinnedMesh>(without_owner).unwrap();
        assert!(
            a.pose_override.is_empty(),
            "an inactive ragdoll must not publish a pose at all"
        );
        assert_eq!(a.joint_matrices.len(), REST_POSITIONS.len());
        assert_eq!(a.joint_matrices.len(), b.joint_matrices.len());
        for (node, (got, want)) in a.joint_matrices.iter().zip(&b.joint_matrices).enumerate() {
            assert_eq!(
                got.to_cols_array(),
                want.to_cols_array(),
                "joint {node} of a character carrying an inactive ragdoll must \
                 be bit-for-bit the same matrix as one carrying no ragdoll"
            );
        }
        // ...and the animation really is what both of them are doing, or the
        // comparison above is between two copies of the same nothing.
        let animated = a.joint_matrices[0].transform_point3(Vec3::ZERO);
        assert!(
            (animated.x - CLIP_ROOT.x).abs() < 0.001,
            "the clip should be driving the root to x = {}, got {animated:?}",
            CLIP_ROOT.x
        );
    }

    #[test]
    fn switching_a_ragdoll_off_hands_the_skeleton_back_to_the_animation() {
        // While a pose override is in place the clips are not read at all, so a
        // ragdoll that stops without clearing it leaves the character frozen in
        // the pose it died in -- forever, and with nothing failing anywhere.
        let (mut app, owner) = skinned_character(Some(Ragdoll {
            active: true,
            ..Default::default()
        }));
        for _ in 0..30 {
            app.update();
        }
        let collapsed = skinned_node_positions(&app, owner)[0];
        assert!(
            (collapsed.x - CLIP_ROOT.x).abs() > 10.0,
            "sanity: while active the ragdoll, not the clip, is driving"
        );

        app.world_mut().get_mut::<Ragdoll>(owner).unwrap().active = false;
        app.update();

        assert!(
            app.world()
                .get::<SkinnedMesh>(owner)
                .unwrap()
                .pose_override
                .is_empty(),
            "the override must be cleared, not merely stop being updated"
        );
        let animated = skinned_node_positions(&app, owner)[0];
        assert!(
            (animated.x - CLIP_ROOT.x).abs() < 0.001,
            "with the ragdoll off the clip drives the root back to x = {}, got \
             {animated:?}",
            CLIP_ROOT.x
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
