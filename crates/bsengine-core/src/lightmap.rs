//! Baked lighting for a static renderer.
//!
//! A lightmap is the indirect light a surface receives, computed offline and
//! stored in a texture laid out by the mesh's second UV set (`Vertex::uv2` in
//! the renderer). Unity (`Lightmap
//! Index` + `Scale Offset` on a `MeshRenderer`), Unreal (`LightMapTexture`
//! per component) and Godot (`LightmapGI` slices) all converge on the same
//! shape: one atlas shared by many renderers, and per renderer the rectangle
//! of it that belongs to that renderer. This is that shape.
//!
//! On a renderer that has one, the lightmap **replaces** the diffuse
//! ambient/IBL/probe term; it never adds to it. The probes and the sky
//! already carry that light, so adding would count it twice. Specular
//! reflections are untouched: a lightmap only knows what a rough surface sees.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// How far above 1.0 a lightmap texel can reach, in the RGBM encoding the
/// atlas is stored in.
///
/// Indirect light is a quantity, not a colour: a texel under a skylight can
/// be several times brighter than 1.0, and an 8-bit texture clamps at 1.0.
/// RGBM keeps the texture an ordinary 8-bit image -- so it loads, uploads,
/// hot-reloads and ships through the same paths every other texture does --
/// by storing `rgb / (a * RANGE)` and `a = max(r, g, b) / RANGE`. Decoding is
/// `rgb * a * RANGE`. The baker writes with this constant and the shader reads
/// with it, so they cannot drift apart.
pub const LIGHTMAP_RGBM_RANGE: f32 = 8.0;

/// A baked lightmap for this entity's mesh.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct Lightmap {
    /// The atlas texture, as a path, the way materials name theirs. RGBM
    /// encoded ([`LIGHTMAP_RGBM_RANGE`]) and **linear**: its `.meta` sidecar
    /// must say `srgb: false`, or the GPU decodes the encoded values as if
    /// they were colours and every texel comes out wrong.
    ///
    /// Empty means no lightmap, and the renderer shades as it always did.
    pub texture_path: String,
    /// Which rectangle of the atlas is this renderer's: `[scale_x, scale_y,
    /// offset_x, offset_y]`, applied as `uv2 * scale + offset`. The default
    /// `[1, 1, 0, 0]` is a renderer that owns the whole atlas.
    pub scale_offset: [f32; 4],
}

impl Default for Lightmap {
    fn default() -> Self {
        Self {
            texture_path: String::new(),
            scale_offset: [1.0, 1.0, 0.0, 0.0],
        }
    }
}

impl Lightmap {
    /// A lightmap that owns the whole of the named atlas.
    pub fn new(texture_path: impl Into<String>) -> Self {
        Self {
            texture_path: texture_path.into(),
            ..Default::default()
        }
    }

    /// Whether this names an atlas at all.
    pub fn is_set(&self) -> bool {
        !self.texture_path.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_lightmap_names_nothing_and_owns_the_whole_atlas() {
        let l = Lightmap::default();
        assert!(!l.is_set(), "nothing to sample until authored");
        assert_eq!(l.scale_offset, [1.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn naming_an_atlas_sets_it() {
        assert!(Lightmap::new("lightmaps/level1.png").is_set());
    }
}
