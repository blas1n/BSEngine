//! How a model file (glTF/GLB) becomes engine geometry, a skeleton and clips.
//!
//! Unity's `ModelImporter` (Scale Factor, Import Animation), Unreal's FBX and
//! glTF importers (Import Uniform Scale, Import Animations) and Godot's scene
//! importer (`nodes/root_scale` with `apply_root_scale`, `animation/import`)
//! all put the same two questions first: one uniform scale, and whether the
//! file's animations come in at all. All three default to `1.0` and *on*, and
//! all three **bake** the scale -- into the vertices, the skeleton's rest
//! pose, the bind matrices and the clips' translation keys -- rather than
//! leaving it on a root node for every consumer to remember.
//! [`ModelImportSettings::default`] is those answers, and `bsengine_gltf`
//! bakes the same way: the ragdoll planner, the IK solver and the nav bake
//! all read the loaded data directly, and a scale each of them had to apply
//! on its own is a scale one of them would forget.
//!
//! Like [`TextureImportSettings`](crate::TextureImportSettings), these live
//! beside the asset in its `.meta` sidecar (`bsengine_asset`) and are read at
//! load time, by whichever path loads the file -- the asset server's, or a
//! direct synchronous load.

use serde::{Deserialize, Serialize};

/// Per-model import settings.
///
/// Spelled in a sidecar as
/// `import: Some(Model((scale: 1.0, import_animations: true)))`.
///
/// Not `Eq`: `scale` is an `f32`, and a sidecar carrying one is compared
/// field by field like any other float-bearing record.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModelImportSettings {
    /// A uniform scale baked into everything the file produces: vertex
    /// positions, every node's rest translation, the translation column of
    /// every inverse bind matrix and every animation translation key.
    ///
    /// For a file authored in centimetres (the usual reason to touch this)
    /// `0.01` brings it to the engine's metres. A skinned character scaled
    /// this way deforms to exactly the scaled pose of the unscaled one -- the
    /// four things above are the four that carry a length, and scaling any
    /// three of them is a character whose skin drifts off its skeleton.
    ///
    /// Must be finite and positive; a loader reads anything else as `1.0`
    /// and says so, since a zero collapses the model to a point and a
    /// negative one turns it inside out.
    pub scale: f32,
    /// Whether the file's animation clips are imported. Off, a skinned model
    /// loads with an empty clip library and holds its rest pose -- what a
    /// static prop exported with a stray idle clip, or a model whose clips
    /// are retargeted from another file, wants.
    pub import_animations: bool,
}

impl Default for ModelImportSettings {
    /// The reference engines' defaults: as authored, animations included.
    fn default() -> Self {
        Self {
            scale: 1.0,
            import_animations: true,
        }
    }
}

impl ModelImportSettings {
    /// `scale` as a loader should apply it: the recorded value when it is a
    /// usable one, else `1.0`.
    ///
    /// Returns whether the recorded value was usable alongside, so the
    /// caller -- which knows the asset's path -- can warn with it. A zero or
    /// negative scale is the one hand-edit of this field that produces no
    /// error and no model, and it deserves a line in the log naming the file.
    pub fn usable_scale(&self) -> (f32, bool) {
        if self.scale.is_finite() && self.scale > 0.0 {
            (self.scale, true)
        } else {
            (1.0, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults are the ones this module's doc claims, spelled out
    /// because a wrong default here rescales every model in every project.
    #[test]
    fn defaults_are_as_authored_with_animations() {
        let d = ModelImportSettings::default();
        assert_eq!(d.scale, 1.0);
        assert!(d.import_animations);
    }

    /// The RON spelling a sidecar uses, pinned, since humans edit it.
    #[test]
    fn the_ron_spelling_is_the_documented_one() {
        let text = ron::to_string(&ModelImportSettings::default()).unwrap();
        assert_eq!(text, "(scale:1.0,import_animations:true)");
        let parsed: ModelImportSettings =
            ron::from_str("(scale: 0.01, import_animations: false)").unwrap();
        assert_eq!(
            parsed,
            ModelImportSettings {
                scale: 0.01,
                import_animations: false,
            }
        );
    }

    /// Every value that would make a model vanish or invert is read as
    /// `1.0` and flagged; every usable one is passed through untouched.
    #[test]
    fn a_scale_that_would_collapse_or_invert_the_model_is_read_as_one_and_flagged() {
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let s = ModelImportSettings {
                scale: bad,
                ..Default::default()
            };
            assert_eq!(s.usable_scale(), (1.0, false), "scale {bad}");
        }
        for good in [0.01, 1.0, 2.5, 100.0] {
            let s = ModelImportSettings {
                scale: good,
                ..Default::default()
            };
            assert_eq!(s.usable_scale(), (good, true), "scale {good}");
        }
    }
}
