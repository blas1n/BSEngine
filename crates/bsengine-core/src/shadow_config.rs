//! Directional shadow settings, set from `project.toml`'s `[render]` section.
//!
//! The constants live here rather than beside the fitting code in
//! `bsengine-rhi-wgpu::shadow` for one reason: that crate needs them as array
//! bounds and fallbacks, and this crate needs them as the defaults a project
//! file falls back to. Defining them twice would let a default and a bound
//! disagree, and the symptom would be a cascade the shader can select but the
//! uniform has no room for.

use bevy_ecs::prelude::Resource;

/// How far from the camera directional shadows are drawn, in world units.
///
/// Everything beyond this is lit as though nothing occludes it. 200 is in the
/// same range the reference engines default to: Unity's Shadow Distance is 150
/// and Unreal's `DynamicShadowDistanceMovableLight` is 20000 uu (200 m).
pub const DEFAULT_SHADOW_DISTANCE: f32 = 200.0;

/// The most cascades the shadow map is built to hold.
///
/// Four, which is where Unity and Godot both cap their directional cascade
/// counts, and comfortably above Unreal's default of three.
pub const MAX_CASCADES: usize = 4;

/// Cascades used when a project does not say otherwise.
pub const DEFAULT_CASCADES: usize = 4;

/// Shape of the cascade split distribution.
///
/// Boundary `i` of `n` sits at `distance * (i / n).powf(exponent)`, so 1.0
/// spaces cascades evenly and larger values pack them towards the camera —
/// which is what you want, because near geometry covers more screen per world
/// unit.
///
/// This is Unreal's mechanism (`CascadeDistributionExponent`) rather than
/// Unity's and Godot's hand-entered percentages, because it generalises over
/// the cascade count instead of needing a table per count. At 2.0 it lands
/// close to Unity's own 4-cascade defaults — 6.25/25/56/100% of the shadow
/// distance against Unity's 6.7/20/46.7/100.
pub const DEFAULT_SPLIT_EXPONENT: f32 = 2.0;

/// Width of the cross-fade at each cascade boundary, as a fraction of that
/// cascade's far distance.
///
/// Nonzero by default, which is a deliberate choice between diverging
/// precedents: Unreal fades between cascades by default, while Unity's cascade
/// blending and Godot's "blend splits" both default off. Neighbouring cascades
/// have visibly different texel densities, so an unfaded boundary reads as a
/// step in shadow sharpness at a fixed distance from the player — which is
/// exactly the artifact a viewer notices while moving. Setting this to 0
/// reproduces Unity's and Godot's hard switch.
pub const DEFAULT_CASCADE_BLEND: f32 = 0.1;

// The uniform array is `MAX_CASCADES` long and the shader indexes it by the
// live count, so a default above the bound would read past the end of a uniform
// -- which WGSL clamps rather than faults on, making it a wrong shadow rather
// than an error. Compile-time, because both sides are constants.
const _: () = assert!(
    DEFAULT_CASCADES <= MAX_CASCADES && DEFAULT_CASCADES >= 1,
    "the default cascade count must be at least 1 and fit in MAX_CASCADES slots"
);

/// Directional shadow settings for this run.
///
/// Inserted by the runtime from `project.toml`'s `[render]` section. When the
/// resource is **absent** — the editor, and every test that builds an app
/// directly — the defaults in this module apply, so a project that says
/// nothing gets the same shadows a project that spells out the defaults does.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct ShadowSettings {
    /// How far from the camera shadows are drawn, in world units.
    pub distance: f32,
    /// How many cascades to split that distance across, clamped to
    /// `1..=`[`MAX_CASCADES`] where it is used.
    pub cascades: usize,
    /// Cross-fade width at each cascade boundary, as a fraction of that
    /// cascade's far distance. Zero switches hard.
    pub blend: f32,
}

impl Default for ShadowSettings {
    fn default() -> Self {
        Self {
            distance: DEFAULT_SHADOW_DISTANCE,
            cascades: DEFAULT_CASCADES,
            blend: DEFAULT_CASCADE_BLEND,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_usable_numbers() {
        let s = ShadowSettings::default();
        assert!(s.distance > 0.0 && s.distance.is_finite());
        assert!((0.0..=1.0).contains(&s.blend));
        assert_eq!(s.cascades, DEFAULT_CASCADES);
    }
}
