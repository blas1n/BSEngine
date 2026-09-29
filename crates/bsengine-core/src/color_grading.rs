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
/// Last, a colour lookup table, if one is named: Unity's Color Lookup and
/// Unreal's LUT, applied after the adjustments as Unity orders them. The
/// image is a horizontal strip of `N` slices, each `N` by `N` -- 256x16 or
/// 1024x32 -- in Unreal's layout: top-left black, red increasing to the right
/// within a slice, green downward, blue from slice to slice. Its texels are
/// the display-space colours to output, the space the adjustments above
/// work in, so a LUT exported from a grading tool applies as authored. The
/// lookup is trilinear: blended across the two nearest slices.
///
/// The default is the identity, and absent means off: a camera without this
/// component renders exactly as it did before it existed.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
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
    /// Project-relative path of a LUT strip image; empty for none. An image
    /// that is not a strip (width not height squared) is reported and not
    /// applied.
    pub lut: String,
    /// How much of the LUT's result replaces the colour: 1 is all of it, 0
    /// none -- Unity's Contribution.
    pub lut_contribution: f32,
}

impl Default for ColorGrading {
    fn default() -> Self {
        Self {
            enabled: true,
            contrast: 1.0,
            saturation: 1.0,
            color_filter: glam::Vec3::ONE.into(),
            lut: String::new(),
            lut_contribution: 1.0,
        }
    }
}
