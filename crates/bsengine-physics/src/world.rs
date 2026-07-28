use std::collections::HashMap;

use bevy_ecs::prelude::{Entity, Resource};
use glam::Vec3;
use rapier3d::pipeline::EventHandler;
use rapier3d::prelude::*;

use crate::components::RaycastHit;

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
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
        }
    }

    /// Advances the simulation by one timestep, reporting contact events via `event_handler`.
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

    /// Cast a ray into the physics world. Returns hit info or None.
    pub fn cast_ray(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RaycastHit> {
        // QueryPipeline<'a> borrows the sets so it is constructed per-call from the broad phase.
        let qp = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.rigid_body_set,
            &self.collider_set,
            QueryFilter::default(),
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
