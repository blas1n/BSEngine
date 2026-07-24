use bevy_ecs::prelude::{Component, Entity, Event};
use glam::{Quat, Vec3};
use rapier3d::prelude::{ColliderHandle, RigidBodyHandle};

/// Fired when two colliders start or stop touching; `started` distinguishes the two cases.
#[derive(Event, Debug, Clone, Copy)]
pub struct CollisionEvent {
    /// The first entity in the contact pair.
    pub entity_a: Entity,
    /// The second entity in the contact pair.
    pub entity_b: Entity,
    /// `true` when contact began this step, `false` when it ended.
    pub started: bool,
}

/// How a `RigidBody` is simulated: affected by forces, fixed in place, or driven by code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigidBodyType {
    /// Moved by forces, impulses, and gravity.
    Dynamic,
    /// Immovable; never affected by forces or collisions.
    Static,
    /// Moved only by explicitly setting its position (e.g. via `PhysicsInput`); ignores forces.
    KinematicPosition,
}

/// ECS component marking an entity as a physics body; paired with a `Collider` for shape/material.
#[derive(Component, Debug, Clone)]
pub struct RigidBody {
    /// Whether the body is dynamic, static, or kinematic.
    pub body_type: RigidBodyType,
    /// Damping applied to linear velocity each step, slowing translation over time.
    pub linear_damping: f32,
    /// Damping applied to angular velocity each step, slowing rotation over time.
    pub angular_damping: f32,
}

impl RigidBody {
    /// Creates a dynamic body with no damping, free to move under forces and gravity.
    pub fn dynamic() -> Self {
        Self {
            body_type: RigidBodyType::Dynamic,
            linear_damping: 0.0,
            angular_damping: 0.0,
        }
    }

    /// Creates a static body that never moves, e.g. for ground or walls.
    pub fn fixed() -> Self {
        Self {
            body_type: RigidBodyType::Static,
            linear_damping: 0.0,
            angular_damping: 0.0,
        }
    }

    /// Creates a kinematic body driven by explicit position updates rather than physics forces.
    pub fn kinematic() -> Self {
        Self {
            body_type: RigidBodyType::KinematicPosition,
            linear_damping: 0.0,
            angular_damping: 0.0,
        }
    }
}

/// The geometric shape a `Collider` uses for contact and raycast queries.
#[derive(Debug, Clone)]
pub enum ColliderShape {
    /// An axis-aligned box, defined by its half-extents along each axis.
    Box {
        /// Half the box's size along each axis (x, y, z).
        half_extents: Vec3,
    },
    /// A sphere, defined by its radius.
    Sphere {
        /// The sphere's radius.
        radius: f32,
    },
    /// A capsule (cylinder with rounded caps) aligned along the Y axis.
    Capsule {
        /// Half the height of the capsule's cylindrical body, excluding the rounded caps.
        half_height: f32,
        /// The radius of the capsule's rounded caps and cylindrical body.
        radius: f32,
    },
}

/// ECS component describing the physical shape and surface material of a `RigidBody`.
#[derive(Component, Debug, Clone)]
pub struct Collider {
    /// The collider's geometric shape.
    pub shape: ColliderShape,
    /// Bounciness of collisions; 0 absorbs all energy, 1 is a perfectly elastic bounce.
    pub restitution: f32,
    /// Surface friction coefficient used when resolving contacts.
    pub friction: f32,
    /// Mass per unit volume, used with the shape's volume to compute the body's mass.
    pub density: f32,
    /// When `true`, the collider detects overlaps but generates no physical response.
    pub sensor: bool,
}

impl Collider {
    /// Creates a box collider with the given half-extents and default material properties.
    pub fn cuboid(hx: f32, hy: f32, hz: f32) -> Self {
        Self {
            shape: ColliderShape::Box {
                half_extents: Vec3::new(hx, hy, hz),
            },
            restitution: 0.0,
            friction: 0.5,
            density: 1.0,
            sensor: false,
        }
    }

    /// Creates a sphere collider with the given radius and default material properties.
    pub fn ball(radius: f32) -> Self {
        Self {
            shape: ColliderShape::Sphere { radius },
            restitution: 0.0,
            friction: 0.5,
            density: 1.0,
            sensor: false,
        }
    }

    /// Creates a capsule collider with the given half-height and radius and default material properties.
    pub fn capsule(half_height: f32, radius: f32) -> Self {
        Self {
            shape: ColliderShape::Capsule {
                half_height,
                radius,
            },
            restitution: 0.0,
            friction: 0.5,
            density: 1.0,
            sensor: false,
        }
    }

    /// Sets the restitution (bounciness) and returns `self` for chaining.
    pub fn with_restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution;
        self
    }

    /// Sets the friction coefficient and returns `self` for chaining.
    pub fn with_friction(mut self, friction: f32) -> Self {
        self.friction = friction;
        self
    }

    /// Sets whether this collider is a sensor (no physical response) and returns `self` for chaining.
    pub fn with_sensor(mut self, sensor: bool) -> Self {
        self.sensor = sensor;
        self
    }
}

/// Result of a raycast query.
#[derive(Debug, Clone)]
pub struct RaycastHit {
    /// The entity whose collider was hit, if the hit collider maps to a known entity.
    pub entity: Option<Entity>,
    /// The world-space point where the ray hit the collider.
    pub point: Vec3,
    /// The surface normal at the hit point.
    pub normal: Vec3,
    /// The distance from the ray origin to the hit point.
    pub distance: f32,
}

/// Written by the physics system after each step — read this to get simulated position/rotation.
#[derive(Component, Debug, Clone, Default)]
pub struct PhysicsTransform {
    /// The body's simulated world-space position.
    pub translation: Vec3,
    /// The body's simulated world-space rotation.
    pub rotation: Quat,
}

/// Input transform for the physics system — set this to teleport or drive kinematic bodies.
#[derive(Component, Debug, Clone)]
pub struct PhysicsInput {
    /// The position to spawn at, or to drive a kinematic body toward.
    pub translation: Vec3,
    /// The rotation to spawn at, or to drive a kinematic body toward.
    pub rotation: Quat,
}

impl Default for PhysicsInput {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

/// Internal: Rapier handles stored per entity after body creation.
#[derive(Component)]
pub(crate) struct PhysicsHandles {
    pub body_handle: RigidBodyHandle,
    // Stored for a future despawn-time collider cleanup pass; not yet read.
    #[allow(dead_code)]
    pub collider_handle: ColliderHandle,
}
