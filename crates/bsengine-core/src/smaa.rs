//! Subpixel morphological antialiasing (SMAA) settings for a camera.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Subpixel morphological antialiasing (SMAA 1x): three post passes over the
/// finished frame -- find edges by luma, classify each edge's shape (a step,
/// a diagonal, a corner) by searching along it, and blend each pixel with the
/// neighbour across the edge by the area the true edge would cover.
///
/// Where FXAA ([`crate::Fxaa`]) blurs across any high-contrast edge it finds,
/// SMAA reconstructs the line the stair-steps came from, so it keeps text and
/// thin features sharper for a little more cost. Unity URP and Godot both
/// offer it next to FXAA (URP's Low/Medium/High; Godot's
/// `screen_space_aa = SMAA`); Unreal does not ship it.
///
/// It runs where FXAA does: on the tonemapped image, before the TAA resolve.
/// Both reference engines make FXAA and SMAA one choice, never both, so a
/// camera with both components runs SMAA and skips FXAA.
///
/// Absent means off: a camera without this component renders exactly as it
/// did before.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct Smaa {
    /// Whether the passes run at all.
    pub enabled: bool,
    /// The SMAA preset: edge threshold, search distance, and whether diagonal
    /// and corner shapes are recognised.
    pub quality: SmaaQuality,
}

impl Default for Smaa {
    fn default() -> Self {
        Self {
            enabled: true,
            quality: SmaaQuality::High,
        }
    }
}

/// SMAA's four presets, as the reference implementation defines them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[reflect(Default)]
pub enum SmaaQuality {
    /// Threshold 0.15, 4 search steps, no diagonal or corner detection.
    Low,
    /// Threshold 0.1, 8 search steps, no diagonal or corner detection.
    Medium,
    /// Threshold 0.1, 16 search steps, 8 diagonal steps, corner rounding --
    /// the default, as in Unity URP.
    #[default]
    High,
    /// Threshold 0.05, 32 search steps, 16 diagonal steps, corner rounding.
    Ultra,
}

impl SmaaQuality {
    /// `(threshold, max_search_steps, max_search_steps_diag, corners)`: the
    /// preset's values from `SMAA.hlsl`. A diagonal step count of 0 means
    /// diagonal detection is off.
    pub fn parameters(self) -> (f32, u32, u32, bool) {
        match self {
            SmaaQuality::Low => (0.15, 4, 0, false),
            SmaaQuality::Medium => (0.1, 8, 0, false),
            SmaaQuality::High => (0.1, 16, 8, true),
            SmaaQuality::Ultra => (0.05, 32, 16, true),
        }
    }
}
