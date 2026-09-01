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
    /// A grid-sampled height surface, for terrain. `heights` is row-major,
    /// `rows * cols` long. `scale` is the world-space size the height grid
    /// spans: `scale.x`/`scale.z` are the horizontal extent, `scale.y` is the
    /// multiplier applied to raw height values (already-scaled world-space
    /// heights go into `heights` directly, so this is usually `1.0`).
    Heightfield {
        /// Row-major height values.
        heights: Vec<f32>,
        /// Number of rows in the height grid.
        rows: usize,
        /// Number of columns in the height grid.
        cols: usize,
        /// World-space extent (x, height-multiplier, z). See field doc above.
        scale: ReflectVec3,
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

/// Constrains this entity's rigid body to another's.
///
/// A joint links two bodies, but an ECS component lives on one entity, so it
/// names the other — the same shape as `TerrainChunkOf(Entity)`.
///
/// The anchors are the point the constraint actually holds: each is given in
/// its own body's local space, and the two are what a joint keeps together.
/// Placing them so they already coincide at spawn means the joint starts
/// satisfied instead of the solver yanking the bodies into place on frame one.
///
/// Public and reflected (catalogue rule R1) because scenes author it and
/// scripting creates it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct Joint {
    /// The other body this entity is constrained to.
    pub body_b: Entity,
    /// Which constraint, with its per-kind parameters.
    pub kind: JointKind,
    /// Attachment point in this entity's local space.
    pub anchor_a: ReflectVec3,
    /// Attachment point in `body_b`'s local space.
    pub anchor_b: ReflectVec3,
}

/// The constraint a [`Joint`] applies.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum JointKind {
    /// Welds the two bodies rigidly: no relative motion at all.
    Fixed,
    /// A hinge. Rotation is allowed only about `axis`, optionally clamped to
    /// `limits` in radians.
    Revolute {
        /// Hinge axis in the first body's local space.
        axis: ReflectVec3,
        /// Optional `[min, max]` angle limits, in radians.
        limits: Option<[f32; 2]>,
    },
    /// A ball joint: free rotation about the shared anchor point.
    Spherical,
}

/// Turns a skeletal mesh into joint-linked rigid bodies so it collapses
/// physically.
///
/// Requires a `SkinnedMesh` on the same entity: the bone hierarchy comes from
/// its `nodes`. Activating this on an entity without one warns and does
/// nothing, rather than panicking — a ragdoll on a non-skeletal entity is an
/// authoring mistake, not a reason to take the game down.
///
/// [`active`] starts **false**. A component that collapsed the character the
/// moment it was attached would make "give this character a ragdoll" and "kill
/// this character" the same action; switching it on is a separate, deliberate
/// step.
///
/// [`active`]: Ragdoll::active
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct Ragdoll {
    /// While true, physics drives the bones instead of animation.
    pub active: bool,
    /// Per-bone joint overrides, keyed by bone name. Bones absent here get
    /// [`JointKind::Spherical`].
    ///
    /// The name is the **skeleton node the joint sits on**, which is the one an
    /// author is thinking of. A bone spans its parent node to itself, so the
    /// constraint that holds it is at its parent — on a Mixamo-style rig, the
    /// shin bone runs `LeftLeg`(knee) → `LeftFoot`(ankle) and the hinge that
    /// stops the knee bending backwards is at `LeftLeg`. So
    /// `{"LeftLeg": Revolute { .. }}` is a knee hinge, which is what it reads
    /// like.
    ///
    /// Spherical is the default because nothing in a skeleton says which bone
    /// is a knee, so a generic answer is required, and spherical is the
    /// physically stable one that needs no configuration at all. This map is
    /// how an author says otherwise — marking a knee or an elbow `Revolute`
    /// so it stops bending backwards. Guessing from the bone's *name* was
    /// rejected: it depends on a rigging naming convention and fails silently
    /// on a different rig or on non-English bone names.
    ///
    /// A `HashMap` rather than a `Vec` of pairs because `bevy_reflect` handles
    /// it: a `Map`-kind field is structurally recursed by
    /// `TypedReflectDeserializer`, so a scene can author this directly. (The
    /// type that does *not* survive that is `HashSet`, which needs an explicit
    /// `ReflectDeserialize` registration — see `AnimationStateMachine`'s
    /// `triggers`.)
    pub joint_overrides: std::collections::HashMap<String, JointKind>,
    /// Radius of each bone's capsule collider, in world units.
    pub bone_radius: f32,
    /// Total mass, distributed across bones in proportion to bone length.
    pub total_mass: f32,
}

