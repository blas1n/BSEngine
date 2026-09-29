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

/// Captures the scene around it into a cubemap, once, and makes surfaces
/// inside its box reflect that capture instead of the sky.
///
/// Unity's Reflection Probe, Unreal's Box Reflection Capture and Godot's
/// `ReflectionProbe` share this shape: a position the capture is taken from,
/// a box of influence, optional box projection, an intensity. Like Godot's
/// default *Update Once* and Unity's baked probes, the capture is taken when
/// the probe appears or its own settings change -- not when the scene around
/// it does, so moving a lamp after load keeps the old reflection.
///
/// Where boxes overlap, the smallest box containing the surface wins.
/// Unity and Godot can also blend across a margin; this does not, so the
/// switch between two probes is a hard edge.
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
}

impl Default for ReflectionProbe {
    fn default() -> Self {
        Self {
            half_extents: glam::Vec3::splat(5.0).into(),
            box_projection: false,
            intensity: 1.0,
        }
    }
}
