use std::collections::HashMap;

use bevy_ecs::prelude::{Entity, Resource};
use glam::Vec3;
use rapier3d::pipeline::EventHandler;
use rapier3d::prelude::*;

use crate::components::{JointKind, RaycastHit};

/// The Rapier simulation state: rigid bodies, colliders, and the pipeline that steps them.
#[derive(Resource)]
pub struct PhysicsWorld {
    pub(crate) rigid_body_set: RigidBodySet,
    pub(crate) collider_set: ColliderSet,
    pub(crate) collider_entity_map: HashMap<ColliderHandle, Entity>,
    pub(crate) entity_body_map: HashMap<Entity, RigidBodyHandle>,
    gravity: Vector,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    /// The reverse of `impulse_joint_set`: which handle a given pair of
    /// entities produced, so `remove_joint` can find what `add_joint` created.
    /// Rapier's set is keyed by its own handle and offers no lookup by body
    /// pair, and a caller only ever has the two entities.
    joint_map: HashMap<(Entity, Entity), ImpulseJointHandle>,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new(9.81)
    }
}

impl PhysicsWorld {
    /// Creates a new empty world with downward gravity of the given magnitude (m/s²).
    pub fn new(gravity_magnitude: f32) -> Self {
        Self {
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            collider_entity_map: HashMap::new(),
            entity_body_map: HashMap::new(),
            gravity: Vector::new(0.0, -gravity_magnitude, 0.0),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            joint_map: HashMap::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
        }
    }

    /// Advances the simulation by one timestep, reporting contact events via `event_handler`.
    ///
    /// Forces and torques are cleared afterwards, so a force applies for the one
    /// step it was added to. Rapier itself keeps them until told otherwise --
    /// its `add_force` describes a thruster that stays on, not a push. The
    /// clearing happens here rather than in a system so that every caller of
    /// `step` gets the same physics, tests included.
    pub fn step(&mut self, event_handler: &dyn EventHandler) {
        self.physics_pipeline.step(
            self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &(),
            event_handler,
        );

        // `false` = do not wake sleeping bodies. Clearing a force a sleeping
        // body is not feeling should not be what wakes it up.
        for (_, body) in self.rigid_body_set.iter_mut() {
            body.reset_forces(false);
            body.reset_torques(false);
        }
    }

    pub(crate) fn add_collider(
        &mut self,
        coll: rapier3d::geometry::Collider,
        body_handle: RigidBodyHandle,
    ) -> ColliderHandle {
        self.collider_set
            .insert_with_parent(coll, body_handle, &mut self.rigid_body_set)
    }

    /// Returns the current gravity magnitude (m/s²), always pointing down along -Y.
    pub fn gravity(&self) -> f32 {
        -self.gravity.y
    }

    /// Sets the gravity magnitude (m/s²), applied downward along -Y.
    pub fn set_gravity(&mut self, magnitude: f32) {
        self.gravity = Vector::new(0.0, -magnitude, 0.0);
    }

    pub(crate) fn register_entity_body(&mut self, entity: Entity, handle: RigidBodyHandle) {
        self.entity_body_map.insert(entity, handle);
    }

