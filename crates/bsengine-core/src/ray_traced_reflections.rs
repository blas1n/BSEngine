//! Ray-traced reflections applied by a camera.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Reflections traced through the scene itself, where the screen has no
/// answer, applied by a camera.
///
/// Screen-space reflections ([`crate::ScreenSpaceReflections`]) reflect only
/// what is on screen: a mirror floor cannot show the wall behind the camera,
/// or anything a pillar hides. This traces a ray along each such pixel's
/// reflection through the realtime GI's scene -- the same tree, moving
/// instances and skinned meshes its probes are traced through -- and shades
/// what it hits as the probes' rays do: its direct light, with shadows, and
/// the probes' bounce. HDRP's "Mixed" ray-traced reflections and Lumen's
/// reflections are built the same way: the screen first, rays for the share it
/// did not find, and what neither finds left to the reflection probes and the
/// sky.
///
/// It needs a realtime [`crate::LightProbeVolume`] (`update: Realtime`):
/// without one there is no scene to trace, and the camera keeps its
/// screen-space reflections alone, as Lumen's reflections need Lumen's scene.
/// What it hits is drawn as the GI sees it -- each surface's material colour,
/// not its textures -- the same approximation Lumen's surface cache makes at
/// a coarser grain.
///
/// Absent means none, which is what every scene authored before this existed
/// says; switching it on changes every smooth surface's reflection, which is
/// the scene author's decision, as [`crate::ScreenSpaceReflections`] is.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct RayTracedReflections {
    /// Whether the effect is applied at all.
    pub enabled: bool,
    /// How much of a surface's specular environment a hit replaces,
    /// `0.0..=1.0`.
    pub intensity: f32,
    /// Roughness up to which surfaces trace, fading out over the next 0.2.
    ///
    /// Rougher than that, one mirror ray is the wrong answer, and the probes
    /// answer instead: their light facing the reflection, through the same
    /// 0.4 to 0.6 blend -- Lumen's `MaxRoughnessToTrace`, whose default this
    /// is.
    pub max_roughness: f32,
    /// How far a ray looks, in world units. A ray that hits nothing within
    /// it leaves the surface to the reflection probes and the sky.
    pub max_distance: f32,
    /// Trace every pixel, rather than one in each 2 x 2 block brought back
    /// up by depth -- four times the rays. Off by default, as HDRP's
    /// ray-traced reflections and this engine's screen-space GI
    /// (`ScreenSpaceGlobalIllumination::full_resolution`) are.
    pub full_resolution: bool,
}

impl Default for RayTracedReflections {
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 1.0,
            max_roughness: 0.4,
            // HDRP's ray-traced reflections' default ray length.
            max_distance: 50.0,
            full_resolution: false,
        }
    }
}

impl RayTracedReflections {
    /// The settings with every field clamped to a value the tracer can use:
    /// a negative intensity would subtract light, and a distance of zero or
    /// less would find nothing while still paying for every ray.
    pub fn clamped(&self) -> Self {
        Self {
            enabled: self.enabled,
            intensity: self.intensity.clamp(0.0, 1.0),
            max_roughness: self.max_roughness.clamp(0.0, 1.0),
            max_distance: self.max_distance.max(1e-3),
            full_resolution: self.full_resolution,
        }
    }

    /// Intensity the renderer should use: zero whenever the effect is off,
    /// so "off" and "contributes nothing" are one state, as for
    /// [`crate::ScreenSpaceReflections::effective_intensity`].
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
        let off = RayTracedReflections {
            enabled: false,
            ..Default::default()
        };
        assert_eq!(off.effective_intensity(), 0.0);
        assert_eq!(RayTracedReflections::default().effective_intensity(), 1.0);
    }

    #[test]
    fn clamping_keeps_every_field_usable() {
        let c = RayTracedReflections {
            enabled: true,
            intensity: 3.0,
            max_roughness: -1.0,
            max_distance: 0.0,
            full_resolution: false,
        }
        .clamped();
        assert_eq!(c.intensity, 1.0);
        assert_eq!(c.max_roughness, 0.0);
        assert!(c.max_distance > 0.0);
    }
}
