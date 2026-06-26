use bevy_ecs::prelude::Resource;
use rapier3d::prelude::*;

#[derive(Resource)]
pub struct PhysicsWorld {
    pub(crate) rigid_body_set: RigidBodySet,
    pub(crate) collider_set: ColliderSet,
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

    pub fn step(&mut self) {
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
            &(),
        );
    }

    pub(crate) fn add_collider(
        &mut self,
        coll: rapier3d::geometry::Collider,
        body_handle: RigidBodyHandle,
    ) -> ColliderHandle {
        self.collider_set.insert_with_parent(coll, body_handle, &mut self.rigid_body_set)
    }

    pub fn gravity(&self) -> f32 {
        -self.gravity.y
    }

    pub fn set_gravity(&mut self, magnitude: f32) {
        self.gravity = Vector::new(0.0, -magnitude, 0.0);
    }
}
