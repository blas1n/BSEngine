//! The one `import:` value an asset's `.meta` sidecar carries, whichever
//! kind of asset it is.
//!
//! Lives here rather than in `bsengine_asset` (which owns the sidecar) because
//! the editor's panels edit it: `bsengine_rhi_wgpu`'s Inspector draws the
//! fields and queues the result as an `InspectorCmd`, and that crate sits
//! below the asset crate in the dependency order. The per-kind settings it
//! wraps were already here for the same reason.

use crate::{ModelImportSettings, TextureImportSettings};
use serde::{Deserialize, Serialize};

/// What a sidecar says about *how* its asset is imported, beside *what* it
/// is. One variant per kind of asset that has import settings at all.
///
/// An enum rather than one struct with every kind's fields, so a texture's
/// sidecar cannot carry a model's scale and a hand-edit that puts the wrong
/// kind's *fields* on an asset is a parse error rather than a silently
/// ignored field. The wrong *variant* -- `Model(..)` beside a `.png` --
/// parses, and each loader warns about it by name.
///
/// Not `Eq`: a model's `scale` is an `f32`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ImportSettings {
    /// An image file: how it becomes a GPU texture.
    Texture(TextureImportSettings),
    /// A model file (glTF/GLB): how it becomes geometry, a skeleton and clips.
    Model(ModelImportSettings),
}

impl ImportSettings {
    /// The variant's name, for a warning that has to say which kind a
    /// sidecar recorded when it was not the kind the loader wanted, and for
    /// the Inspector's header.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Texture(_) => "Texture",
            Self::Model(_) => "Model",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The RON spelling inside a sidecar's `import: Some(..)`, pinned here
    /// as well as in the sidecar's own tests, since this is the type that
    /// decides it.
    #[test]
    fn the_ron_spelling_names_the_kind_around_its_fields() {
        assert_eq!(
            ron::to_string(&ImportSettings::Model(ModelImportSettings::default())).unwrap(),
            "Model((scale:1.0,import_animations:true))"
        );
        assert_eq!(
            ron::to_string(&ImportSettings::Texture(TextureImportSettings::default())).unwrap(),
            "Texture((srgb:true,mipmaps:true,filter:Linear,wrap:Repeat))"
        );
        assert_eq!(
            ImportSettings::Model(ModelImportSettings::default()).kind(),
            "Model"
        );
        assert_eq!(
            ImportSettings::Texture(TextureImportSettings::default()).kind(),
            "Texture"
        );
    }
}
