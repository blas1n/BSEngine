//! Colour grading: contrast, saturation and a colour filter applied to the
//! final image.

use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Grades what the camera shows, after tonemapping.
///
/// Unity's Color Adjustments, Godot's Environment adjustments and the
/// global part of Unreal's Color Grading agree on these three controls.
/// Where they disagree -- Unreal and HDRP grade in scene-referred HDR before
/// the tonemapper, Godot and URP's LDR mode after it -- this follows Godot:
/// display-referred, after tonemapping. Every engine applies a colour LUT
/// in display space, and grading in the same space as the LUT keeps one
/// meaning for "0.5" across both.
///
/// Applied in Unity's order: contrast, then the colour filter, then
/// saturation. Contrast pivots on mid-grey (display 0.5), saturation on
/// Rec. 709 luma -- Godot's formulas.
///
/// The default is the identity, and absent means off: a camera without this
/// component renders exactly as it did before it existed.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct ColorGrading {
    /// Whether the grade is applied at all.
    pub enabled: bool,
    /// 1 leaves the image alone; above 1 pushes values away from mid-grey,
    /// below 1 pulls them toward it, 0 is flat grey.
    pub contrast: f32,
    /// 1 leaves colour alone; 0 is greyscale; above 1 intensifies.
    pub saturation: f32,
    /// Multiplies every pixel: white leaves the image alone, a colour tints
    /// it toward that colour.
    pub color_filter: ReflectVec3,
}

impl Default for ColorGrading {
    fn default() -> Self {
        Self {
            enabled: true,
            contrast: 1.0,
            saturation: 1.0,
            color_filter: glam::Vec3::ONE.into(),
        }
    }
}
