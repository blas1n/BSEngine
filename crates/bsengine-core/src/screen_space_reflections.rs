//! Screen-space reflections applied by a camera.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Reflections traced through the depth buffer, applied by a camera.
///
/// Unity (HDRP), Unreal and Godot all offer this as a camera or volume
/// setting rather than a material one, and for the same reason: what it costs
/// and what it can reach are properties of the *view*, not of any one surface.
///
/// Absent means no reflections, which is what every scene authored before this
/// existed says. It is off rather than on by default because switching it on
/// changes the shading of every reflective surface in a scene — that is a
/// decision for whoever authored the scene, not a silent upgrade.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct ScreenSpaceReflections {
    /// Whether the effect is applied at all.
    pub enabled: bool,
    /// How strongly the reflection is added, `0.0..=1.0`.
    pub intensity: f32,
    /// Roughness at which reflections have faded out entirely.
    ///
    /// A rough surface scatters, and a single traced ray is the wrong answer
    /// for it — all three reference engines fade the effect out with roughness
    /// rather than pretending one ray describes a rough reflection.
    pub max_roughness: f32,
    /// How many steps a ray takes before giving up.
    ///
    /// The cost is linear in this and so is how far a reflection can reach.
    /// A ray that runs out of steps returns nothing, which leaves the surface
    /// shaded as it was.
    pub steps: u32,
    /// World-space distance per step.
    ///
    /// Together with [`Self::steps`] this is the reach: `steps * stride`. A
    /// long stride reaches further for the same cost and steps over thin
    /// geometry while doing it.
    pub stride: f32,
    /// How deep a surface is assumed to be, in NDC depth.
    ///
    /// The depth buffer says where surfaces start, never where they end, so a
    /// ray that has gone behind one has to decide whether it hit the surface or
    /// passed it. Without this every ray eventually reports a hit on the far
    /// side of the world.
    pub thickness: f32,
}

impl Default for ScreenSpaceReflections {
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 1.0,
            max_roughness: 0.4,
            steps: 32,
            stride: 0.25,
            thickness: 0.002,
        }
    }
}

impl ScreenSpaceReflections {
    /// The settings with every field clamped to a value the tracer can use.
    ///
    /// Each of these is silent rather than loud if left alone: a zero stride
    /// marches in place and always reports a hit at the origin, a zero
    /// thickness never reports one at all, and a negative intensity subtracts
    /// light.
    pub fn clamped(&self) -> Self {
        Self {
            enabled: self.enabled,
            intensity: self.intensity.clamp(0.0, 1.0),
            max_roughness: self.max_roughness.clamp(0.0, 1.0),
            steps: self.steps.clamp(1, 256),
            stride: self.stride.max(1e-3),
            thickness: self.thickness.max(1e-5),
        }
    }

    /// Intensity the renderer should use: zero whenever the effect is off.
    ///
    /// One number rather than a flag plus a number, because the tracer already
    /// multiplies by intensity — so "off" and "contributes nothing" are the
    /// same state, and there is no way for them to disagree.
    pub fn effective_intensity(&self) -> f32 {
        if self.enabled {
            self.clamped().intensity
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabling_is_the_same_as_contributing_nothing() {
        // The renderer branches on intensity alone, so these must not be able
        // to disagree.
        let off = ScreenSpaceReflections {
            enabled: false,
            ..Default::default()
        };
        assert_eq!(off.effective_intensity(), 0.0);
        assert!(ScreenSpaceReflections::default().effective_intensity() > 0.0);
    }

    #[test]
    fn a_zero_stride_is_widened_so_the_ray_actually_moves() {
        // A ray that steps nowhere samples its own pixel every time and reports
        // a hit at the surface it started from -- a mirror showing itself.
        let s = ScreenSpaceReflections {
            stride: 0.0,
            ..Default::default()
        }
        .clamped();
        assert!(s.stride > 0.0);
    }

    #[test]
    fn a_zero_thickness_is_widened_so_a_hit_is_possible() {
        // Thickness is the window in which a ray counts as having hit. Zero
        // closes it, and no ray ever hits anything.
        let s = ScreenSpaceReflections {
            thickness: 0.0,
            ..Default::default()
        }
        .clamped();
        assert!(s.thickness > 0.0);
    }

    #[test]
    fn nonsense_values_are_clamped_rather_than_passed_on() {
        let s = ScreenSpaceReflections {
            intensity: 5.0,
            max_roughness: -1.0,
            steps: 100_000,
            ..Default::default()
        }
        .clamped();
        assert_eq!(s.intensity, 1.0);
        assert_eq!(s.max_roughness, 0.0);
        assert_eq!(s.steps, 256, "an unbounded march would hang the frame");
    }

    #[test]
    fn the_default_reaches_a_useful_distance() {
        // steps * stride is the reach. A default that could only see a few
        // centimetres would look broken rather than subtle.
        let s = ScreenSpaceReflections::default().clamped();
        assert!(
            s.steps as f32 * s.stride >= 4.0,
            "reach {} is too short to reflect anything in a room-sized scene",
            s.steps as f32 * s.stride
        );
    }
}
