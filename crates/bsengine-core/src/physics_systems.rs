//! The set the physics step lives in, so other crates can order against it.

use bevy_ecs::schedule::SystemSet;

/// The physics frame in `Update`: the kinematic `Transform -> PhysicsInput`
/// copy, body creation, joints, vehicles, character controllers, the Rapier
/// step and the sync back into `Transform`, chained in that order by
/// `bsengine_physics::PhysicsPlugin`.
///
/// Defined here rather than in the physics crate so the scripting crate can
/// order itself after it without depending on physics. Before this set
/// existed nothing ordered scripts against the step at all: the scheduler
/// happened to sort physics first, every recording was made that way, and
/// `--shuffle-seed 4` (and 7, 8, 13, 16) put scripts first and failed
/// tilt-run's level 5, whose recording launches the ball on a timed gap in
/// a script-driven obstacle: a script's writes landing in the step on the
/// other side of the frame is a one-frame phase shift, and that recording
/// cannot absorb one.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PhysicsSystems;
