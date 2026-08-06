use bevy_ecs::prelude::{Component, Entity, Event, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use bsengine_core::{ReflectQuat, ReflectVec3};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum RigidBodyType {
    /// Moved by forces, impulses, and gravity.
    Dynamic,
    /// Immovable; never affected by forces or collisions.
    Static,
    /// Moved only by explicitly setting its position (e.g. via `PhysicsInput`); ignores forces.
    KinematicPosition,
}

/// ECS component marking an entity as a physics body; paired with a `Collider` for shape/material.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
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
#[derive(Debug, Clone, Reflect)]
pub enum ColliderShape {
    /// An axis-aligned box, defined by its half-extents along each axis.
    Box {
        /// Half the box's size along each axis (x, y, z).
        ///
        /// A [`ReflectVec3`] rather than a bare `glam::Vec3` for the reason
        /// `bsengine_core::reflect_glam` documents: `glam`'s types cannot
        /// implement `bevy_reflect::Reflect` here (orphan rule), so a field
        /// that has to be reflected wears the wrapper. `Deref` means readers
        /// still write `half_extents.x`.
        half_extents: ReflectVec3,
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
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
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
                half_extents: Vec3::new(hx, hy, hz).into(),
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

/// Where the simulation says a body **is**. Physics writes it; you read it.
///
/// Structurally identical to [`PhysicsInput`], and the difference is entirely
/// direction of travel — which is invisible in an Inspector showing two
/// components with the same two fields. If you are about to write to one of
/// them, it is [`PhysicsInput`]; if you are about to read where something
/// ended up, it is this one. Writing here is silently pointless: the next step
/// overwrites it.
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component, Default)]
pub struct PhysicsTransform {
    /// The body's simulated world-space position.
    pub position: ReflectVec3,
    /// The body's simulated world-space rotation.
    pub rotation: ReflectQuat,
}

/// Where a body should **go**. You write it; physics reads it.
///
/// Structurally identical to [`PhysicsTransform`] — see that type for how to
/// tell them apart. Used to place a body when it spawns, to teleport one, and
/// to drive a kinematic body frame by frame; for a dynamic body after spawn,
/// prefer impulses, since a teleport skips collision resolution entirely.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct PhysicsInput {
    /// The position to spawn at, or to drive a kinematic body toward.
    pub position: ReflectVec3,
    /// The rotation to spawn at, or to drive a kinematic body toward.
    pub rotation: ReflectQuat,
}

impl Default for PhysicsInput {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO.into(),
            rotation: Quat::IDENTITY.into(),
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

/// Marks a [`RigidBody`] as a walking character, and carries what the physics
/// engine cannot infer on its own.
///
/// A character is an ordinary `Dynamic` body — it takes impulses like anything
/// else, which is what makes knockback real rather than a script pushing a
/// transform around. Two things separate it from a crate or a ball:
///
/// * **It stays upright.** Adding this component locks pitch and roll, leaving
///   yaw free, so a capsule shoved sideways slides instead of toppling. Rapier
///   supports this per-body and the scene format has no way to say it, which is
///   why the component is where it gets said.
/// * **It knows whether it is standing on something.** [`grounded`] is written
///   after each physics step from a downward ray, and is what gameplay asks
///   before jumping, playing a landing sound, or applying air control.
///
/// [`grounded`]: CharacterBody::grounded
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct CharacterBody {
    /// How far below the character's feet to look for ground, in world units.
    ///
    /// Small enough not to claim ground while genuinely airborne, large enough
    /// to survive the gap physics leaves between a resting body and the surface
    /// it rests on.
    pub ground_check_distance: f32,
    /// The steepest surface, in degrees from horizontal, that still counts as
    /// ground. A steeper hit leaves [`grounded`] false, so a character pressed
    /// against a wall is not standing on it.
    ///
    /// [`grounded`]: CharacterBody::grounded
    pub max_slope_deg: f32,
    /// Whether the character is standing on something.
    ///
    /// Written by the physics system after every step. Authoring it in a scene
    /// file has no effect beyond the first frame.
    pub grounded: bool,
}

impl Default for CharacterBody {
    fn default() -> Self {
        Self {
            ground_check_distance: 0.2,
            max_slope_deg: 50.0,
            grounded: false,
        }
    }
}
