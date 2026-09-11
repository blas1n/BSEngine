//! Positional audio: a listener, emitters, and the conversion between this
//! engine's maths and `kira`'s.
//!
//! `kira` does the acoustics. A sound played on a spatial sub-track that is
//! linked to a listener gets distance attenuation and left/right panning
//! without this crate computing either. What lives here is the wiring: which
//! entity is the ears, which entities are sources, and keeping both in step
//! with their `Transform`s.
//!
//! ## Why `mint` and not `glam`
//!
//! `kira`'s spatial API takes `mint::Vector3<f32>` and `mint::Quaternion<f32>`.
//! It depends on `glam` 0.33 for convenience conversions while this workspace
//! is on 0.29 — six `glam` versions are in `Cargo.lock` — so a `glam::Vec3`
//! from here is a *different type* to one from there and cannot be passed
//! straight in. `mint` exists precisely for this, and converting component-wise
//! sidesteps the version skew entirely.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::{Quat, Vec3};

/// Marks the entity whose `Transform` is the listener — the ears the scene is
/// heard from. Normally the camera.
///
/// Exactly one entity should carry this. If several do, the first one found
/// wins and the rest are ignored with a warning, because "which of these is the
/// listener" has no good answer and silently picking one hides the mistake.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Default)]
pub struct AudioListener;

/// Marks an entity as a source of positional sound.
///
/// The entity's `Transform` is where its sounds come from. Sounds started with
/// `Bsengine.playSound3D` play on this emitter's spatial track; sounds started
/// with the plain `playSound` are not positional and ignore emitters entirely.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct AudioEmitter {
    /// How strongly this emitter is panned by direction, from 0 (heard equally
    /// in both ears, however it is placed) to 1 (fully directional).
    ///
    /// Distance attenuation is unaffected — a sound with no spatialization
    /// still gets quieter with distance.
    pub spatialization_strength: f32,
}

impl Default for AudioEmitter {
    fn default() -> Self {
        Self {
            spatialization_strength: 1.0,
        }
    }
}

/// Converts a position into the type `kira`'s spatial API accepts.
pub fn to_mint_vec(v: Vec3) -> mint::Vector3<f32> {
    mint::Vector3 {
        x: v.x,
        y: v.y,
        z: v.z,
    }
}

/// Converts an orientation into the type `kira`'s listener API accepts.
pub fn to_mint_quat(q: Quat) -> mint::Quaternion<f32> {
    mint::Quaternion {
        v: mint::Vector3 {
            x: q.x,
            y: q.y,
            z: q.z,
        },
        s: q.w,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_survives_the_trip_to_mint() {
        let v = to_mint_vec(Vec3::new(1.0, -2.0, 3.5));
        assert_eq!((v.x, v.y, v.z), (1.0, -2.0, 3.5));
    }

    #[test]
    fn a_quaternion_keeps_its_scalar_in_the_scalar_slot() {
        // mint splits a quaternion into a vector part and a scalar part, and
        // getting w into `s` rather than into `v` is the whole of the
        // conversion. Swapping them compiles and turns every orientation into a
        // different one, silently.
        let q = Quat::from_xyzw(0.1, 0.2, 0.3, 0.9);
        let m = to_mint_quat(q);
        assert_eq!(m.s, 0.9, "w belongs in the scalar part");
        assert_eq!((m.v.x, m.v.y, m.v.z), (0.1, 0.2, 0.3));
    }

    #[test]
    fn an_emitter_is_fully_directional_by_default() {
        assert_eq!(AudioEmitter::default().spatialization_strength, 1.0);
    }
}

/// Makes an emitter's sound muffle and quieten when something solid stands
/// between it and the listener.
///
/// **A separate component rather than fields on [`AudioEmitter`], deliberately.**
/// Scene components are deserialised by `bevy_reflect`, which requires every
/// reflected field to be present in the RON; a missing one does not error, it
/// silently empties the containing collection. `games/mini-arena` authors
/// `AudioEmitter(spatialization_strength: 1.0)`, so adding fields there would
/// have broken it quietly. Nothing authors this type yet, so it cannot break
/// anything — and "occluded only if the component is present" is also how
/// Unreal ships occlusion: off unless asked for.
///
/// The defaults mirror Unreal's occlusion settings, which is where the names
/// come from too.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct AudioOcclusion {
    /// Low-pass cutoff in hertz applied when fully occluded. Lower is more
    /// muffled. Unreal calls this `OcclusionLowPassFilterFrequency`.
    pub cutoff_hz: f32,
    /// Volume in decibels applied when fully occluded, relative to unoccluded.
    pub volume_db: f32,
    /// How long the change takes, in milliseconds.
    ///
    /// This is what stops a sound flickering when the ray grazes an edge, and
    /// it is how both reference engines solve that — Unreal calls it
    /// `OcclusionInterpolationTime`. Smoothing rather than hysteresis: a
    /// half-occluded frame moves the value part-way instead of toggling.
    pub interpolation_ms: f32,
}

impl Default for AudioOcclusion {
    fn default() -> Self {
        Self {
            cutoff_hz: 800.0,
            volume_db: -6.0,
            interpolation_ms: 200.0,
        }
    }
}
