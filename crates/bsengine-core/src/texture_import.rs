//! How an image file becomes a GPU texture.
//!
//! Unity's `TextureImporter`, Godot's `.import` file and Unreal's texture
//! asset settings all answer the same four questions about a texture before
//! it is uploaded, and every one of them defaults the same way: colour
//! textures are sRGB, mipmaps are generated, sampling is linear, and UVs
//! repeat. [`TextureImportSettings::default`] is those answers. A fifth,
//! whether the mip chain is streamed in rather than uploaded whole, is asked
//! per texture by Unity and Unreal and defaults to off here as it does in
//! Unity (Godot 4 has no texture streaming). The settings live beside the
//! asset in its `.meta` sidecar (`bsengine_asset`), which is where Unity and
//! Godot keep theirs too, and are read at load time by the texture loader.
//!
//! Before these existed every texture was uploaded the same one way --
//! linear, no mips, clamped -- which is [`TextureImportSettings::raw`], and
//! is still what a texture built from bytes in memory gets: a pixel test's
//! two-texel probe or a UI image has no sidecar and asked for nothing.

use serde::{Deserialize, Serialize};

/// Per-texture import settings.
///
/// Spelled in a sidecar as
/// `import: Some(Texture((srgb: true, mipmaps: true, filter: Linear, wrap: Repeat, streaming: false)))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextureImportSettings {
    /// Whether the pixel values are sRGB-encoded, as every colour image an
    /// artist exports is. The GPU then decodes them to linear light when
    /// sampling, which is what the lighting math expects. Off for *data*
    /// textures -- normal maps, roughness, masks -- whose numbers are not
    /// colours and must be read as written.
    ///
    /// The renderer's output target is sRGB, so a colour texture uploaded
    /// as linear reads its midtones as brighter than they were painted:
    /// a 50% grey comes back at about 73%.
    pub srgb: bool,
    /// Whether to generate the full mip chain. Without it a texture drawn
    /// smaller than its own resolution is point-sampled from the full image
    /// and shimmers as the camera moves.
    pub mipmaps: bool,
    /// How texels are blended when the texture is magnified or minified.
    pub filter: TextureFilter,
    /// What a UV outside `0..1` samples.
    pub wrap: TextureWrap,
    /// Whether the texture's mip levels are streamed: only the small levels
    /// are resident on the GPU at first, and the larger ones are brought in
    /// afterwards, level by level -- Unity's per-texture "Streaming Mipmaps"
    /// and the inverse of Unreal's "Never Stream". Off, the whole chain is
    /// uploaded at load, as every texture was before this existed.
    ///
    /// Only meaningful with `mipmaps`; a streamed texture without a chain has
    /// nothing to stream and is uploaded whole.
    ///
    /// `serde(default)` so every sidecar written before this field existed
    /// still parses -- a missing field is a hard error otherwise, and the
    /// loader would then read the whole sidecar as the defaults and silently
    /// drop the settings an author had tuned.
    #[serde(default)]
    pub streaming: bool,
}

impl Default for TextureImportSettings {
    /// The reference engines' defaults: a colour texture, mipmapped,
    /// sampled linearly, tiling, not streamed.
    fn default() -> Self {
        Self {
            srgb: true,
            mipmaps: true,
            filter: TextureFilter::Linear,
            wrap: TextureWrap::Repeat,
            streaming: false,
        }
    }
}

impl TextureImportSettings {
    /// What a texture built from bytes in memory gets, and what every
    /// texture got before import settings existed: linear values, one mip
    /// level, clamped UVs.
    ///
    /// Kept as the behaviour of the settings-less upload path on purpose.
    /// The pixel tests build their probe textures from a handful of texels
    /// and assert on exact channel values; decoding those as sRGB or
    /// blending them across mips would change every number they pin.
    pub const fn raw() -> Self {
        Self {
            srgb: false,
            mipmaps: false,
            filter: TextureFilter::Linear,
            wrap: TextureWrap::Clamp,
            streaming: false,
        }
    }
}

/// How texels are blended when sampled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TextureFilter {
    /// Bilinear between the nearest texels; smooth, the default everywhere.
    #[default]
    Linear,
    /// The nearest texel, unblended: hard edges, for pixel art.
    Nearest,
}

/// What a UV outside `0..1` samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TextureWrap {
    /// The texture tiles.
    #[default]
    Repeat,
    /// The edge texel is stretched.
    Clamp,
    /// The texture tiles, flipped on every other repeat.
    Mirror,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults are the ones this module's doc claims, and `raw` is the
    /// old behaviour -- both spelled out because a wrong default here changes
    /// how every texture in every project looks.
    #[test]
    fn defaults_are_the_reference_engines_and_raw_is_the_old_upload() {
        let d = TextureImportSettings::default();
        assert!(d.srgb && d.mipmaps);
        assert_eq!(
            (d.filter, d.wrap),
            (TextureFilter::Linear, TextureWrap::Repeat)
        );

        let r = TextureImportSettings::raw();
        assert!(!r.srgb && !r.mipmaps);
        assert_eq!(
            (r.filter, r.wrap),
            (TextureFilter::Linear, TextureWrap::Clamp)
        );
    }

    /// The RON spelling a sidecar uses, pinned, since humans edit it.
    #[test]
    fn the_ron_spelling_is_the_documented_one() {
        let text = ron::to_string(&TextureImportSettings::default()).unwrap();
        assert_eq!(
            text,
            "(srgb:true,mipmaps:true,filter:Linear,wrap:Repeat,streaming:false)"
        );
        let parsed: TextureImportSettings = ron::from_str(
            "(srgb: false, mipmaps: false, filter: Nearest, wrap: Mirror, streaming: true)",
        )
        .unwrap();
        assert_eq!(
            parsed,
            TextureImportSettings {
                srgb: false,
                mipmaps: false,
                filter: TextureFilter::Nearest,
                wrap: TextureWrap::Mirror,
                streaming: true,
            }
        );
    }

    /// Every sidecar tuned before `streaming` existed spells the settings
    /// without it, and must keep meaning what it meant -- not fail, and not
    /// fall back to the defaults with the author's `srgb: false` lost.
    #[test]
    fn a_sidecar_written_before_streaming_existed_still_parses_with_it_off() {
        let parsed: TextureImportSettings =
            ron::from_str("(srgb: false, mipmaps: true, filter: Nearest, wrap: Clamp)")
                .expect("the pre-streaming shape must parse");
        assert!(!parsed.streaming);
        assert!(!parsed.srgb, "and the tuned fields must survive");
        assert_eq!(parsed.filter, TextureFilter::Nearest);
    }
}
