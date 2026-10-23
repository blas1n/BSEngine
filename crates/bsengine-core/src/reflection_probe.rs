//! Reflection probes: a captured cubemap that gives surfaces inside a box
//! reflections of the room they are in, instead of the sky.

use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Largest number of reflection probes rendered at once, matching the GPU
/// cube array's slot count. Probes past it are ignored -- the scene still
/// renders, with the sky reflection where the ignored probes would have been.
pub const MAX_REFLECTION_PROBES: usize = 4;

/// Captures the scene around it into a cubemap and makes surfaces inside its
/// box reflect that capture instead of the sky -- once, or kept up to date
/// ([`ReflectionProbe::update`]).
///
/// Unity's Reflection Probe, Unreal's Box Reflection Capture and Godot's
/// `ReflectionProbe` share this shape: a position the capture is taken from,
/// a box of influence, optional box projection, an intensity, and a refresh
/// mode. With the default [`crate::ProbeUpdate::Once`] -- Godot's default
/// *Update Once*, Unity's baked probes -- the capture is taken when the probe
/// appears or its own settings change, not when the scene around it does, so
/// moving a lamp after load keeps the old reflection; `Realtime` re-captures
/// it a face a frame.
///
/// Where boxes overlap they blend, the smaller over the larger: inside its
/// box, within [`ReflectionProbe::blend_distance`] of a face, a probe's
/// share fades from all of the reflection to none, and what it leaves is the
/// next larger box's, then the sky's. The shape Unreal's captures have
/// (sorted by size, each blended over the ones behind it by a transition
/// distance) and Unity HDRP's and Godot's blend distances, all measured
/// inward from the box's faces.
///
/// Absent means no probe: surfaces reflect the skybox (or nothing without
/// one), exactly as before.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct ReflectionProbe {
    /// Half-extents of the box of influence, in world units, around the
    /// entity's position. Axis-aligned: the entity's rotation and scale do
    /// not turn or stretch it.
    pub half_extents: ReflectVec3,
    /// Treat the capture as the inside of the box rather than as infinitely
    /// far away, so a reflection lines up with the walls it shows. Right for
    /// a room the box fits; wrong for an open area, where it bends distant
    /// things onto the box. Off by default, as in Unity and Godot.
    pub box_projection: bool,
    /// Multiplies the reflection's brightness. 1 is physically what the
    /// probe saw.
    pub intensity: f32,
    /// Captured once (the default) or kept up to date, a face a frame. See
    /// [`crate::ProbeUpdate`].
    pub update: crate::ProbeUpdate,
    /// How far in from its box's faces the probe fades out, in world units:
    /// at a face its share is none, this far inside it is all. 0 is a hard
    /// edge. 1, as Godot's and Unity HDRP's default blend distance and
    /// Unreal's default transition distance (100 cm) are.
    pub blend_distance: f32,
}

impl Default for ReflectionProbe {
    fn default() -> Self {
        Self {
            half_extents: glam::Vec3::splat(5.0).into(),
            box_projection: false,
            intensity: 1.0,
            update: crate::ProbeUpdate::Once,
            blend_distance: 1.0,
        }
    }
}
