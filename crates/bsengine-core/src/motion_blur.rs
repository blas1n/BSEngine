//! Camera motion blur: smearing the image along how the camera moved.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Blurs each pixel along the path its surface took across the screen since
/// the previous frame, as far as the *camera's* motion moved it.
///
/// This is Unity URP's motion blur, which is camera-only: the velocity comes
/// from reprojecting the pixel's depth through last frame's camera, so a
/// turning or moving camera streaks the world, while an object moving on its
/// own in front of a still camera is not blurred. Unreal and HDRP also blur
/// moving objects, from a per-object velocity buffer the renderer does not
/// have; Godot has no motion blur at all. The parameters are the ones Unity
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
