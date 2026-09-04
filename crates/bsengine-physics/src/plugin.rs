use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bsengine_gltf::{IkChains, SkinnedMesh};
use glam::{Quat, Vec3};
use rapier3d::geometry::{CollisionEvent as RapierCollisionEvent, ContactPair};
use rapier3d::pipeline::EventHandler;
use rapier3d::prelude::*;

use crate::{
    components::{
        CharacterBody, Collider, ColliderShape, CollisionEvent, FootIkGround, Joint,
        PhysicsHandles, PhysicsInput, PhysicsTransform, Ragdoll, RagdollBone, RigidBody,
        RigidBodyType, Vehicle, WheelIndex, WheelState,
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
                // After `spawn_bodies`, because the chassis body it looks up
                // does not exist until then; before `step_world`, because
                // `update_vehicle` applies impulses and an impulse added after
                // the step has integrated does not move the car until the next
                // frame. That failure is quiet -- the car still drives, just a
                // frame behind, which reads as sluggish handling.
                sync_vehicles,
                // After `sync_vehicles`, which is what fills the wheel states
                // this reads. Before `step_world` only incidentally -- it
                // touches no physics, just the visuals the last step produced.
                sync_wheel_transforms,
                step_world,
                sync_from_rapier,
                // After `sync_from_rapier`, so the ray is cast against where
                // bodies actually ended up this step rather than where they
                // were before it -- the same reason `update_grounded` sits
                // late in this chain.
                probe_foot_ik_ground,
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

/// The chassis-space direction a wheel ray is cast along: straight down.
///
/// Not authored. `add_wheel` wants a basis vector, but it follows from the
/// chassis convention (+Y up, -Z forward) rather than being a per-wheel choice,
/// so a scene author writes a mount point and a radius instead.
const WHEEL_RAY_DIRECTION: Vector = Vector::new(0.0, -1.0, 0.0);

/// The chassis-space axle a wheel spins about: the lateral axis.
///
/// Not authored, for the same reason as [`WHEEL_RAY_DIRECTION`].
const WHEEL_AXLE: Vector = Vector::new(-1.0, 0.0, 0.0);

/// Builds a Rapier vehicle controller for each [`Vehicle`], pushes this frame's
/// throttle/steering/brake into it, and steps it.
///
/// The controller holds its wheel state outside the ECS, so it lives in a
/// `Local` side table keyed by entity — the same shape [`sync_ragdolls`] uses
/// for the bodies it builds, and for the same reason.
fn sync_vehicles(
    mut world: ResMut<PhysicsWorld>,
    mut controllers: Local<HashMap<Entity, rapier3d::control::DynamicRayCastVehicleController>>,
    mut vehicles: Query<(Entity, &mut Vehicle)>,
) {
    // Tear down first, matching `sync_ragdolls`. A lookup that misses covers an
    // entity that despawned and one that merely lost its `Vehicle`; neither is
    // in `vehicles` any more, and without this the table grows forever and a
    // re-added `Vehicle` would inherit the old controller's wheels.
    let stale: Vec<Entity> = controllers
        .keys()
        .copied()
        .filter(|e| vehicles.get(*e).is_err())
        .collect();
    for entity in stale {
        controllers.remove(&entity);
    }

    for (entity, mut vehicle) in vehicles.iter_mut() {
        let Some(&chassis) = world.entity_body_map.get(&entity) else {
            // No body yet. `spawn_bodies` runs before this, so this is a
            // vehicle whose `RigidBody`/`Collider` are missing rather than a
            // one-frame lag; leaving it alone is what lets it start working if
            // they arrive later.
            continue;
        };

        let controller = controllers.entry(entity).or_insert_with(|| {
            let mut c = rapier3d::control::DynamicRayCastVehicleController::new(chassis);
            for wheel in &vehicle.wheels {
                // `..Default::default()` for the fields `WheelConfig` does not
                // expose (max suspension force, side friction stiffness),
                // rather than assigning field by field after a `default()`.
                let tuning = rapier3d::control::WheelTuning {
                    suspension_stiffness: wheel.suspension_stiffness,
                    suspension_compression: wheel.damping_compression,
                    suspension_damping: wheel.damping_relaxation,
                    friction_slip: wheel.friction_slip,
                    max_suspension_travel: wheel.max_suspension_travel,
                    ..Default::default()
                };
                let c_ws = wheel.connection.0;
                c.add_wheel(
                    Vector::new(c_ws.x, c_ws.y, c_ws.z),
                    WHEEL_RAY_DIRECTION,
                    WHEEL_AXLE,
                    wheel.suspension_rest_length,
                    wheel.radius,
                    &tuning,
                );
            }
            c
        });

        // Push this frame's inputs in. Engine force reaches only `drives`
        // wheels and steering only `steers` wheels, so a rear-wheel-drive,
        // front-steering car falls out of the authored layout rather than
        // needing a drivetrain enum.
        for (i, cfg) in vehicle.wheels.iter().enumerate() {
            let Some(w) = controller.wheels_mut().get_mut(i) else {
                break;
            };
            w.engine_force = if cfg.drives { vehicle.throttle } else { 0.0 };
            w.steering = if cfg.steers { vehicle.steering } else { 0.0 };
            w.brake = vehicle.brake;
        }

        world.update_vehicle(controller);

        // Copy out what the controller just computed. Sub-step 1/2 discarded
        // all of it; this is the channel that keeps it, because the controller
        // lives in a `Local` no other system can see.
        //
        // Roll is integrated here rather than read back: Rapier's `Wheel`
        // exposes suspension, steering and contact, but not accumulated spin.
        // `timestep()` rather than a frame delta, so the wheels turn in lockstep
        // with the simulation that moved the car.
        let dt = world.timestep();
        let speed = controller.current_vehicle_speed;
        let wheel_count = vehicle.wheels.len();
        vehicle
            .wheel_states
            .resize(wheel_count, WheelState::default());
        for i in 0..wheel_count {
            let (Some(w), Some(radius)) = (
                controller.wheels().get(i),
                vehicle.wheels.get(i).map(|c| c.radius.max(1.0e-3)),
            ) else {
                break;
            };
            let info = *w.raycast_info();
            let steering = w.steering;
            let suspension_length = info.suspension_length;
            let grounded = info.is_in_contact;
            let state = &mut vehicle.wheel_states[i];
            state.suspension_length = suspension_length;
            state.steering = steering;
            state.grounded = grounded;
            state.rotation += speed / radius * dt;
        }
    }
}

/// Poses each wheel visual from its parent vehicle's published wheel state.
///
/// Reads only what `sync_vehicles` already computed — this simulates nothing.
/// The wheels stay raycasts; giving them bodies would put them in the
/// simulation twice.
fn sync_wheel_transforms(
    vehicles: Query<&Vehicle>,
    mut wheels: Query<(
        &bsengine_core::Parent,
        &WheelIndex,
        &mut bsengine_core::Transform,
    )>,
) {
    for (parent, index, mut transform) in wheels.iter_mut() {
        let Ok(vehicle) = vehicles.get(parent.0) else {
            continue;
        };
        // A visual whose index outruns the wheel list is skipped, not treated
        // as an error: a car may be authored with fewer visuals than wheels.
        let Some(state) = vehicle.wheel_states.get(index.0) else {
            continue;
        };
        let Some(cfg) = vehicle.wheels.get(index.0) else {
            continue;
        };

        // The mount point is fixed on the chassis; the suspension decides how
        // far below it the wheel actually sits, so the visual hangs from the
        // mount by the current suspension length.
        let mount = cfg.connection.0;
        transform.position = Vec3::new(mount.x, mount.y - state.suspension_length, mount.z).into();

        // Steer about the chassis up axis, then roll about the axle. Order
        // matters: rolling first would spin the wheel about a steered axle and
        // wobble it as it turns.
        let steer = Quat::from_rotation_y(state.steering);
        let roll = Quat::from_rotation_x(state.rotation);
        transform.rotation = (steer * roll).into();
    }
}

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
        .filter(|&owner| {
            !ragdolls
                .get(owner)
                .is_ok_and(|(_, r, _)| r.active || r.return_remaining > 0.0)
        })
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
    mut owners: Query<(Entity, &mut Ragdoll, &mut SkinnedMesh)>,
    time: Option<Res<bsengine_core::Time>>,
) {
    let dt = time.as_ref().map(|t| t.delta_seconds).unwrap_or(0.0);

    let mut by_owner: HashMap<Entity, HashMap<usize, (Vec3, Quat)>> = HashMap::new();
    for (bone, transform) in bones.iter() {
        by_owner
            .entry(bone.owner)
            .or_default()
            .insert(bone.node, (transform.position.0, transform.rotation.0));
    }

    for (owner, mut ragdoll, mut skinned) in owners.iter_mut() {
        let is_active = ragdoll.active || ragdoll.return_remaining > 0.0;
        let poses = by_owner.get(&owner).filter(|_| is_active);
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

        // While returning to animation: update the blend weight from the
        // countdown, then tick the countdown down.  When it hits zero, hand
        // off to animation immediately -- the pose published here would be
        // discarded anyway, so skip it and clear the override.
        if !ragdoll.active && ragdoll.return_remaining > 0.0 {
            if ragdoll.return_duration > 0.0 {
                skinned.pose_override_weight = ragdoll.return_remaining / ragdoll.return_duration;
            }
            ragdoll.return_remaining = (ragdoll.return_remaining - dt).max(0.0);
            if ragdoll.return_remaining == 0.0 {
                if !skinned.pose_override.is_empty() {
                    skinned.pose_override.clear();
                }
                skinned.pose_override_weight = 1.0;
                continue;
            }
        }

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

/// Casts a ray down from each IK chain's tip bone and puts that chain's target
/// on the ground it finds.
///
/// Reads the foot position `bsengine-gltf` published last frame rather than
/// re-deriving the pose: this crate has no clips and no globals, and one frame
/// of lag on a foot target is imperceptible. Reaching across the other way is
/// not an option -- `bsengine-gltf` must not depend on this crate.
fn probe_foot_ik_ground(
    world: Res<PhysicsWorld>,
    mut characters: Query<(Entity, &FootIkGround, &SkinnedMesh, &mut IkChains)>,
) {
    for (entity, ground, skinned, mut ik) in characters.iter_mut() {
        for (i, chain) in ik.chains.iter_mut().enumerate() {
            let Some(&foot) = skinned.ik_tip_positions.get(i) else {
                // No published position yet -- the skinning system has not run,
                // or this chain names a bone the rig lacks and was skipped.
                continue;
            };

            // Start above the foot: on a slope the animation routinely puts the
            // foot INSIDE the hill, and a ray starting at the foot would begin
            // below the surface and miss it entirely.
            let origin = foot + Vec3::Y * ground.probe_height;
            let max = ground.probe_height + ground.max_drop;
            let Some(hit) = world.cast_ray_excluding(origin, -Vec3::Y, max, entity) else {
                // Nothing underneath. Leave the target where it was rather than
                // writing one: a character stepping off a ledge would otherwise
                // have its feet yanked to wherever the ray gave up.
                continue;
            };

            chain.target = (hit.point + Vec3::Y * ground.offset).into();
        }
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
            ik_tip_positions: Vec::new(),
            animated_locals: Vec::new(),
            pose_override_weight: 1.0,
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
            ik_tip_positions: Vec::new(),
            animated_locals: Vec::new(),
            pose_override_weight: 1.0,
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

    // ---- Ragdoll return blend (roadmap item 52, sub-step 2/2, Task 3b) ---
    //
    // The blend ENDS at the animation pose, so an implementation that skips
    // blending entirely and snaps immediately reaches an identical final state.
    // Only the intermediate frames differ, which is why "after returning the
    // pose is the animated pose" cannot stand alone: it passes on a snap.
    // The mid-blend test is the load-bearing one.

    /// Starts a skinned character with an active ragdoll (no gravity, no
    /// floor) and advances until the skeleton has been driven by physics for
    /// a bit, then manually starts a return blend and returns the app, the
    /// owner entity, and the root position as the physics bodies left it.
    fn start_return_blend(blend_duration: f32) -> (App, Entity, Vec3) {
        let (mut app, owner) = skinned_character(Some(Ragdoll {
            active: true,
            ..Default::default()
        }));
        // No gravity: every bone falls identically, so the skeleton moves as a
        // rigid body.  The clip would put the root at CLIP_ROOT (x = 50); the
        // ragdoll keeps it at the rest position (x = 0).  The two poses are far
        // enough apart that a non-blending implementation is detectable.
        app.world_mut()
            .resource_mut::<PhysicsWorld>()
            .set_gravity(0.0);

        // A few frames so bodies exist and pose_override is populated.
        for _ in 0..5 {
            app.update();
        }

        // Record where the ragdoll has driven the root to.
        let ragdoll_root = skinned_node_positions(&app, owner)[0];

        // Start the return blend by writing the runtime fields directly.
        // In a running game these are set by the ASM bridge in
        // `bsengine-app`; tests in *this* crate drive the physics layer only.
        {
            let mut r = app.world_mut().get_mut::<Ragdoll>(owner).unwrap();
            r.active = false;
            r.return_remaining = blend_duration;
            r.return_duration = blend_duration;
        }

        // Set a fixed time step so the blend ticks predictably.
        {
            let mut t = bsengine_core::Time::default();
            t.set_delta_for_test(0.1);
            app.insert_resource(t);
        }

        (app, owner, ragdoll_root)
    }

    #[test]
    fn returning_from_a_ragdoll_blends_rather_than_snapping() {
        // THE test.  The blend ENDS at the animation pose, so an
        // implementation that skips blending entirely and snaps immediately
        // reaches an identical final state -- only the intermediate frames
        // differ.  A test asserting "after returning the pose is the animated
        // pose" passes on a snap.
        //
        // So sample MID-BLEND and assert the pose is neither the ragdoll pose
        // nor the animated pose.  Without this the whole feature can be a
        // no-op with every other test green.
        //
        // blend_duration = 0.5 s, dt = 0.1 s → blend takes 5 frames.
        // Frame 1 of return: weight = 0.5/0.5 = 1.0 (still ragdoll)
        // Frame 2 of return: weight = 0.4/0.5 = 0.8 (mid-blend)
        // We check after frame 2.
        let (mut app, owner, ragdoll_root) = start_return_blend(0.5);

        // Two frames into the return blend.
        app.update();
        app.update();

        let root = skinned_node_positions(&app, owner)[0];

        // The clip drives the root to CLIP_ROOT.x = 50; the ragdoll left it
        // near REST_POSITIONS[0].x = 0.  Mid-blend the root must be a
        // MEANINGFUL fraction of the way between them.
        //
        // "Strictly between the two" is not enough and was the first version of
        // this assertion: the bones keep settling under the joint solver, so
        // the live physics pose drifts a hair off `ragdoll_root`, and a
        // completely unblended pose satisfies `> ragdoll_root.x` by that noise
        // alone.  Pinning the weight to 1.0 -- no blending at all -- passed.
        // The band below fails on that mutation, which is the only reason this
        // test is worth having.
        //
        // Expected here: two frames in, weight = 0.4/0.5 = 0.8 override, so the
        // root sits about 20% of the way toward the clip.
        let frac = (root.x - ragdoll_root.x) / (CLIP_ROOT.x - ragdoll_root.x);
        assert!(
            (0.05..0.95).contains(&frac),
            "mid-blend: the root should be a real fraction of the way from the \
             ragdoll pose (x = {}) to the clip pose (x = {}), but sits at {:.4} \
             of the way (x = {}). Near 0 means it never blended and snapped at \
             the end; near 1 means it snapped to the animation immediately.",
            ragdoll_root.x,
            CLIP_ROOT.x,
            frac,
            root.x
        );
    }

    #[test]
    fn the_return_blend_finishes_at_the_animated_pose() {
        // The endpoint still has to be right.  Cannot stand alone (see above).
        // blend_duration = 0.5 s, dt = 0.1 s → 5 frames to complete.
        let (mut app, owner, _) = start_return_blend(0.5);

        // Run until the blend timer would have expired (6 frames ≥ 5 needed).
        for _ in 0..6 {
            app.update();
        }

        let root = skinned_node_positions(&app, owner)[0];
        assert!(
            (root.x - CLIP_ROOT.x).abs() < 0.5,
            "after the return blend completes the clip drives the root back to \
             x = {}; got x = {}",
            CLIP_ROOT.x,
            root.x
        );
        assert!(
            app.world()
                .get::<SkinnedMesh>(owner)
                .unwrap()
                .pose_override
                .is_empty(),
            "the pose_override must be cleared once the return blend finishes"
        );
    }

    #[test]
    fn the_ragdoll_bodies_outlive_the_return_blend() {
        // The blend interpolates FROM the ragdoll pose, so the bodies must
        // still exist while it runs.  Assert the bone entities are still
        // present mid-blend, and gone once it completes.
        let (mut app, _owner, _) = start_return_blend(0.5);

        // Mid-blend: bones must still be present.
        app.update(); // frame 1 of return
        assert!(
            !bone_bodies(&mut app).is_empty(),
            "bone entities must still exist during the return blend"
        );

        // Run past the end of the blend.
        for _ in 0..6 {
            app.update();
        }
        assert!(
            bone_bodies(&mut app).is_empty(),
            "bone entities must be despawned once the return blend completes"
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

    // ---- Foot IK ground probe (roadmap item 54, sub-step 1/2) -------------

    /// A character with one IK chain, standing over ground the caller supplies.
    ///
    /// The chain's tip position is published directly rather than derived from
    /// a clip: this crate is testing the PROBE, and making it depend on the
    /// skinning system as well would mean a failure here could be either.
    fn character_over_ground(foot: Vec3, ground_y: f32, slope: bool) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(PhysicsPlugin);

        // Ground: a wide fixed box. Rotated slightly when `slope`, so the two
        // ends sit at different heights.
        let rot = if slope {
            bsengine_core::ReflectQuat(Quat::from_rotation_z(0.2))
        } else {
            Default::default()
        };
        app.world_mut().spawn((
            RigidBody::fixed(),
            Collider::cuboid(50.0, 0.5, 50.0),
            PhysicsInput {
                position: Vec3::new(0.0, ground_y - 0.5, 0.0).into(),
                rotation: rot,
            },
        ));

        let character = app
            .world_mut()
            .spawn((
                SkinnedMesh {
                    mesh_id: 1,
                    rest_vertices: Vec::new(),
                    skin: Vec::new(),
                    skin_data: bsengine_gltf::loader::SkinData {
                        joint_node_indices: Vec::new(),
                        inverse_bind_matrices: Vec::new(),
                    },
                    nodes: Vec::new(),
                    pose_override: Vec::new(),
                    pose_override_weight: 1.0,
                    ik_tip_positions: vec![foot],
                    joint_matrices: Vec::new(),
                },
                IkChains {
                    chains: vec![bsengine_gltf::IkChain {
                        root_bone: "hip".to_string(),
                        mid_bone: "knee".to_string(),
                        tip_bone: "foot".to_string(),
                        target: Vec3::ZERO.into(),
                        weight: 1.0,
                    }],
                },
                FootIkGround::default(),
            ))
            .id();

        // The broad-phase BVH the ray queries is only rebuilt inside `step()`.
        // Without a frame first the probe finds nothing, for a reason that has
        // nothing to do with the probe.
        app.update();
        (app, character)
    }

    fn chain_target(app: &App, e: Entity) -> Vec3 {
        app.world().get::<IkChains>(e).unwrap().chains[0].target.0
    }

    #[test]
    fn a_foot_over_ground_gets_a_target_on_the_surface() {
        // The probe's whole job. Asserts the target sits ON the ground, not
        // merely that it changed from the zero it started at.
        let (mut app, e) = character_over_ground(Vec3::new(0.0, 0.2, 0.0), 0.0, false);
        app.update();
        let t = chain_target(&app, e);
        println!("target landed at {t:?}");
        assert!(
            t.y.abs() < 0.05,
            "the target must sit on the ground at y = 0, not at {}",
            t.y
        );
    }

    #[test]
    fn a_foot_already_sunk_into_the_ground_is_still_found() {
        // The case foot IK exists FOR. On a slope the animation routinely puts
        // the foot inside the hill, and a probe that cast from the foot itself
        // would start below the surface and find nothing -- silently leaving
        // the foot buried, which is exactly the artefact this feature removes.
        let (mut app, e) = character_over_ground(Vec3::new(0.0, -0.2, 0.0), 0.0, false);
        app.update();
        let t = chain_target(&app, e);
        println!("sunk foot resolved to {t:?}");
        assert!(
            t.y.abs() < 0.05,
            "a foot below the surface must still resolve onto it; got y = {}",
            t.y
        );
    }

    #[test]
    fn a_foot_over_nothing_keeps_the_target_it_had() {
        // A character walking off a ledge must not have its feet yanked to
        // wherever the ray gave up.
        let (mut app, e) = character_over_ground(Vec3::new(500.0, 40.0, 500.0), 0.0, false);
        let before = chain_target(&app, e);
        app.update();
        let after = chain_target(&app, e);
        assert_eq!(
            before, after,
            "with no ground beneath it the probe must leave the target alone"
        );
    }

    #[test]
    fn two_feet_at_different_places_on_a_slope_get_different_targets() {
        // What makes this FOOT IK rather than a single global offset. On a
        // slope the two feet must resolve to different heights; on flat ground
        // they would not, which is why the ground is tilted here.
        let mut app = App::new();
        app.add_plugins(PhysicsPlugin);
        app.world_mut().spawn((
            RigidBody::fixed(),
            Collider::cuboid(50.0, 0.5, 50.0),
            PhysicsInput {
                position: Vec3::new(0.0, -0.5, 0.0).into(),
                rotation: bsengine_core::ReflectQuat(Quat::from_rotation_z(0.25)),
            },
        ));
        let character = app
            .world_mut()
            .spawn((
                SkinnedMesh {
                    mesh_id: 1,
                    rest_vertices: Vec::new(),
                    skin: Vec::new(),
                    skin_data: bsengine_gltf::loader::SkinData {
                        joint_node_indices: Vec::new(),
                        inverse_bind_matrices: Vec::new(),
                    },
                    nodes: Vec::new(),
                    pose_override: Vec::new(),
                    pose_override_weight: 1.0,
                    // Two feet, two metres apart across the slope.
                    ik_tip_positions: vec![Vec3::new(-1.0, 0.5, 0.0), Vec3::new(1.0, 0.5, 0.0)],
                    joint_matrices: Vec::new(),
                },
                IkChains {
                    chains: vec![
                        bsengine_gltf::IkChain {
                            tip_bone: "l_foot".to_string(),
                            weight: 1.0,
                            ..Default::default()
                        },
                        bsengine_gltf::IkChain {
                            tip_bone: "r_foot".to_string(),
                            weight: 1.0,
                            ..Default::default()
                        },
                    ],
                },
                FootIkGround::default(),
            ))
            .id();
        app.update();
        app.update();

        let chains = &app.world().get::<IkChains>(character).unwrap().chains;
        let left = chains[0].target.0;
        let right = chains[1].target.0;
        println!("left foot target {left:?}, right foot target {right:?}");
        assert!(
            (left.y - right.y).abs() > 0.1,
            "on a slope the two feet must resolve to different heights: left \
             y = {}, right y = {}. Equal heights mean one offset is being \
             applied to the whole character rather than per foot.",
            left.y,
            right.y
        );
    }

    // ---- Vehicle physics (roadmap item 53, sub-step 1/2) -----------------

    /// A car on flat ground, with `Time` inserted so the controller advances by
    /// a fixed step and the numbers below are reproducible.
    ///
    /// The wheels mount at local y = -0.2, INSIDE the chassis box (local y
    /// spans -0.4..0.4), which is where real wheels sit and what makes the
    /// chassis exclusion in `PhysicsWorld::update_vehicle` load-bearing. The
    /// suspension reaches 0.5 + 0.35 = 0.85 from a mount at world y = 0.8, so
    /// it touches the ground at y = 0 with a little compression.
    fn car_on_flat_ground(throttle: f32, steering: f32, brake: f32) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(PhysicsPlugin);
        let mut t = bsengine_core::Time::default();
        t.set_delta_for_test(1.0 / 60.0);
        app.insert_resource(t);

        // Ground: top face at y = 0.
        app.world_mut().spawn((
            RigidBody::fixed(),
            Collider::cuboid(100.0, 0.5, 100.0),
            PhysicsInput {
                position: Vec3::new(0.0, -0.5, 0.0).into(),
                rotation: Default::default(),
            },
        ));

        let wheels = vec![
            wheel_at(Vec3::new(0.8, -0.2, 1.4), true, false),
            wheel_at(Vec3::new(-0.8, -0.2, 1.4), true, false),
            wheel_at(Vec3::new(0.8, -0.2, -1.4), false, true),
            wheel_at(Vec3::new(-0.8, -0.2, -1.4), false, true),
        ];
        let car = app
            .world_mut()
            .spawn((
                RigidBody::dynamic(),
                Collider::cuboid(0.9, 0.4, 2.0),
                PhysicsInput {
                    position: Vec3::new(0.0, 1.0, 0.0).into(),
                    rotation: Default::default(),
                },
                Vehicle {
                    wheels,
                    throttle,
                    steering,
                    brake,
                    wheel_states: Vec::new(),
                },
            ))
            .id();
        (app, car)
    }

    fn wheel_at(at: Vec3, steers: bool, drives: bool) -> crate::components::WheelConfig {
        let mut w = crate::components::WheelConfig::new(at.into(), 0.35);
        w.suspension_rest_length = 0.5;
        w.steers = steers;
        w.drives = drives;
        w
    }

    fn car_position(app: &App, car: Entity) -> Vec3 {
        app.world()
            .get::<PhysicsTransform>(car)
            .expect("the car keeps its physics transform")
            .position
            .0
    }

    fn car_rotation(app: &App, car: Entity) -> Quat {
        app.world()
            .get::<PhysicsTransform>(car)
            .expect("the car keeps its physics transform")
            .rotation
            .0
    }

    fn drive_for(app: &mut App, frames: usize) {
        for _ in 0..frames {
            app.update();
        }
    }

    /// Spawns four wheel visuals as children of `car`, one per authored wheel.
    fn attach_wheel_visuals(app: &mut App, car: Entity) -> Vec<Entity> {
        (0..4)
            .map(|i| {
                app.world_mut()
                    .spawn((
                        bsengine_core::Parent(car),
                        WheelIndex(i),
                        bsengine_core::Transform::default(),
                    ))
                    .id()
            })
            .collect()
    }

    fn wheel_local(app: &App, e: Entity) -> (Vec3, Quat) {
        let t = app
            .world()
            .get::<bsengine_core::Transform>(e)
            .expect("a wheel visual keeps its transform");
        (t.position.0, t.rotation.0)
    }

    #[test]
    fn suspension_moves_the_wheel_and_parking_does_not() {
        // A wheel must ride its suspension, and must NOT jitter while parked.
        // Without the second half a wheel whose transform is rewritten with
        // noise every frame passes the first.
        let (mut app, car) = car_on_flat_ground(0.0, 0.0, 0.0);
        let visuals = attach_wheel_visuals(&mut app, car);

        // Dropped from y = 1.0, the car settles onto its springs: the wheel's
        // offset below its mount has to change while that happens.
        drive_for(&mut app, 2);
        let early = wheel_local(&app, visuals[0]).0;
        drive_for(&mut app, 60);
        let settled = wheel_local(&app, visuals[0]).0;
        let travel = (settled.y - early.y).abs();
        println!("wheel dropped {travel} m onto its suspension");
        assert!(
            travel > 0.01,
            "the wheel must ride the suspension as the car settles; it moved \
             {travel} m, which is a transform nothing is driving"
        );

        // Now let the spring oscillation damp out before measuring stillness.
        // A light chassis on a stiff spring is still ringing ~9mm at 60 frames,
        // which is physics doing its job, not the transform being driven by
        // noise -- so this waits rather than widening the bound.
        drive_for(&mut app, 240);
        let before = wheel_local(&app, visuals[0]).0;
        drive_for(&mut app, 60);
        let after = wheel_local(&app, visuals[0]).0;
        let drift = (after.y - before.y).abs();
        println!("parked wheel drifted {drift} m");
        assert!(
            drift < 0.005,
            "a parked car's wheel must hold its height; it moved {drift} m"
        );
    }

    #[test]
    fn steering_turns_the_front_wheels_and_leaves_the_rear_alone() {
        // THE load-bearing assertion of this task, and the PAIRING is the
        // assertion. A bug that steers all four wheels passes "the front
        // wheels turned"; a bug that steers none passes "the rear wheels did
        // not turn". Neither half means anything by itself.
        //
        // `car_on_flat_ground` authors wheels 0 and 1 as `steers`, 2 and 3 as
        // `drives`, so the split is the scene's, not this test's.
        let (mut app, car) = car_on_flat_ground(20.0, 0.6, 0.0);
        let visuals = attach_wheel_visuals(&mut app, car);
        drive_for(&mut app, 30);

        // Measure steering by where the AXLE ends up, not by an Euler angle.
        // The wheel's rotation is `steer(Y) * roll(X)`, and roll about X leaves
        // the X axis fixed -- so `rotation * X` isolates the steering exactly.
        // Reading `to_euler(...).0` instead reports pi once roll accumulates
        // past half a turn, which is a decomposition artefact and not yaw: the
        // first version of this test failed with the rear wheel at exactly
        // 3.1415927 rad while its steering was genuinely zero.
        let axle_yaw = |q: Quat| {
            let a = q * Vec3::X;
            a.z.atan2(a.x).abs()
        };
        let front_yaw = axle_yaw(wheel_local(&app, visuals[0]).1);
        let rear_yaw = axle_yaw(wheel_local(&app, visuals[2]).1);
        println!("front wheel yaw {front_yaw} rad, rear wheel yaw {rear_yaw} rad");

        assert!(
            front_yaw > 0.05,
            "a steered wheel must yaw; the front wheel is at {front_yaw} rad"
        );
        assert!(
            rear_yaw < 1e-4,
            "a non-steering wheel must NOT yaw; the rear wheel is at \
             {rear_yaw} rad. If both are turning, steering is being applied to \
             every wheel rather than the ones authored `steers`"
        );
    }

    #[test]
    fn the_wheels_roll_while_driving_and_stop_when_parked() {
        // Roll must accumulate under throttle and hold when stopped. A wheel
        // that spins on a parked car is as wrong as one that never turns.
        // Measured on the published scalar, not the quaternion. Roll
        // accumulates without bound while a quaternion wraps every 2*pi, and
        // `Quat::angle_between` is capped at pi -- a wheel that spun several
        // full turns reads as a small angle. The first version of this test
        // reported 2.94 rad for a wheel that had actually rolled much further.
        let (mut app, car) = car_on_flat_ground(20.0, 0.0, 0.0);
        let visuals = attach_wheel_visuals(&mut app, car);
        let roll = |app: &App| app.world().get::<Vehicle>(car).unwrap().wheel_states[0].rotation;
        drive_for(&mut app, 60);
        let a = roll(&app);
        drive_for(&mut app, 60);
        let driving_delta = (roll(&app) - a).abs();
        println!("driving wheel turned {driving_delta} rad over 60 frames");
        assert!(
            driving_delta > 0.1,
            "a driven wheel must keep rolling; it turned {driving_delta} rad"
        );

        // Brake and compare the RATE. Braking does not bring this car to a dead
        // stop in any reasonable window -- `braking_slows_a_rolling_car`
        // measures it still moving at ~1.1 m/s -- so asserting "stopped" would
        // be asserting something untrue about the physics rather than about the
        // wheels.
        {
            let mut v = app.world_mut().get_mut::<Vehicle>(car).unwrap();
            v.throttle = 0.0;
            v.brake = 500.0;
        }
        drive_for(&mut app, 90);
        let b = roll(&app);
        drive_for(&mut app, 60);
        let braked_delta = (roll(&app) - b).abs();
        println!("braked wheel turned {braked_delta} rad over the same 60 frames");
        assert!(
            braked_delta < driving_delta * 0.5,
            "a braked wheel must roll markedly slower: {braked_delta} rad              against {driving_delta} rad while driving"
        );

        // And the visual must actually reflect that roll, not just the scalar.
        let (_, rot) = wheel_local(&app, visuals[0]);
        assert!(
            rot.angle_between(Quat::IDENTITY) > 1e-3,
            "the wheel visual must carry the accumulated roll"
        );
    }

    #[test]
    fn a_wheel_visual_past_the_end_of_the_wheel_list_is_skipped() {
        // A car may be authored with fewer visuals than wheels, or more. The
        // extra must be left alone rather than panicking on an out-of-range
        // index.
        let (mut app, car) = car_on_flat_ground(0.0, 0.0, 0.0);
        let stray = app
            .world_mut()
            .spawn((
                bsengine_core::Parent(car),
                WheelIndex(99),
                bsengine_core::Transform::default(),
            ))
            .id();
        drive_for(&mut app, 10);
        let (pos, _) = wheel_local(&app, stray);
        assert_eq!(
            pos,
            Vec3::ZERO,
            "an out-of-range wheel visual must be left untouched"
        );
    }

    #[test]
    fn a_driving_car_publishes_its_wheel_state() {
        // The controller computes suspension length, steering and roll every
        // frame, and sub-step 1/2 threw all three away. This is the channel
        // that keeps them, so it has to carry real values.
        //
        // Assert the state CHANGES: a `WheelState::default()` published every
        // frame satisfies "the field exists and has four entries", which is
        // what a disconnected publish looks like from the outside.
        let (mut app, car) = car_on_flat_ground(20.0, 0.3, 0.0);
        drive_for(&mut app, 1);
        let first = app
            .world()
            .get::<Vehicle>(car)
            .unwrap()
            .wheel_states
            .clone();
        assert_eq!(first.len(), 4, "one state per authored wheel");

        drive_for(&mut app, 60);
        let later = app
            .world()
            .get::<Vehicle>(car)
            .unwrap()
            .wheel_states
            .clone();

        assert!(
            later.iter().any(|w| w.grounded),
            "a car resting on the ground must report at least one wheel in              contact; none did, so the raycasts are not reaching the floor"
        );
        assert!(
            later[0].rotation.abs() > 0.5,
            "the wheels must accumulate roll while driving; wheel 0 turned              {} rad, which is a publish of zeros rather than real state",
            later[0].rotation
        );
        // Front wheels steer, rear do not -- the layout `car_on_flat_ground`
        // authors. This is what catches a publish that reads the wrong wheel.
        assert!(
            later[0].steering.abs() > 0.01 && later[2].steering.abs() < 1e-6,
            "steering must be published per wheel and follow the authored              layout: front {} rad, rear {} rad",
            later[0].steering,
            later[2].steering
        );
        assert!(
            later[0].suspension_length > 0.0,
            "a grounded wheel must publish a real suspension length, got {}",
            later[0].suspension_length
        );
        let _ = first;
    }

    #[test]
    fn throttle_drives_the_car_forward() {
        // The headline assertion of the whole feature.
        //
        // The bound is METRES, deliberately. An earlier feature in this repo
        // shipped a test whose bound was satisfied by ~2e-12 of solver settling
        // noise and passed with the feature disabled; "it moved" has to mean
        // moved.
        //
        // Measured: 14.0 m against a 1.0 m bound, versus 0.034 m for the
        // no-throttle pair -- a ~400x separation, so neither test is anywhere
        // near its threshold. Cutting the `engine_force` assignment in
        // `sync_vehicles` drops this to 0.03444338 m, byte-identical to the
        // idle run, which is what proves the distance comes from the throttle
        // path and not from rolling or settling.
        let (mut app, car) = car_on_flat_ground(20.0, 0.0, 0.0);
        drive_for(&mut app, 1);
        let start = car_position(&app, car);
        drive_for(&mut app, 120);
        let travelled = (car_position(&app, car) - start).length();
        println!("throttle run travelled {travelled} m in 120 frames");
        assert!(
            travelled > 1.0,
            "a car under throttle must cover real ground in two seconds; it \
             moved {travelled} m, which is settling, not driving"
        );
    }

    #[test]
    fn a_car_with_no_throttle_stays_put() {
        // The pair. Without it, an implementation that applies engine force
        // unconditionally -- or one that ignores `drives` -- passes the test
        // above. The tolerance covers the suspension settling onto its springs
        // and is far below the metre the driving test demands.
        let (mut app, car) = car_on_flat_ground(0.0, 0.0, 0.0);
        drive_for(&mut app, 1);
        let start = car_position(&app, car);
        drive_for(&mut app, 120);
        let drift = (car_position(&app, car) - start).length();
        println!("idle run drifted {drift} m in 120 frames");
        assert!(
            drift < 0.25,
            "a car with no throttle must stay where it is; it moved {drift} m"
        );
    }

    #[test]
    fn braking_slows_a_rolling_car() {
        // Two runs from the same rolling start, one braking and one not. A
        // single-run "the speed went down" test also passes on plain friction,
        // which is why the control run is the assertion.
        let roll = |brake: f32| {
            let (mut app, car) = car_on_flat_ground(20.0, 0.0, 0.0);
            drive_for(&mut app, 60);
            {
                let mut v = app.world_mut().get_mut::<Vehicle>(car).unwrap();
                v.throttle = 0.0;
                v.brake = brake;
            }
            drive_for(&mut app, 60);
            let world = app.world().resource::<PhysicsWorld>();
            world.get_linvel(car).unwrap_or(Vec3::ZERO).length()
        };
        let coasting = roll(0.0);
        let braking = roll(50.0);
        println!("coasting ended at {coasting} m/s, braking at {braking} m/s");
        assert!(
            braking < coasting * 0.75,
            "braking must slow the car materially more than coasting does: \
             coasting ended at {coasting} m/s, braking at {braking} m/s"
        );
    }

    #[test]
    fn steering_changes_the_heading_not_just_the_position() {
        // Asserts on ROTATION. A car shoved sideways also changes position, so
        // position alone does not show that steering works.
        let (mut app, car) = car_on_flat_ground(20.0, 0.5, 0.0);
        drive_for(&mut app, 1);
        let start = car_rotation(&app, car);
        drive_for(&mut app, 120);
        let ended = car_rotation(&app, car);
        let turned = start.angle_between(ended).to_degrees();
        println!("steered run turned {turned} degrees");
        assert!(
            turned > 5.0,
            "a steering car must actually change heading; it turned {turned} \
             degrees, which is body roll, not steering"
        );
    }

    #[test]
    fn suspension_holds_the_chassis_off_the_ground() {
        // What distinguishes working suspension from a box lying on the floor.
        // The chassis half-height is 0.4, so resting directly on the ground
        // would put its centre at y = 0.4; the suspension should hold it
        // meaningfully higher.
        let (mut app, car) = car_on_flat_ground(0.0, 0.0, 0.0);
        drive_for(&mut app, 180);
        let y = car_position(&app, car).y;
        println!("chassis settled at y = {y}");
        assert!(
            y > 0.55,
            "the suspension must hold the chassis clear of the ground; its \
             centre settled at y = {y}, and 0.4 is the collider resting flat"
        );
    }

    #[test]
    fn a_vehicle_whose_component_is_removed_loses_its_controller() {
        // Mirrors `sync_ragdolls`' stale sweep: without it the side table grows
        // forever, and a re-added `Vehicle` inherits the old wheels.
        let (mut app, car) = car_on_flat_ground(0.0, 0.0, 0.0);
        drive_for(&mut app, 2);
        app.world_mut().entity_mut(car).remove::<Vehicle>();
        drive_for(&mut app, 2);
        // Re-adding must give a fresh controller rather than reviving the old
        // one: a stale controller holding four wheels indexed against this
        // one-wheel config is the bug this guards.
        app.world_mut().entity_mut(car).insert(Vehicle {
            wheels: vec![wheel_at(Vec3::new(0.0, -0.2, 0.0), false, true)],
            throttle: 0.0,
            steering: 0.0,
            brake: 0.0,
            wheel_states: Vec::new(),
        });
        drive_for(&mut app, 2);
        assert!(app.world().get::<Vehicle>(car).is_some());
    }
}
