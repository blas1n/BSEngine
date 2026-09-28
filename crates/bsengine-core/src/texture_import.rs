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
    /// Whether the decoded pixels are dropped from system memory once the
    /// texture is on the GPU -- the opposite of Unity's "Read/Write
    /// Enabled", which keeps a CPU copy an author can read back. Off, the
    /// pixels stay in `Assets<TextureAsset>` for as long as the texture is
    /// used, which is one image's worth of RAM per texture on top of the
    /// GPU copy. On, the texture cache's upload releases them; the asset
    /// keeps its size, settings and hot-reload handle, and a file change
    /// loads fresh pixels for one more upload.
    ///
    /// **On by default**, as in Unity, where Read/Write Enabled is off
    /// unless an author turns it on. It started off (#1897) while the
    /// skybox and terrain layers still read the pixels for themselves; now
    /// that they take their copy from the texture cache there is no
    /// consumer left that a released image surprises. A sidecar that says
    /// `false` keeps the pixels, which is the switch for an image something
    /// reads on the CPU.
    ///
    /// Safe for any image the GPU samples: materials, UI images, the skybox
    /// and a terrain's layers all take their copy from the one the texture
    /// cache uploads. The one image read on the CPU is a terrain's splatmap
    /// (blend weights baked into vertices); it is never uploaded, so nothing
    /// releases it -- unless the same file is also a material's texture,
    /// which the terrain reports and gives up on.
    ///
    /// `#[serde(default)]` here means a sidecar written before the field
    /// existed reads as *on* -- the field's default, not the old behaviour.
    /// Deliberate: those sidecars were written when there was no choice to
    /// make, and the choice Unity makes for them is the one made here.
    #[serde(default = "default_release_pixels")]
    pub release_pixels: bool,
    /// How the texture is stored on the GPU: as it was decoded, or block
    /// compressed at import. Every reference engine compresses at import
    /// (Unity per platform, Unreal's `TC_Default` as DXT1/DXT5, Godot's
    /// "VRAM Compressed" `.ctex`), because a compressed texture is a
    /// quarter to an eighth of the memory and the bandwidth, and the loss
    /// is invisible on a colour texture. The compressed levels are written
    /// to the project's mip cache once, so the encode is paid at first
    /// import and not at every load.
    ///
    /// Off by default here, unlike the reference engines: this engine's
    /// pixel tests and E2E recordings assert exact texel values, and a
    /// default that made every texture lossy would change what they see.
    /// Turning the default over is its own change, as `release_pixels`'s
    /// was.
    ///
    /// `serde(default)` so every sidecar written before this field existed
    /// still parses, as uncompressed.
    #[serde(default)]
    pub compression: TextureCompression,
}

/// Block compression for a texture's GPU copy.
///
/// The two desktop formats every engine starts from: BC1 (DXT1) for an
/// opaque colour texture, four bits per texel; BC3 (DXT5) for one with an
/// alpha channel worth keeping, eight. Chosen by the author rather than by
/// looking at the alpha channel, as Unreal's `TC_Default` does: an image
/// with an all-opaque alpha and one with a mask look the same to a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TextureCompression {
    /// RGBA8 as decoded. Lossless, four bytes per texel.
    #[default]
    None,
    /// BC1 / DXT1: 8 bytes per 4x4 block, half a byte per texel. No alpha
    /// worth the name (one bit, unused here).
    Bc1,
    /// BC3 / DXT5: 16 bytes per 4x4 block, one byte per texel; BC1 colour
    /// with an interpolated alpha channel.
    Bc3,
}

/// What `release_pixels` is when a sidecar does not say: on, see the field.
fn default_release_pixels() -> bool {
    true
}

impl Default for TextureImportSettings {
    /// The reference engines' defaults: a colour texture, mipmapped,
    /// sampled linearly, tiling, not streamed, pixels released once on the
    /// GPU.
    fn default() -> Self {
        Self {
            srgb: true,
            mipmaps: true,
            filter: TextureFilter::Linear,
            wrap: TextureWrap::Repeat,
            streaming: false,
            release_pixels: default_release_pixels(),
            compression: TextureCompression::None,
        }
    }
}

impl TextureImportSettings {
    /// What a texture built from bytes in memory gets, and what every
    /// texture got before import settings existed: linear values, one mip
    /// level, clamped UVs. `release_pixels` is off because there is no
    /// asset to release from: these bytes never went through `Assets`.
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
            release_pixels: false,
            compression: TextureCompression::None,
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
        assert!(
            d.release_pixels,
            "released once on the GPU, as Unity's Read/Write Enabled is off by default"
        );

        let r = TextureImportSettings::raw();
        assert!(!r.srgb && !r.mipmaps);
        assert_eq!(
            (r.filter, r.wrap),
            (TextureFilter::Linear, TextureWrap::Clamp)
        );
        assert!(
            !r.release_pixels,
            "bytes from memory have no asset to release from"
        );
    }

    /// The RON spelling a sidecar uses, pinned, since humans edit it.
    #[test]
    fn the_ron_spelling_is_the_documented_one() {
        let text = ron::to_string(&TextureImportSettings::default()).unwrap();
        assert_eq!(
            text,
            "(srgb:true,mipmaps:true,filter:Linear,wrap:Repeat,streaming:false,release_pixels:true,compression:None)"
        );
        let parsed: TextureImportSettings = ron::from_str(
            "(srgb: false, mipmaps: false, filter: Nearest, wrap: Mirror, streaming: true, release_pixels: false, compression: Bc3)",
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
                release_pixels: false,
                compression: TextureCompression::Bc3,
            }
        );
    }

    /// Every sidecar tuned before `streaming` or `release_pixels` existed
    /// spells the settings without them, and must keep meaning what it
    /// meant -- not fail, and not fall back to the defaults with the
    /// author's `srgb: false` lost. `release_pixels` reads as on, the
    /// field's default and Unity's: those sidecars were written when there
    /// was no choice to make.
    #[test]
    fn a_sidecar_written_before_streaming_existed_still_parses_with_it_off() {
        let parsed: TextureImportSettings =
            ron::from_str("(srgb: false, mipmaps: true, filter: Nearest, wrap: Clamp)")
                .expect("the pre-streaming shape must parse");
        assert!(!parsed.streaming);
        assert!(
            parsed.release_pixels,
            "an old sidecar takes the field's default, which is on"
        );
        assert!(!parsed.srgb, "and the tuned fields must survive");
        assert_eq!(parsed.filter, TextureFilter::Nearest);
        let parsed: TextureImportSettings = ron::from_str(
            "(srgb: false, mipmaps: true, filter: Nearest, wrap: Clamp, streaming: true)",
        )
        .expect("the pre-release_pixels shape must parse");
        assert!(parsed.streaming && parsed.release_pixels);
        let parsed: TextureImportSettings = ron::from_str(
            "(srgb: false, mipmaps: true, filter: Nearest, wrap: Clamp, streaming: true, release_pixels: false)",
        )
        .expect("the pre-compression shape must parse");
        assert_eq!(
            parsed.compression,
            TextureCompression::None,
            "a sidecar from before compression existed is uncompressed, as it was"
        );
    }
}
