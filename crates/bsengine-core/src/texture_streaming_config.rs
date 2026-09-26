//! Texture streaming settings, set from `project.toml`'s `[render]` section.
//!
//! The constants live here for the reason `shadow_config` gives: the
//! streaming code in `bsengine-rhi-wgpu` needs the defaults as fallbacks,
//! and the runtime needs them as what a project file falls back to. One
//! definition keeps the two from disagreeing.

use bevy_ecs::prelude::Resource;

/// How much GPU memory streamed textures may hold between them, in
/// mebibytes, when a project does not say otherwise.
///
/// Unity's `streamingMipmapsMemoryBudget` defaults to 512 MB; Unreal's
/// `r.Streaming.PoolSize` to 1000 MB. The lower of the two: the projects in
/// this repository hold a few kilobytes of texture between them, so the
/// budget is a ceiling nothing here approaches, and the smaller default
/// is the one a project that *does* approach it would rather find out
/// about first.
pub const DEFAULT_TEXTURE_STREAMING_BUDGET_MB: u32 = 512;

/// Mip bias applied to every streamed texture's wanted level when a
/// project does not say otherwise: none.
///
/// Both Unity (`QualitySettings.streamingMipmapsMemoryBudget`'s companion
/// `Texture.streamingTextureDiscardUnusedMips` aside, the per-quality mip
/// bias) and Unreal (`r.Streaming.MipBias`) expose one number that trades
/// sharpness for memory across the board; `1` means every texture settles
/// one level smaller than the screen asks for.
pub const DEFAULT_TEXTURE_MIP_BIAS: i32 = 0;

/// Texture streaming settings for this run.
///
/// Inserted by the runtime from `project.toml`'s `[render]` section. When
/// the resource is **absent** -- the editor, and every test that builds an
/// app directly -- the defaults in this module apply.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureStreamingSettings {
    /// The most bytes streamed textures may hold on the GPU between them.
    /// Over it, the streamer drops the largest resident level of the
    /// texture that needs it least, one level per frame, until under;
    /// under it, a level is only brought in when it fits. Zero means no
    /// budget, which is what a project with a few small textures wants.
    pub budget_bytes: u64,
    /// Added to every texture's wanted level: positive keeps textures
    /// smaller than the screen asks for, negative larger. Applied after the
    /// screen-size estimate and clamped to the chain, so `-8` means "always
    /// whole".
    pub mip_bias: i32,
}

impl TextureStreamingSettings {
    /// The budget in mebibytes, as the manifest spells it.
    pub fn from_manifest(budget_mb: u32, mip_bias: i32) -> Self {
        Self {
            budget_bytes: budget_mb as u64 * 1024 * 1024,
            mip_bias,
        }
    }
}

impl Default for TextureStreamingSettings {
    fn default() -> Self {
        Self::from_manifest(
            DEFAULT_TEXTURE_STREAMING_BUDGET_MB,
            DEFAULT_TEXTURE_MIP_BIAS,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_budget_is_the_constant_in_bytes() {
        let s = TextureStreamingSettings::default();
        assert_eq!(s.budget_bytes, 512 * 1024 * 1024);
        assert_eq!(s.mip_bias, 0);
        assert_eq!(
            TextureStreamingSettings::from_manifest(0, -2),
            TextureStreamingSettings {
                budget_bytes: 0,
                mip_bias: -2
            }
        );
    }
}
