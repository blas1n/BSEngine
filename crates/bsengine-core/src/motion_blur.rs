//! Motion blur: smearing the image along how each surface moved on screen.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Blurs each pixel along the path its surface took across the screen since
/// the previous frame.
///
/// The path is the velocity the opaque pass writes for every mesh -- its
/// own motion since last frame and the camera's together -- so a turning
/// camera streaks the world and an object moving on its own streaks in front
/// of a still camera, as in Unreal and HDRP (Unity URP blurs the camera's
/// motion only; Godot has no motion blur). Surfaces that write no velocity
/// -- the sky, terrain, custom shaders -- streak by the camera's motion
/// alone, reprojected from depth. Skinned meshes streak by their entity's
/// motion, not their bones'. The parameters are the ones Unity
/// and Unreal share: an intensity (the fraction of the frame's motion blurred,
/// Unity's Intensity and Unreal's Amount, both 0.5 by default) and a clamp on
/// the length of the streak (both default to 5% of the screen).
///
/// The streak is centred on the pixel and averaged over [`Self::samples`]
/// taps of the HDR image before tonemapping, after depth of field -- where
/// Unreal places it -- so bright highlights streak brightly.
///
/// The first frame after a camera cut, and the first frame ever, has no
/// previous camera to measure against and is left sharp.
///
/// Absent means off: a camera without this component renders exactly as it
/// did before.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct MotionBlur {
    /// Whether the effect is applied at all.
    pub enabled: bool,
    /// Fraction of the camera's motion over the last frame that is blurred:
    /// 1 streaks a pixel across everywhere it went since the last frame, 0.5
    /// (the default) across half of it, like a 180-degree shutter.
    pub intensity: f32,
    /// Longest streak, as a fraction of the screen's width. A sudden turn is
    /// clamped to this rather than smeared across the whole frame.
    pub max_blur: f32,
    /// Taps along the streak. More is smoother and slower; clamped to 2..=32.
    pub samples: u32,
}

impl MotionBlur {
    /// The sample count the pass actually uses.
    pub fn clamped_samples(&self) -> u32 {
        self.samples.clamp(2, 32)
    }
}

impl Default for MotionBlur {
    /// Unity's and Unreal's defaults: half the frame's motion, streaks no
    /// longer than 5% of the screen; and Unity's Medium quality's 8 taps.
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 0.5,
            max_blur: 0.05,
            samples: 8,
        }
    }
}
