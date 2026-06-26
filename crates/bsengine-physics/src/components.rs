use bevy_ecs::prelude::Component;
use glam::{Quat, Vec3};
use rapier3d::prelude::{ColliderHandle, RigidBodyHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigidBodyType {
    Dynamic,
    Static,
    KinematicPosition,
}

#[derive(Component, Debug, Clone)]
pub struct RigidBody {
    pub body_type: RigidBodyType,
    pub linear_damping: f32,
    pub angular_damping: f32,
}

impl RigidBody {
    pub fn dynamic() -> Self {
        Self { body_type: RigidBodyType::Dynamic, linear_damping: 0.0, angular_damping: 0.0 }
    }

    pub fn fixed() -> Self {
        Self { body_type: RigidBodyType::Static, linear_damping: 0.0, angular_damping: 0.0 }
    }

    pub fn kinematic() -> Self {
        Self { body_type: RigidBodyType::KinematicPosition, linear_damping: 0.0, angular_damping: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub enum ColliderShape {
    Box { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
}

#[derive(Component, Debug, Clone)]
pub struct Collider {
    pub shape: ColliderShape,
    pub restitution: f32,
    pub friction: f32,
    pub density: f32,
}

impl Collider {
    pub fn cuboid(hx: f32, hy: f32, hz: f32) -> Self {
        Self {
            shape: ColliderShape::Box { half_extents: Vec3::new(hx, hy, hz) },
            restitution: 0.0,
            friction: 0.5,
            density: 1.0,
        }
    }

    pub fn ball(radius: f32) -> Self {
        Self {
            shape: ColliderShape::Sphere { radius },
            restitution: 0.0,
            friction: 0.5,
            density: 1.0,
        }
    }

    pub fn capsule(half_height: f32, radius: f32) -> Self {
        Self {
            shape: ColliderShape::Capsule { half_height, radius },
            restitution: 0.0,
            friction: 0.5,
            density: 1.0,
        }
    }

    pub fn with_restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution;
        self
    }

    pub fn with_friction(mut self, friction: f32) -> Self {
        self.friction = friction;
        self
    }
}

/// Written by the physics system after each step — read this to get simulated position/rotation.
#[derive(Component, Debug, Clone, Default)]
pub struct PhysicsTransform {
    pub translation: Vec3,
    pub rotation: Quat,
}

/// Input transform for the physics system — set this to teleport or drive kinematic bodies.
#[derive(Component, Debug, Clone)]
pub struct PhysicsInput {
    pub translation: Vec3,
    pub rotation: Quat,
}

impl Default for PhysicsInput {
    fn default() -> Self {
        Self { translation: Vec3::ZERO, rotation: Quat::IDENTITY }
    }
}

/// Internal: Rapier handles stored per entity after body creation.
#[derive(Component)]
pub(crate) struct PhysicsHandles {
    pub body_handle: RigidBodyHandle,
    pub collider_handle: ColliderHandle,
}
