//! Screen-space global illumination applied by a camera.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Diffuse light bounced off what the camera can see, traced through the
/// depth buffer: a red wall bleeding onto the floor between the light probes,
/// light from a moving object no probe was placed for, the bounce in a
/// corner. Where a ray finds nothing on screen, the light the surface already
/// had -- the sky's, the probes', the lightmap's -- stands; where it finds
/// something, that is what the surface is lit by instead (Unity HDRP's SSGI
/// and Unreal's Lumen screen traces replace the indirect diffuse the same
/// way, rather than adding to it).
///
/// A camera setting, as Unity, Unreal and Godot all make it: its cost and its
/// reach belong to the view. Absent means no screen-space GI, which is what
/// every scene authored before it says; present, it is on.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct ScreenSpaceGlobalIllumination {
    /// Whether the effect is applied at all.
    pub enabled: bool,
    /// How much of the traced light replaces the light the surface had,
    /// `0.0..=1.0`.
    pub intensity: f32,
    /// Rays each pixel traces, in a cosine-weighted hemisphere. More is less
    /// noise for more time.
    pub rays: u32,
    /// How many steps a ray takes before giving up. A ray that runs out of
    /// steps finds nothing, and leaves that share of the light as it was.
    pub steps: u32,
    /// World-space distance per step; `steps * stride` is the reach.
    pub stride: f32,
    /// How deep a surface is assumed to be, in NDC depth: the depth buffer
    /// says where surfaces start, never where they end.
    pub thickness: f32,
}

impl Default for ScreenSpaceGlobalIllumination {
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 1.0,
            rays: 4,
            steps: 24,
            stride: 0.2,
            thickness: 0.002,
        }
    }
}

impl ScreenSpaceGlobalIllumination {
    /// The settings with every field clamped to a value the tracer can use:
    /// a zero stride marches in place, a zero thickness never hits, an
    /// unbounded ray or step count would hang the frame.
    pub fn clamped(&self) -> Self {
        Self {
            enabled: self.enabled,
            intensity: self.intensity.clamp(0.0, 1.0),
            rays: self.rays.clamp(1, 16),
            steps: self.steps.clamp(1, 128),
            stride: self.stride.max(1e-3),
            thickness: self.thickness.max(1e-5),
        }
    }

    /// Intensity the renderer should use: zero whenever the effect is off,
    /// so "off" and "contributes nothing" are one state.
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
        let off = ScreenSpaceGlobalIllumination {
            enabled: false,
            ..Default::default()
        };
        assert_eq!(off.effective_intensity(), 0.0);
        assert!(ScreenSpaceGlobalIllumination::default().effective_intensity() > 0.0);
    }

    #[test]
    fn nonsense_values_are_clamped_rather_than_passed_on() {
        let s = ScreenSpaceGlobalIllumination {
            intensity: 3.0,
            rays: 0,
            steps: 100_000,
            stride: 0.0,
            thickness: 0.0,
            ..Default::default()
        }
        .clamped();
        assert_eq!(s.intensity, 1.0);
        assert_eq!(s.rays, 1, "no rays would trace nothing");
        assert_eq!(s.steps, 128, "an unbounded march would hang the frame");
        assert!(s.stride > 0.0 && s.thickness > 0.0);
    }

    #[test]
    fn the_default_reaches_across_a_room() {
        let s = ScreenSpaceGlobalIllumination::default().clamped();
        assert!(
            s.steps as f32 * s.stride >= 4.0,
            "reach {}",
            s.steps as f32 * s.stride
        );
    }
}