impl Default for Ragdoll {
    fn default() -> Self {
        Self {
            active: false,
            joint_overrides: std::collections::HashMap::new(),
            bone_radius: 0.08,
            total_mass: 70.0,
        }
    }
}

impl Ragdoll {
    /// The joint kind for the bone whose joint sits on the node named `bone`,
    /// falling back to [`JointKind::Spherical`] when nothing overrides it.
    ///
    /// See [`joint_overrides`] for which of a bone's two ends names it.
    ///
    /// [`joint_overrides`]: Ragdoll::joint_overrides
    pub fn joint_for_bone(&self, bone: &str) -> JointKind {
        self.joint_overrides
            .get(bone)
            .copied()
            .unwrap_or(JointKind::Spherical)
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

/// Internal: marks one of the bone bodies a [`Ragdoll`] spawned, and says which
/// bone of whose skeleton it is.
///
/// Deliberately not public, and so not a catalogue entry: these entities are
/// the simulation's own bookkeeping, created and destroyed by the ragdoll pass
/// rather than authored, and an engine-wide component catalogue should no more
/// list them than it lists [`PhysicsHandles`]. The same reasoning `sync_joints`
/// gives for keeping its `created` map a `Local`.
///
/// The two fields are what lets the pose the bodies imply be published back to
/// the skeleton by a *different* system from the one that built them — the pass
/// that reads where the bodies ended up has to run after Rapier has been
/// stepped, and a `Local` cannot be shared between systems.
#[derive(Component)]
pub(crate) struct RagdollBone {
    /// The entity whose [`Ragdoll`] built this bone.
    pub owner: Entity,
    /// Index into that entity's `SkinnedMesh.nodes` — the bone's child end.
    pub node: usize,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ragdoll_defaults_to_spherical_for_every_bone() {
        // With an empty override map, every bone must resolve to Spherical.
        // Spherical is the physically stable default that works with no
        // configuration at all; the overrides exist so knees and elbows can be
        // marked Revolute and stop bending backwards. Nothing in a skeleton
        // says which bone is a knee, so there has to be a generic answer, and
        // a name-based guess was rejected: it depends on a rigging convention
        // and fails silently on a rig that names bones differently.
        let r = Ragdoll::default();
        assert!(r.joint_overrides.is_empty(), "the default has no overrides");
        for bone in ["Thigh", "Spine", "LeftForeArm", "허벅지", ""] {
            assert!(
                matches!(r.joint_for_bone(bone), JointKind::Spherical),
                "bone {bone:?} should fall back to Spherical, got {:?}",
                r.joint_for_bone(bone)
            );
        }
    }

    #[test]
    fn an_override_replaces_the_default_for_that_bone_only() {
        // Without this, the override map could be ignored entirely -- read and
        // then dropped on the floor -- and the test above would still pass.
        let mut r = Ragdoll::default();
        r.joint_overrides.insert(
            "LeftLeg".to_string(),
            JointKind::Revolute {
                axis: Vec3::X.into(),
                limits: Some([0.0, 2.2]),
            },
        );

        match r.joint_for_bone("LeftLeg") {
            JointKind::Revolute { axis, limits } => {
                assert_eq!(axis.0, Vec3::X, "the override's own axis must come back");
                assert_eq!(limits, Some([0.0, 2.2]), "and its own limits");
            }
            other => panic!("the overridden bone should be Revolute, got {other:?}"),
        }

        // "for that bone only": an override is not a global switch.
        assert!(
            matches!(r.joint_for_bone("RightLeg"), JointKind::Spherical),
            "a bone with no entry keeps the Spherical default even when a \
             sibling was overridden"
        );
    }

    #[test]
    fn a_ragdoll_is_inactive_until_something_switches_it_on() {
        // Attaching the component to a working character must change nothing.
        // If this defaulted to true, adding a Ragdoll in the Inspector -- or a
        // scene gaining the component -- would collapse the character on the
        // spot.
        assert!(
            !Ragdoll::default().active,
            "Ragdoll::default() must be inert"
        );
    }
}