    /// Returns the entity's linear velocity, or `None` if it has no physics body.
    pub fn get_linvel(&self, entity: Entity) -> Option<Vec3> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        let v = body.linvel();
        Some(Vec3::new(v.x, v.y, v.z))
    }

    /// Sets the entity's linear velocity directly, waking the body if it was asleep.
    pub fn set_linvel(&mut self, entity: Entity, vel: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_linvel(Vector::new(vel.x, vel.y, vel.z), true);
            }
        }
    }

    /// Applies an instantaneous linear impulse to the entity's body, waking it if asleep.
    pub fn apply_impulse(&mut self, entity: Entity, impulse: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.apply_impulse(Vector::new(impulse.x, impulse.y, impulse.z), true);
            }
        }
    }

    /// Applies a linear impulse at a specific world-space point, inducing torque if off-center.
    pub fn apply_impulse_at_point(&mut self, entity: Entity, impulse: Vec3, point: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.apply_impulse_at_point(
                    Vector::new(impulse.x, impulse.y, impulse.z),
                    Vector::new(point.x, point.y, point.z),
                    true,
                );
            }
        }
    }

    /// Applies a continuous force to the entity's body for the current step, waking it if asleep.
    pub fn apply_force(&mut self, entity: Entity, force: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.add_force(Vector::new(force.x, force.y, force.z), true);
            }
        }
    }

    /// Applies a continuous force at a specific world-space point, inducing torque if off-center.
    pub fn apply_force_at_point(&mut self, entity: Entity, force: Vec3, point: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.add_force_at_point(
                    Vector::new(force.x, force.y, force.z),
                    Vector::new(point.x, point.y, point.z),
                    true,
                );
            }
        }
    }

    /// Returns the entity's angular velocity, or `None` if it has no physics body.
    pub fn get_angvel(&self, entity: Entity) -> Option<Vec3> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        let v = body.angvel();
        Some(Vec3::new(v.x, v.y, v.z))
    }

    /// Sets the entity's angular velocity directly, waking the body if it was asleep.
    pub fn set_angvel(&mut self, entity: Entity, vel: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_angvel(Vector::new(vel.x, vel.y, vel.z), true);
            }
        }
    }

    /// Teleports a body's actual Rapier position, not just the `Transform`
    /// component. For a `Dynamic` body, `Transform` is overwritten every
    /// frame from the simulated Rapier position (see
    /// `sync_transform_from_physics` in bsengine-runtime's physics plugin),
    /// so a script calling `Bsengine.setPosition` on one needs this to make
    /// the teleport actually stick instead of being silently undone next
    /// frame.
    pub fn set_translation(&mut self, entity: Entity, pos: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_translation(Vector::new(pos.x, pos.y, pos.z), true);
            }
        }
    }

    /// Rotation counterpart to [`Self::set_translation`] — see its doc for
    /// why `Dynamic` bodies need this instead of only writing `Transform`.
    pub fn set_rotation(&mut self, entity: Entity, rot: glam::Quat) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_rotation(Rotation::from_xyzw(rot.x, rot.y, rot.z, rot.w), true);
            }
        }
    }

    /// Discards force/torque added earlier in the current frame, before
    /// [`Self::step`] gets to apply it.
    ///
    /// Narrow by design. Forces do not survive a step -- `step` clears them --
    /// so this is only for the case where something already queued a force this
    /// frame and something else then decides the body should not move at all: a
    /// teleport, a "game over" freeze. Outside that window it does nothing.
    pub fn reset_forces(&mut self, entity: Entity) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.reset_forces(false);
                body.reset_torques(false);
            }
        }
    }

    /// Applies an instantaneous angular impulse (torque) to the entity's body.
    pub fn apply_torque_impulse(&mut self, entity: Entity, impulse: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.apply_torque_impulse(Vector::new(impulse.x, impulse.y, impulse.z), true);
            }
        }
    }

    /// Applies a continuous torque to the entity's body for the current step, waking it if asleep.
    pub fn add_torque(&mut self, entity: Entity, torque: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.add_torque(Vector::new(torque.x, torque.y, torque.z), true);
            }
        }
    }

    /// Enables or disables continuous collision detection, preventing fast bodies from tunneling.
    pub fn set_ccd_enabled(&mut self, entity: Entity, enabled: bool) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.enable_ccd(enabled);
            }
        }
    }

    /// Sets how quickly the entity's linear velocity decays over time.
    pub fn set_linear_damping(&mut self, entity: Entity, damping: f32) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_linear_damping(damping);
            }
        }
    }

    /// Sets how quickly the entity's angular velocity decays over time.
    pub fn set_angular_damping(&mut self, entity: Entity, damping: f32) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_angular_damping(damping);
            }
        }
    }

    /// Returns the entity's linear velocity damping factor, or `None` if it has no physics body.
    pub fn get_linear_damping(&self, entity: Entity) -> Option<f32> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        Some(body.linear_damping())
    }

    /// Returns the entity's angular velocity damping factor, or `None` if it has no physics body.
    pub fn get_angular_damping(&self, entity: Entity) -> Option<f32> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        Some(body.angular_damping())
    }

    /// Returns the entity's total mass, or `None` if it has no physics body.
    pub fn get_mass(&self, entity: Entity) -> Option<f32> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        Some(body.mass())
    }

    /// Overrides the entity's mass, replacing what its colliders' density would otherwise compute.
    pub fn set_mass(&mut self, entity: Entity, mass: f32) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_additional_mass(mass, true);
            }
        }
    }

    /// Locks or unlocks rotation of the entity's body around each world axis.
    pub fn lock_rotations(&mut self, entity: Entity, lock_x: bool, lock_y: bool, lock_z: bool) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_enabled_rotations(!lock_x, !lock_y, !lock_z, true);
            }
        }
    }

    /// Locks or unlocks translation of the entity's body along each world axis.
    pub fn lock_translations(&mut self, entity: Entity, lock_x: bool, lock_y: bool, lock_z: bool) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_enabled_translations(!lock_x, !lock_y, !lock_z, true);
            }
        }
    }

    /// Returns whether the entity's body is currently asleep (excluded from active simulation).
    pub fn is_sleeping(&self, entity: Entity) -> Option<bool> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        Some(body.is_sleeping())
    }

    /// Forces the entity's body to wake up if it was sleeping.
    pub fn wake_up(&mut self, entity: Entity) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.wake_up(true);
            }
        }
    }

    /// Forces the entity's body to sleep immediately, excluding it from active simulation.
    pub fn put_to_sleep(&mut self, entity: Entity) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.sleep();
            }
        }
    }

    /// Returns the restitution (bounciness) of the entity's first collider, or `None` if absent.
    pub fn get_restitution(&self, entity: Entity) -> Option<f32> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        let coll_handle = *body.colliders().first()?;
        let collider = self.collider_set.get(coll_handle)?;
        Some(collider.restitution())
    }

    /// Sets the restitution (bounciness) on every collider attached to the entity's body.
    pub fn set_restitution(&mut self, entity: Entity, restitution: f32) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get(handle) {
                for &coll_handle in body.colliders() {
                    if let Some(collider) = self.collider_set.get_mut(coll_handle) {
                        collider.set_restitution(restitution);
                    }
                }
            }
        }
    }

    /// Returns the friction coefficient of the entity's first collider, or `None` if absent.
    pub fn get_friction(&self, entity: Entity) -> Option<f32> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        let coll_handle = *body.colliders().first()?;
        let collider = self.collider_set.get(coll_handle)?;
        Some(collider.friction())
    }

    /// Sets the friction coefficient on every collider attached to the entity's body.
    pub fn set_friction(&mut self, entity: Entity, friction: f32) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get(handle) {
                for &coll_handle in body.colliders() {
                    if let Some(collider) = self.collider_set.get_mut(coll_handle) {
                        collider.set_friction(friction);
                    }
                }
            }
        }
    }

    /// Sets sensor mode (overlap detection without physical response) on every collider on the entity's body.
    pub fn set_collider_sensor(&mut self, entity: Entity, sensor: bool) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get(handle) {
                for &coll_handle in body.colliders() {
                    if let Some(collider) = self.collider_set.get_mut(coll_handle) {
                        collider.set_sensor(sensor);
                    }
                }
            }
        }
    }

    /// Returns the entity's per-body gravity multiplier, or `None` if it has no physics body.
    pub fn get_gravity_scale(&self, entity: Entity) -> Option<f32> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        Some(body.gravity_scale())
    }

    /// Returns whether the entity's body is kinematic (position-driven, not force-driven).
    pub fn is_kinematic(&self, entity: Entity) -> Option<bool> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        Some(body.is_kinematic())
    }

    /// Returns whether the entity's first collider is a sensor, or `None` if absent.
    pub fn is_collider_sensor(&self, entity: Entity) -> Option<bool> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        let coll_handle = *body.colliders().first()?;
        let collider = self.collider_set.get(coll_handle)?;
        Some(collider.is_sensor())
    }

    /// Sets the entity's per-body gravity multiplier (1.0 = normal gravity, 0.0 = unaffected).
    pub fn set_gravity_scale(&mut self, entity: Entity, scale: f32) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_gravity_scale(scale, true);
            }
        }
    }

    /// Switches the entity's body between dynamic and kinematic-position-based simulation.
    pub fn set_body_type(&mut self, entity: Entity, kinematic: bool) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                let body_type = if kinematic {
                    RigidBodyType::KinematicPositionBased
                } else {
                    RigidBodyType::Dynamic
                };
                body.set_body_type(body_type, true);
            }
        }
    }

    /// Cast a ray into the physics world, ignoring one entity's own body.
    ///
    /// A ground check casts downward from inside the character it is checking,
    /// so without the exclusion the first thing the ray meets is the character
    /// itself and every character reports standing on something forever —
    /// including one falling through empty space. Excluding at the query rather
    /// than filtering the result also means a self-hit cannot mask the real
    /// surface behind it.
    pub fn cast_ray_excluding(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_dist: f32,
        exclude: Entity,
    ) -> Option<RaycastHit> {
        let filter = match self.entity_body_map.get(&exclude) {
            Some(&handle) => QueryFilter::default().exclude_rigid_body(handle),
            None => QueryFilter::default(),
        };
        self.cast_ray_filtered(origin, dir, max_dist, filter)
    }

    /// Cast a ray into the physics world. Returns hit info or None.
    pub fn cast_ray(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RaycastHit> {
        self.cast_ray_filtered(origin, dir, max_dist, QueryFilter::default())
    }

    /// Rebuilds `entity`'s collider shape in place, keeping the same
    /// `ColliderHandle` (so nothing referencing it, e.g. `PhysicsHandles`,
    /// needs to change) -- the first place in this engine that mutates a
    /// collider's shape after it was originally spawned. Returns whether
    /// `entity` had a body with an attached collider to update.
    pub fn set_collider_shape(
        &mut self,
        entity: Entity,
        shape: &crate::components::ColliderShape,
    ) -> bool {
        let Some(&body_handle) = self.entity_body_map.get(&entity) else {
            return false;
        };
        let Some(body) = self.rigid_body_set.get(body_handle) else {
            return false;
        };
        let Some(&collider_handle) = body.colliders().first() else {
            return false;
        };
        let Some(collider) = self.collider_set.get_mut(collider_handle) else {
            return false;
        };
        collider.set_shape(crate::plugin::make_shape(shape));
        true
    }

    /// Constrains `a`'s body to `b`'s, with `anchor_a`/`anchor_b` given in each
    /// body's own local space.
    ///
    /// Returns false when either entity has no rigid body, matching
    /// [`Self::set_collider_shape`]'s convention that a stale entity is a
    /// value-level failure rather than a panic — a scene naming an entity that
    /// was never spawned, or a script holding an id that has since despawned,
    /// should degrade to a missing joint and a warning.
    ///
    /// Nothing else has to happen for the joint to take effect: [`Self::step`]
    /// already hands its `ImpulseJointSet` to the pipeline, so the constraint
    /// is simulated from the very next step.
    pub fn add_joint(
        &mut self,
        a: Entity,
        b: Entity,
        kind: &JointKind,
        anchor_a: Vec3,
        anchor_b: Vec3,
    ) -> bool {
        let (Some(&handle_a), Some(&handle_b)) =
            (self.entity_body_map.get(&a), self.entity_body_map.get(&b))
        else {
            return false;
        };

        let anchor1 = Vector::new(anchor_a.x, anchor_a.y, anchor_a.z);
        let anchor2 = Vector::new(anchor_b.x, anchor_b.y, anchor_b.z);
        let data: GenericJoint = match kind {
            JointKind::Fixed => FixedJointBuilder::new()
                .local_anchor1(anchor1)
                .local_anchor2(anchor2)
                .build()
                .into(),
            JointKind::Revolute { axis, limits } => {
                let mut builder = RevoluteJointBuilder::new(Vector::new(axis.x, axis.y, axis.z))
                    .local_anchor1(anchor1)
                    .local_anchor2(anchor2);
                if let Some(limits) = limits {
                    builder = builder.limits(*limits);
                }
                builder.build().into()
            }
            JointKind::Spherical => SphericalJointBuilder::new()
                .local_anchor1(anchor1)
                .local_anchor2(anchor2)
                .build()
                .into(),
        };

        // Re-jointing a pair replaces rather than stacks. Without this the old
        // handle would stay in Rapier's set with nothing left pointing at it:
        // two contradictory constraints fighting each other, and a
        // `remove_joint` that could only ever undo one of them.
        self.remove_joint(a, b);

        let handle = self
            .impulse_joint_set
            .insert(handle_a, handle_b, data, true);
        self.joint_map.insert((a, b), handle);
        true
    }

    /// Removes the joint linking `a` and `b`, returning whether there was one.
    ///
    /// Either order finds it. A joint is symmetric, and a caller — a script
    /// detaching one, a despawn sweep cleaning up — has no reason to know
    /// which of the two entities [`Self::add_joint`] happened to be given
    /// first.
    pub fn remove_joint(&mut self, a: Entity, b: Entity) -> bool {
        let Some(handle) = self
            .joint_map
            .remove(&(a, b))
            .or_else(|| self.joint_map.remove(&(b, a)))
        else {
            return false;
        };
        self.impulse_joint_set.remove(handle, true);
        true
    }

    /// Whether a joint currently links `a` and `b`, in either order.
    ///
    /// The read-only counterpart to [`Self::add_joint`]/[`Self::remove_joint`],
    /// which report only what they just did. A caller that wants to know
    /// whether the simulation actually holds a constraint — a test asserting a
    /// script's `attach`/`detach` reached the world, a system deciding whether
    /// to re-create one — otherwise has to remove the joint to find out.
    pub fn has_joint(&self, a: Entity, b: Entity) -> bool {
        self.joint_map.contains_key(&(a, b)) || self.joint_map.contains_key(&(b, a))
    }

    /// Takes `entity`'s rigid body, its colliders, and every joint attached to
    /// it out of the simulation, returning whether there was one.
    ///
    /// Despawning an entity does *not* do this on its own — `spawn_bodies` has
    /// no despawn-time counterpart — so a body whose entity is gone otherwise
    /// keeps falling, keeps colliding with the level, and keeps reporting
    /// contacts against a collider handle that maps to nothing. That is
    /// tolerable for entities that live as long as the scene; it is not
    /// tolerable for a ragdoll, whose whole point is to be built and torn down
    /// while the game runs.
    ///
    /// The three maps are cleaned in step with Rapier's sets. A `joint_map`
    /// entry left behind would make [`Self::has_joint`] claim a constraint
    /// that no longer exists and hand [`Self::remove_joint`] a dangling
    /// handle; a `collider_entity_map` entry left behind would attribute a
    /// later contact to a dead entity.
    pub fn remove_body(&mut self, entity: Entity) -> bool {
        let Some(handle) = self.entity_body_map.remove(&entity) else {
            return false;
        };
        self.collider_entity_map.retain(|_, e| *e != entity);
        self.joint_map
            .retain(|&(a, b), _| a != entity && b != entity);
        self.rigid_body_set
            .remove(
                handle,
                &mut self.island_manager,
                &mut self.collider_set,
                &mut self.impulse_joint_set,
                &mut self.multibody_joint_set,
                true,
            )
            .is_some()
    }

    /// Restricts which other colliders `entity`'s collider interacts with.
    ///
    /// Crate-internal: the only caller is the ragdoll, which puts a
    /// character's own bones in a group that does not collide with itself.
    /// Exposing collision filtering as an authoring concept is a bigger
    /// decision than this needs (it would want a `Collider` field, a scene
    /// syntax, and a name for each group), and nothing outside this crate has
    /// asked for one yet.
    pub(crate) fn set_collision_groups(&mut self, entity: Entity, groups: InteractionGroups) {
        let Some(&body_handle) = self.entity_body_map.get(&entity) else {
            return;
        };
        let Some(body) = self.rigid_body_set.get(body_handle) else {
            return;
        };
        for &collider_handle in body.colliders() {
            if let Some(collider) = self.collider_set.get_mut(collider_handle) {
                collider.set_collision_groups(groups);
            }
        }
    }

    fn cast_ray_filtered(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_dist: f32,
        filter: QueryFilter,
    ) -> Option<RaycastHit> {
        // QueryPipeline<'a> borrows the sets so it is constructed per-call from the broad phase.
        let qp = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.rigid_body_set,
            &self.collider_set,
            filter,
        );
        let ray = Ray::new(
            Vector::new(origin.x, origin.y, origin.z),
            Vector::new(dir.x, dir.y, dir.z),
        );
        qp.cast_ray_and_get_normal(&ray, max_dist, true)
            .map(|(handle, intersection)| {
                let t = intersection.time_of_impact;
                let p = ray.origin + ray.dir * t;
                let n = intersection.normal;
                RaycastHit {
                    entity: self.collider_entity_map.get(&handle).copied(),
                    point: Vec3::new(p.x, p.y, p.z),
                    normal: Vec3::new(n.x, n.y, n.z),
                    distance: t,
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_collider_shape_changes_where_a_raycast_hits() {
        let mut world = PhysicsWorld::new(9.81);
        // A flat box collider at the origin, top surface at y = 0.5.
        let entity = bevy_ecs::prelude::Entity::from_raw(0);
        let body = rapier3d::prelude::RigidBodyBuilder::fixed().build();
        let body_handle = world.rigid_body_set.insert(body);
        let shape = crate::plugin::make_shape(&crate::components::ColliderShape::Box {
            half_extents: Vec3::new(1.0, 0.5, 1.0).into(),
        });
        let collider = rapier3d::prelude::ColliderBuilder::new(shape).build();
        let collider_handle = world.add_collider(collider, body_handle);
        world.collider_entity_map.insert(collider_handle, entity);
        world.register_entity_body(entity, body_handle);

        // The broad-phase BVH that `cast_ray` queries is only rebuilt inside
        // `step()` (see `PhysicsPipeline::step`'s call to `broad_phase.update`
        // in the vendored rapier3d source) -- inserting a collider directly
        // via `add_collider` does not register it in the tree by itself. The
        // body is fixed, so stepping does not move it; `&()` is rapier's
        // built-in no-op `EventHandler`.
        world.step(&());

        let before = world.cast_ray(Vec3::new(0.0, 10.0, 0.0), Vec3::new(0.0, -1.0, 0.0), 100.0);
        assert!(
            (before.as_ref().unwrap().point.y - 0.5).abs() < 1e-4,
            "sanity: ray should hit the original box's top face at y=0.5, got {:?}",
            before
        );

        let changed = world.set_collider_shape(
            entity,
            &crate::components::ColliderShape::Box {
                half_extents: Vec3::new(1.0, 2.0, 1.0).into(),
            },
        );
        assert!(
            changed,
            "set_collider_shape must report success for a real entity"
        );

        // Same reason as above: the shape change needs a step to propagate
        // into the broad-phase tree before a raycast will see it.
        world.step(&());

        let after = world.cast_ray(Vec3::new(0.0, 10.0, 0.0), Vec3::new(0.0, -1.0, 0.0), 100.0);
        assert!(
            (after.as_ref().unwrap().point.y - 2.0).abs() < 1e-4,
            "after set_collider_shape, a raycast must hit the NEW shape's top face at y=2.0, \
             got {:?} -- the collider was not actually rebuilt",
            after
        );
    }

    #[test]
    fn set_collider_shape_reports_failure_for_an_unknown_entity() {
        let mut world = PhysicsWorld::new(9.81);
        let unknown = bevy_ecs::prelude::Entity::from_raw(999);
        assert!(!world.set_collider_shape(
            unknown,
            &crate::components::ColliderShape::Sphere { radius: 1.0 }
        ));
    }

    // ---------------------------------------------------------------- joints
    //
    // Every joint test builds its bodies straight into the sets rather than
    // through `PhysicsPlugin`, the same way the `set_collider_shape` tests
    // above do: the claim is about `PhysicsWorld` itself, and going through a
    // plugin app would put spawn ordering and `PhysicsInput` syncing between
    // the setup and the assertion. Gravity is zero in all of them, so the only
    // things that can move a body are the test's own push and the joint under
    // test.

    /// A fixed body with no collider — the immovable end of a joint.
    ///
    /// No collider on purpose: nothing in these tests should ever *touch* the
    /// anchor, so the only thing connecting it to anything is the joint being
    /// tested. A contact would be an alternative explanation for a body that
    /// stayed put.
    fn spawn_anchor(world: &mut PhysicsWorld, entity: Entity, pos: Vec3) {
        let body = RigidBodyBuilder::fixed()
            .translation(Vector::new(pos.x, pos.y, pos.z))
            .build();
        let handle = world.rigid_body_set.insert(body);
        world.register_entity_body(entity, handle);
    }

    /// A free unit-density ball of radius 0.5 at `pos`.
    ///
    /// The collider is what gives a dynamic body its mass — one without a
    /// collider has none, and Rapier will not move it however hard it is
    /// pushed.
    fn spawn_ball(world: &mut PhysicsWorld, entity: Entity, pos: Vec3) {
        let body = RigidBodyBuilder::dynamic()
            .translation(Vector::new(pos.x, pos.y, pos.z))
            .build();
        let body_handle = world.rigid_body_set.insert(body);
        let collider = ColliderBuilder::ball(0.5).density(1.0).build();
        let collider_handle = world.add_collider(collider, body_handle);
        world.collider_entity_map.insert(collider_handle, entity);
        world.register_entity_body(entity, body_handle);
    }

    fn position_of(world: &PhysicsWorld, entity: Entity) -> Vec3 {
        let handle = world.entity_body_map[&entity];
        let t = world.rigid_body_set[handle].translation();
        Vec3::new(t.x, t.y, t.z)
    }

    fn rotation_of(world: &PhysicsWorld, entity: Entity) -> glam::Quat {
        let handle = world.entity_body_map[&entity];
        let r = world.rigid_body_set[handle].rotation();
        glam::Quat::from_xyzw(r.x, r.y, r.z, r.w)
    }

    /// Where `entity`'s body-local point `local` currently is in the world —
    /// which is how a joint anchor is checked, since the anchors are what the
    /// constraint actually holds together.
    fn world_point(world: &PhysicsWorld, entity: Entity, local: Vec3) -> Vec3 {
        position_of(world, entity) + rotation_of(world, entity) * local
    }

    /// The scene the two fixed-joint tests share: `a` at the origin and `b`
    /// two metres along +X, both free, in zero gravity.
    ///
    /// [`WELD_A`]/[`WELD_B`] put the joint frame at (1, 0, 0) for both bodies,
    /// so the joint starts already satisfied — any change in separation is the
    /// test's own push, not the solver correcting a setup that was wrong to
    /// begin with.
    fn free_pair() -> (PhysicsWorld, Entity, Entity) {
        let mut world = PhysicsWorld::new(0.0);
        let a = Entity::from_raw(1);
        let b = Entity::from_raw(2);
        spawn_ball(&mut world, a, Vec3::ZERO);
        spawn_ball(&mut world, b, Vec3::new(2.0, 0.0, 0.0));
        (world, a, b)
    }

    const WELD_A: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    const WELD_B: Vec3 = Vec3::new(-1.0, 0.0, 0.0);
    /// Applied to `a` only, perpendicular to the line joining the two bodies,
    /// so an unconstrained `b` is left behind immediately.
    const PUSH: Vec3 = Vec3::new(0.0, 0.0, 3.0);

    #[test]
    fn a_fixed_joint_preserves_the_distance_between_two_bodies() {
        let (mut world, a, b) = free_pair();
        assert!(
            world.add_joint(a, b, &JointKind::Fixed, WELD_A, WELD_B),
            "add_joint must report success when both entities have bodies"
        );

        let before = position_of(&world, a).distance(position_of(&world, b));
        world.apply_impulse(a, PUSH);
        for _ in 0..60 {
            world.step(&());
        }
        let after = position_of(&world, a).distance(position_of(&world, b));

        assert!(
            position_of(&world, a).length() > 0.5,
            "sanity: the push has to have moved `a`. A preserved distance that \
             nothing tried to change proves nothing at all. a is at {:?}",
            position_of(&world, a)
        );
        assert!(
            (after - before).abs() < 0.01,
            "a fixed joint welds the two bodies, so pushing only `a` must carry `b` \
             along and leave the separation unchanged. The claim is not that a body \
             moved — it is that the relationship held. before={before}, after={after}"
        );
    }

    #[test]
    fn removing_the_joint_lets_the_bodies_separate() {
        let (mut world, a, b) = free_pair();
        assert!(world.add_joint(a, b, &JointKind::Fixed, WELD_A, WELD_B));
        assert!(
            world.remove_joint(a, b),
            "remove_joint must report success for a pair it really unlinked"
        );
        assert!(
            !world.remove_joint(a, b),
            "a second removal has nothing left to remove"
        );

        let before = position_of(&world, a).distance(position_of(&world, b));
        world.apply_impulse(a, PUSH);
        for _ in 0..60 {
            world.step(&());
        }
        let after = position_of(&world, a).distance(position_of(&world, b));

        assert!(
            after - before > 1.0,
            "with the joint removed, the same push must pull the bodies apart. This \
             is what makes the test above mean anything: without it, an implementation \
             where nothing ever moves — a joint that freezes the world, or a push that \
             never landed — passes that one too. before={before}, after={after}"
        );
    }

    #[test]
    fn a_revolute_joint_allows_rotation_about_its_axis_but_not_the_others() {
        // A hinge at the origin: `frame` is immovable, `door` hangs one metre
        // along +X from it, and the hinge axis is +Y.
        fn hinge_then_torque(torque: Vec3) -> glam::Quat {
            let mut world = PhysicsWorld::new(0.0);
            let frame = Entity::from_raw(1);
            let door = Entity::from_raw(2);
            spawn_anchor(&mut world, frame, Vec3::ZERO);
            spawn_ball(&mut world, door, Vec3::new(1.0, 0.0, 0.0));
            assert!(world.add_joint(
                frame,
                door,
                &JointKind::Revolute {
                    axis: Vec3::Y.into(),
                    limits: None,
                },
                Vec3::ZERO,
                Vec3::new(-1.0, 0.0, 0.0),
            ));
            world.apply_torque_impulse(door, torque);
            for _ in 0..60 {
                world.step(&());
            }
            rotation_of(&world, door)
        }

        let (axis, angle) = hinge_then_torque(Vec3::new(0.0, 0.5, 0.0)).to_axis_angle();
        assert!(
            angle > 0.1,
            "a hinge must turn when it is torqued about its own axis, got {angle} rad"
        );
        assert!(
            axis.dot(Vec3::Y).abs() > 0.99,
            "and it must turn about that axis and no other, got axis {axis:?}"
        );

        let (_, off_axis) = hinge_then_torque(Vec3::new(0.5, 0.0, 0.0)).to_axis_angle();
        assert!(
            off_axis < 0.02,
            "torque perpendicular to the hinge axis must not turn the door. Asserting \
             only that it rotates would pass on a joint constraining nothing at all, \
             and the door would come off its hinges. got {off_axis} rad"
        );
    }

    #[test]
    fn a_spherical_joint_holds_the_anchor_distance_while_rotation_stays_free() {
        // The ball hangs one metre below the pivot at (0, -1, 0), which is the
        // anchor body's local (0, -1, 0) and the ball's local (0, 1, 0) — the
        // two coincide at rest.
        fn swing(torque: Vec3) -> (f32, f32) {
            let mut world = PhysicsWorld::new(0.0);
            let pivot = Entity::from_raw(1);
            let ball = Entity::from_raw(2);
            spawn_anchor(&mut world, pivot, Vec3::ZERO);
            spawn_ball(&mut world, ball, Vec3::new(0.0, -2.0, 0.0));
            assert!(world.add_joint(
                pivot,
                ball,
                &JointKind::Spherical,
                Vec3::new(0.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ));
            world.apply_torque_impulse(ball, torque);
            for _ in 0..30 {
                world.step(&());
            }
            let (_, angle) = rotation_of(&world, ball).to_axis_angle();
            let gap = world_point(&world, pivot, Vec3::new(0.0, -1.0, 0.0)).distance(world_point(
                &world,
                ball,
                Vec3::new(0.0, 1.0, 0.0),
            ));
            (angle, gap)
        }

        // Every axis, because "free rotation" is the whole difference between
        // this and the hinge above: a revolute joint would pass on one axis
        // and fail on the other two.
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            let (angle, gap) = swing(axis * 0.2);
            assert!(
                angle > 0.05,
                "a ball joint turns freely about every axis, {axis:?} included; \
                 got only {angle} rad"
            );
            assert!(
                gap < 0.02,
                "...and however it turns, the two anchor points must stay coincident — \
                 that separation is the entire constraint. About {axis:?} the gap \
                 opened to {gap}"
            );
        }
    }

    #[test]
    fn adding_a_joint_for_an_unknown_entity_returns_false() {
        // `set_collider_shape`'s convention: a stale entity is a value-level
        // failure, never a panic. A joint meets it twice over, since either of
        // the two entities can be the stale one.
        let mut world = PhysicsWorld::new(0.0);
        let real = Entity::from_raw(1);
        let ghost = Entity::from_raw(999);
        spawn_ball(&mut world, real, Vec3::ZERO);

        assert!(!world.add_joint(real, ghost, &JointKind::Fixed, Vec3::ZERO, Vec3::ZERO));
        assert!(!world.add_joint(ghost, real, &JointKind::Fixed, Vec3::ZERO, Vec3::ZERO));
        assert!(!world.remove_joint(real, ghost));
    }
}
