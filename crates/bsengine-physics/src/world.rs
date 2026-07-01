use std::collections::HashMap;

use bevy_ecs::prelude::{Entity, Resource};
use glam::Vec3;
use rapier3d::pipeline::EventHandler;
use rapier3d::prelude::*;

use crate::components::RaycastHit;

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

    pub fn gravity(&self) -> f32 {
        -self.gravity.y
    }

    pub fn set_gravity(&mut self, magnitude: f32) {
        self.gravity = Vector::new(0.0, -magnitude, 0.0);
    }

    pub(crate) fn register_entity_body(&mut self, entity: Entity, handle: RigidBodyHandle) {
        self.entity_body_map.insert(entity, handle);
    }

    pub fn get_linvel(&self, entity: Entity) -> Option<Vec3> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        let v = body.linvel();
        Some(Vec3::new(v.x, v.y, v.z))
    }

    pub fn set_linvel(&mut self, entity: Entity, vel: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_linvel(Vector::new(vel.x, vel.y, vel.z), true);
            }
        }
    }

    pub fn apply_impulse(&mut self, entity: Entity, impulse: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.apply_impulse(Vector::new(impulse.x, impulse.y, impulse.z), true);
            }
        }
    }

    pub fn apply_force(&mut self, entity: Entity, force: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.add_force(Vector::new(force.x, force.y, force.z), true);
            }
        }
    }

    pub fn get_angvel(&self, entity: Entity) -> Option<Vec3> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        let v = body.angvel();
        Some(Vec3::new(v.x, v.y, v.z))
    }

    pub fn set_angvel(&mut self, entity: Entity, vel: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_angvel(Vector::new(vel.x, vel.y, vel.z), true);
            }
        }
    }

    pub fn apply_torque_impulse(&mut self, entity: Entity, impulse: Vec3) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.apply_torque_impulse(Vector::new(impulse.x, impulse.y, impulse.z), true);
            }
        }
    }

    pub fn set_linear_damping(&mut self, entity: Entity, damping: f32) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_linear_damping(damping);
            }
        }
    }

    pub fn set_angular_damping(&mut self, entity: Entity, damping: f32) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_angular_damping(damping);
            }
        }
    }

    pub fn get_mass(&self, entity: Entity) -> Option<f32> {
        let handle = self.entity_body_map.get(&entity)?;
        let body = self.rigid_body_set.get(*handle)?;
        Some(body.mass())
    }

    pub fn set_mass(&mut self, entity: Entity, mass: f32) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_additional_mass(mass, true);
            }
        }
    }

    pub fn lock_rotations(&mut self, entity: Entity, lock_x: bool, lock_y: bool, lock_z: bool) {
        if let Some(&handle) = self.entity_body_map.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_enabled_rotations(!lock_x, !lock_y, !lock_z, true);
            }
        }
    }

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
