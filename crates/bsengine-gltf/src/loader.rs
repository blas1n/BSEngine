use bevy_asset::Asset;
use bevy_reflect::TypePath;
use bsengine_asset::identity::sidecar::{sidecar_path, Sidecar};
use bsengine_core::ModelImportSettings;
use bsengine_rhi_wgpu::Vertex;
use gltf::image::Format as GltfFormat;

use crate::animation::{AnimationChannel, AnimationClip, Interpolation, KeyframeValues};

/// A single mesh primitive extracted from a GLTF file, ready for GPU upload.
pub struct MeshData {
    /// The mesh's name, as given in the GLTF file (or a fallback if unnamed).
    pub name: String,
    /// Vertex buffer data (position, color, normal, uv).
    pub vertices: Vec<Vertex>,
    /// Index buffer data, referencing into `vertices`.
    pub indices: Vec<u32>,
    /// Per-vertex joint/weight skinning data, one entry per `vertices` entry, if this mesh's primitive had a skin.
    pub skin: Option<Vec<VertexSkin>>,
}

/// A decoded texture image, converted to raw RGBA8 pixel data.
pub struct GltfImageData {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Raw pixel data in RGBA8 order, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// Per-node local rest-pose transform plus its parent, as decomposed straight
/// from the glTF document — the "bind pose" a skinned mesh returns to for any
/// node/joint not overridden by the currently-sampled animation clip.
// Not `Copy`: `name` is a `String`. Nothing relied on the copy — every reader
// takes `&[NodeTransform]` and looks at fields through the reference.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeTransform {
    /// The node's name, as given in the glTF file, or empty when it has none.
    ///
    /// This is the bone name a skeleton is authored against — what
    /// `Ragdoll::joint_overrides` keys on to mark a knee or an elbow as a
    /// hinge. glTF node names are optional, so an unnamed node gets `""`
    /// rather than a synthesised placeholder: a made-up name would look
    /// authorable and silently never match.
    pub name: String,
    /// Local-space translation.
    pub position: [f32; 3],
    /// Local-space rotation quaternion, [x, y, z, w].
    pub rotation: [f32; 4],
    /// Local-space scale.
    pub scale: [f32; 3],
    /// Index of this node's parent in the same `nodes` list, or `None` for a root.
    pub parent: Option<usize>,
}

impl Default for NodeTransform {
    fn default() -> Self {
        Self {
            name: String::new(),
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            parent: None,
        }
    }
}

/// Joint data for one glTF skin: which nodes are joints (in "joint order",
/// matching vertex `JOINTS_0` indices) and each joint's inverse bind matrix
/// (column-major 4x4, as glTF stores it).
// `Default` (an empty skin: no joints, no inverse bind matrices) is what
// `SkinnedMesh::skin_data`'s `#[reflect(ignore)]` constructs when a
// `SkinnedMesh` is built reflectively — see that type's note on what is and
// is not reflected.
#[derive(Debug, Clone, Default)]
pub struct SkinData {
    /// Index (into `LoadedGltf::nodes`) of each joint, in joint order.
    pub joint_node_indices: Vec<usize>,
    /// One inverse bind matrix per joint, same order as `joint_node_indices`.
    pub inverse_bind_matrices: Vec<[[f32; 4]; 4]>,
}

/// Per-vertex skinning data: up to 4 joint indices (into a `SkinData`'s
/// `joint_node_indices`, i.e. 0..joint_count, NOT node indices) and their
/// blend weights, straight from glTF's `JOINTS_0`/`WEIGHTS_0` accessors.
#[derive(Debug, Clone, Copy, Default)]
pub struct VertexSkin {
    /// Up to 4 joint indices this vertex is bound to.
    pub joints: [u16; 4],
    /// Blend weight per joint in `joints`, same order, should sum to ~1.0.
    pub weights: [f32; 4],
}

/// The full result of loading a GLTF/GLB file: meshes, images, animations,
/// and the raw node/skin hierarchy.
#[derive(Asset, TypePath)]
pub struct LoadedGltf {
    /// All mesh primitives found in the file, in document order.
    pub meshes: Vec<MeshData>,
    /// All decoded images found in the file, in document order.
    pub images: Vec<GltfImageData>,
    /// For each entry in `meshes`, the index into `images` of its base color
    /// texture, if any.
    pub mesh_tex_indices: Vec<Option<usize>>,
    /// All animation clips found in the file.
    pub animations: Vec<AnimationClip>,
    /// Every node's local rest-pose transform and parent, indexed by glTF node index.
    pub nodes: Vec<NodeTransform>,
    /// Every skin defined in the file, in document order.
    pub skins: Vec<SkinData>,
}

impl LoadedGltf {
    /// Bakes a uniform scale into everything here that carries a length:
    /// vertex positions, every node's rest translation, the translation
    /// column of every inverse bind matrix, and every animation translation
    /// key. Rotations, node scales, normals and UVs are lengthless and stay.
    ///
    /// # Why all four, and why exactly these
    ///
    /// Skinning a vertex is `Σ w · G_j · B_j⁻¹ · v` -- the joint's animated
    /// global, the inverse of its bind global, the rest vertex. Scaling the
    /// model uniformly by `s` about the origin conjugates every node's local
    /// transform: `S·L·S⁻¹` keeps the rotation and scale and multiplies the
    /// translation by `s`, so the globals become `S·G·S⁻¹` and the bind
    /// inverses `S·B⁻¹·S⁻¹` -- again a translation column times `s`. With
    /// `v` itself times `s`, the sum is `S · (Σ w · G · B⁻¹ · v)`: exactly the
    /// scaled pose of the unscaled character. Leave any one of the four out
    /// and the skin drifts off its skeleton by the missing factor;
    /// `a_baked_scale_deforms_to_exactly_the_scaled_unscaled_pose` in
    /// `skinned_mesh.rs` fails on each such omission.
    ///
    /// Baked rather than kept as a root scale for every consumer to apply,
    /// because the consumers are many and read this data directly -- the CPU
    /// skinner, the ragdoll planner (bone lengths from node translations), the
    /// IK solver (goal distances), the nav bake (collider extents). This is
    /// also what Unity, Unreal and (with `apply_root_scale`, its default)
    /// Godot do.
    pub fn bake_scale(&mut self, scale: f32) {
        if scale == 1.0 {
            return;
        }
        for mesh in &mut self.meshes {
            for v in &mut mesh.vertices {
                for c in &mut v.position {
                    *c *= scale;
                }
            }
        }
        for node in &mut self.nodes {
            for c in &mut node.position {
                *c *= scale;
            }
        }
        for skin in &mut self.skins {
            for ibm in &mut skin.inverse_bind_matrices {
                // Column-major, as glTF stores it: column 3 is the translation.
                for c in &mut ibm[3][..3] {
                    *c *= scale;
                }
            }
        }
        for clip in &mut self.animations {
            for channel in &mut clip.channels {
                if let KeyframeValues::Translations(keys) = &mut channel.values {
                    // Under CubicSpline the keys are in-tangent / value /
                    // out-tangent triples; tangents are translation rates
                    // and scale the same way.
                    for key in keys {
                        for c in key {
                            *c *= scale;
                        }
                    }
                }
            }
        }
    }
}

/// Loads GLTF/GLB files from disk into engine-native mesh and animation data.
pub struct GltfLoader;

impl GltfLoader {
    /// Loads a GLTF/GLB file and returns just its meshes, discarding images
    /// and animations.
    pub fn load(path: &str) -> Result<Vec<MeshData>, String> {
        Ok(Self::load_full(path)?.meshes)
    }

    /// Loads a GLTF/GLB file, parsing its meshes, textures, and animations
    /// into engine-native data, with the import settings recorded in the
    /// `.meta` sidecar beside it -- or the defaults when there is none.
    ///
    /// Reads through the filesystem, which is what lets `gltf-rs` resolve a
    /// `.gltf`'s sibling `.bin` and image files, and is why the sidecar can
    /// be read from beside the file here at all. See
    /// [`Self::load_full_from_slice`] for the case where there is no
    /// filesystem to resolve against, and [`Self::load_full_with`] when the
    /// caller has already read the settings.
    pub fn load_full(path: &str) -> Result<LoadedGltf, String> {
        // Absent is the ordinary case and says nothing; a sidecar that exists
        // and will not read is worth a line, since its settings are ignored,
        // but not a failed load, since the model itself is fine.
        let settings = match Sidecar::read(sidecar_path(path)) {
            Ok(Some(sidecar)) => {
                if sidecar.import_is_another_kind("Model") {
                    tracing::warn!(
                        "model import: the sidecar beside {path} records {} import settings, \
                         which a model has no use for; loading it with the defaults",
                        sidecar.import.map(|i| i.kind()).unwrap_or_default()
                    );
                }
                sidecar.model_import()
            }
            Ok(None) => ModelImportSettings::default(),
            Err(e) => {
                tracing::warn!(
                    "model import: the sidecar beside {path} could not be read ({e}); \
                     loading it with the defaults"
                );
                ModelImportSettings::default()
            }
        };
        Self::load_full_with(path, &settings)
    }

    /// [`Self::load_full`] with the import settings given rather than read
    /// from beside the file.
    pub fn load_full_with(
        path: &str,
        settings: &ModelImportSettings,
    ) -> Result<LoadedGltf, String> {
        let (doc, buffers, raw_images) = gltf::import(path).map_err(|e| format!("gltf: {e}"))?;
        Self::from_parts(doc, buffers, raw_images, settings, path)
    }

    /// The same, from bytes already in hand -- and so with the import
    /// settings in hand too, since bytes have no sidecar beside them.
    ///
    /// # When this works, and when it cannot
    ///
    /// Only for a **self-contained** asset — a `.glb`, or a `.gltf` whose
    /// buffers and images are embedded as data URIs. A `.gltf` that references
    /// sibling files has nothing to resolve them against here, and `gltf-rs`
    /// will say so rather than silently returning a model with no geometry.
    ///
    /// This exists because a packaged build serves assets out of a `.pak`,
    /// where there is no path to hand to [`Self::load_full`] at all.
    pub fn load_full_from_slice(
        bytes: &[u8],
        settings: &ModelImportSettings,
    ) -> Result<LoadedGltf, String> {
        let (doc, buffers, raw_images) =
            gltf::import_slice(bytes).map_err(|e| format!("gltf: {e}"))?;
        Self::from_parts(doc, buffers, raw_images, settings, "<bytes>")
    }

    /// Turns what `gltf-rs` produced into engine-native data, however it was
    /// read, and applies the import settings to it. `what` names the source
    /// in the one warning this can raise.
    fn from_parts(
        doc: gltf::Document,
        buffers: Vec<gltf::buffer::Data>,
        raw_images: Vec<gltf::image::Data>,
        settings: &ModelImportSettings,
        what: &str,
    ) -> Result<LoadedGltf, String> {
        let images: Vec<GltfImageData> = raw_images
            .iter()
            .map(|img| {
                let rgba = gltf_pixels_to_rgba(&img.pixels, img.format, img.width, img.height);
                GltfImageData {
                    width: img.width,
                    height: img.height,
                    rgba,
                }
            })
            .collect();

        let nodes: Vec<NodeTransform> = {
            let mut out = vec![NodeTransform::default(); doc.nodes().count()];
            for node in doc.nodes() {
                let (t, r, s) = node.transform().decomposed();
                out[node.index()] = NodeTransform {
                    name: node.name().unwrap_or_default().to_string(),
                    position: t,
                    rotation: r,
                    scale: s,
                    parent: None,
                };
            }
            for node in doc.nodes() {
                for child in node.children() {
                    out[child.index()].parent = Some(node.index());
                }
            }
            out
        };

        let skins: Vec<SkinData> = doc
            .skins()
            .map(|skin| {
                let reader = skin.reader(|b| Some(&buffers[b.index()]));
                let joint_node_indices: Vec<usize> = skin.joints().map(|j| j.index()).collect();
                let inverse_bind_matrices: Vec<[[f32; 4]; 4]> = reader
                    .read_inverse_bind_matrices()
                    .map(|m| m.collect())
                    .unwrap_or_else(|| {
                        vec![glam::Mat4::IDENTITY.to_cols_array_2d(); joint_node_indices.len()]
                    });
                SkinData {
                    joint_node_indices,
                    inverse_bind_matrices,
                }
            })
            .collect();

        let mut meshes = Vec::new();
        let mut mesh_tex_indices = Vec::new();

        for mesh in doc.meshes() {
            let name = mesh.name().unwrap_or("mesh").to_string();
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|b| Some(&buffers[b.index()]));

                let tex_idx = primitive
                    .material()
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .map(|info| info.texture().source().index());

                let positions: Vec<[f32; 3]> = reader
                    .read_positions()
                    .ok_or("primitive has no positions")?
                    .collect();

                // Some valid glTF primitives omit the indices accessor entirely
                // (a flat, non-indexed triangle list -- legal per the glTF spec,
                // and used by real-world assets, e.g. Khronos's own Fox sample
                // model). Fall back to a sequential 0..N index buffer rather
                // than rejecting the whole file.
                let indices: Vec<u32> = match reader.read_indices() {
                    Some(indices) => indices.into_u32().collect(),
                    None => (0..positions.len() as u32).collect(),
                };

                let colors: Vec<[f32; 3]> = reader
                    .read_colors(0)
                    .map(|c| c.into_rgb_f32().collect())
                    .unwrap_or_else(|| vec![[0.8, 0.8, 0.8]; positions.len()]);

                let normals: Vec<[f32; 3]> = reader
                    .read_normals()
                    .map(|n| n.collect())
                    .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

                let uvs: Vec<[f32; 2]> = reader
                    .read_tex_coords(0)
                    .map(|t| t.into_f32().collect())
                    .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

                let skin: Option<Vec<VertexSkin>> = {
                    let joints_u16: Option<Vec<[u16; 4]>> =
                        reader.read_joints(0).map(|j| j.into_u16().collect());
                    let weights_f32: Option<Vec<[f32; 4]>> =
                        reader.read_weights(0).map(|w| w.into_f32().collect());
                    match (joints_u16, weights_f32) {
                        (Some(js), Some(ws)) => Some(
                            js.into_iter()
                                .zip(ws)
                                .map(|(joints, weights)| VertexSkin { joints, weights })
                                .collect(),
                        ),
                        _ => None,
                    }
                };

                let vertices: Vec<Vertex> = positions
                    .into_iter()
                    .zip(colors)
                    .zip(normals)
                    .zip(uvs)
                    .map(|(((position, color), normal), uv)| Vertex {
                        position,
                        color,
                        normal,
                        uv,
                    })
                    .collect();

                meshes.push(MeshData {
                    name: name.clone(),
                    vertices,
                    indices,
                    skin,
                });
                mesh_tex_indices.push(tex_idx);
            }
        }

        // Off means the clips never exist, not that they load and are hidden:
        // a skinned model then gets an empty clip library and holds its rest
        // pose, and nothing downstream can find a clip to play by accident.
        let animations = if settings.import_animations {
            parse_animations(&doc, &buffers)
        } else {
            Vec::new()
        };

        let mut loaded = LoadedGltf {
            meshes,
            images,
            mesh_tex_indices,
            animations,
            skins,
            nodes,
        };
        let (scale, usable) = settings.usable_scale();
        if !usable {
            tracing::warn!(
                "model import: {what} asks for a scale of {}, which would collapse or invert \
                 it; loading it at 1.0",
                settings.scale
            );
        }
        loaded.bake_scale(scale);
        Ok(loaded)
    }
}

fn parse_animations(doc: &gltf::Document, buffers: &[gltf::buffer::Data]) -> Vec<AnimationClip> {
    let mut clips = Vec::new();

    for anim in doc.animations() {
        let name = anim.name().unwrap_or("animation").to_string();
        let mut channels = Vec::new();
        let mut duration = 0.0f32;

        for channel in anim.channels() {
            let node_index = channel.target().node().index();
            let reader = channel.reader(|b| Some(&buffers[b.index()]));

            let times: Vec<f32> = match reader.read_inputs() {
                Some(inputs) => inputs.collect(),
                None => continue,
            };

            if let Some(&last) = times.last() {
                duration = duration.max(last);
            }

            let interpolation = match channel.sampler().interpolation() {
                gltf::animation::Interpolation::Linear => Interpolation::Linear,
                gltf::animation::Interpolation::Step => Interpolation::Step,
                gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
            };

            let values = match reader.read_outputs() {
                Some(gltf::animation::util::ReadOutputs::Translations(t)) => {
                    KeyframeValues::Translations(t.collect())
                }
                Some(gltf::animation::util::ReadOutputs::Rotations(r)) => {
                    KeyframeValues::Rotations(r.into_f32().collect())
                }
                Some(gltf::animation::util::ReadOutputs::Scales(s)) => {
                    KeyframeValues::Scales(s.collect())
                }
                _ => continue,
            };

            channels.push(AnimationChannel {
                node_index,
                times,
                values,
                interpolation,
            });
        }

        clips.push(AnimationClip {
            name,
            channels,
            duration,
        });
    }

    clips
}

fn gltf_pixels_to_rgba(pixels: &[u8], format: GltfFormat, width: u32, height: u32) -> Vec<u8> {
    match format {
        GltfFormat::R8G8B8A8 => pixels.to_vec(),
        GltfFormat::R8G8B8 => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for chunk in pixels.chunks(3) {
                out.extend_from_slice(chunk);
                out.push(255);
            }
            out
        }
        _ => vec![255u8; (width * height * 4) as usize],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_nonexistent_file_returns_error() {
        assert!(GltfLoader::load("nonexistent.gltf").is_err());
    }

    #[test]
    fn load_full_nonexistent_returns_error() {
        assert!(GltfLoader::load_full("nonexistent.gltf").is_err());
    }

    #[test]
    fn load_full_result_has_animations_field() {
        let result = GltfLoader::load_full("nonexistent.gltf");
        assert!(result.is_err());
        // Verify LoadedGltf struct has the animations field by constructing one
        let loaded = LoadedGltf {
            meshes: vec![],
            images: vec![],
            mesh_tex_indices: vec![],
            animations: vec![],
            skins: vec![],
            nodes: vec![],
        };
        assert_eq!(loaded.animations.len(), 0);
    }

    #[test]
    fn skin_joint_and_node_data_default_empty_for_unskinned_asset() {
        let result = GltfLoader::load_full("nonexistent.gltf");
        assert!(result.is_err());
        let loaded = LoadedGltf {
            meshes: vec![],
            images: vec![],
            mesh_tex_indices: vec![],
            animations: vec![],
            skins: vec![],
            nodes: vec![],
        };
        assert!(loaded.skins.is_empty());
        assert!(loaded.nodes.is_empty());
    }

    #[test]
    fn node_transform_decomposes_identity_by_default() {
        let n = NodeTransform::default();
        assert_eq!(n.position, [0.0, 0.0, 0.0]);
        assert_eq!(n.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(n.scale, [1.0, 1.0, 1.0]);
        assert_eq!(n.parent, None);
    }

    #[test]
    fn gltf_pixels_rgb_to_rgba_adds_alpha() {
        let rgb = vec![255u8, 0, 0, 0, 255, 0];
        let out = gltf_pixels_to_rgba(&rgb, GltfFormat::R8G8B8, 2, 1);
        assert_eq!(out, vec![255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn gltf_pixels_rgba_passthrough() {
        let rgba = vec![1u8, 2, 3, 4];
        let out = gltf_pixels_to_rgba(&rgba, GltfFormat::R8G8B8A8, 1, 1);
        assert_eq!(out, rgba);
    }

    #[test]
    fn loaded_gltf_is_an_asset() {
        fn assert_asset<T: bevy_asset::Asset>() {}
        assert_asset::<LoadedGltf>();
    }

    fn fox() -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../games/mini-arena/assets/models/fox.glb")
            .to_str()
            .unwrap()
            .to_owned()
    }

    fn fox_with(settings: ModelImportSettings) -> LoadedGltf {
        GltfLoader::load_full_with(&fox(), &settings).expect("fox.glb loads")
    }

    fn scaled(scale: f32) -> ModelImportSettings {
        ModelImportSettings {
            scale,
            ..Default::default()
        }
    }

    /// Every translation key of every clip, flattened -- what the bake must
    /// scale and the rotation/scale keys beside it must not.
    fn translation_keys(g: &LoadedGltf) -> Vec<[f32; 3]> {
        g.animations
            .iter()
            .flat_map(|c| &c.channels)
            .filter_map(|ch| match &ch.values {
                KeyframeValues::Translations(t) => Some(t.iter().copied()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    fn close(scaled: f32, expected: f32) -> bool {
        (scaled - expected).abs() <= 1e-4 * expected.abs().max(1.0)
    }

    /// Every length the file carries is scaled, and nothing lengthless is --
    /// listed one by one. The skinning identity in `skinned_mesh.rs` says only
    /// that the *composition* is right; a bake that scaled the wrong things
    /// consistently (a bind matrix's whole linear part along with its
    /// translation, say) could pass it while breaking every consumer that
    /// reads one part alone, such as the ragdoll planner reading node
    /// translations.
    #[test]
    fn a_baked_scale_reaches_every_length_the_file_carries_and_nothing_else() {
        const S: f32 = 2.5;
        let one = fox_with(scaled(1.0));
        let two = fox_with(scaled(S));

        // Premises: the fixture has each of the four things, non-trivially.
        // A file whose nodes all sat at the origin would pass a bake that
        // skipped them.
        assert!(
            !one.skins.is_empty() && !one.animations.is_empty(),
            "premise: fox.glb is skinned and animated"
        );
        assert!(
            one.nodes.iter().any(|n| n.position != [0.0; 3]),
            "premise: some node sits away from its parent"
        );
        assert!(
            one.skins[0]
                .inverse_bind_matrices
                .iter()
                .any(|m| m[3][..3] != [0.0; 3]),
            "premise: some inverse bind matrix has a translation"
        );
        assert!(
            translation_keys(&one).iter().any(|k| *k != [0.0; 3]),
            "premise: some translation key is non-zero"
        );

        assert_eq!(one.meshes.len(), two.meshes.len());
        for (a, b) in one.meshes.iter().zip(&two.meshes) {
            assert_eq!(a.indices, b.indices, "indices are not lengths");
            for (va, vb) in a.vertices.iter().zip(&b.vertices) {
                for i in 0..3 {
                    assert!(
                        close(vb.position[i], va.position[i] * S),
                        "vertex position {:?} should scale to {:?}, got {:?}",
                        va.position,
                        va.position.map(|c| c * S),
                        vb.position
                    );
                }
                assert_eq!(va.normal, vb.normal, "normals are directions");
                assert_eq!(va.uv, vb.uv, "uvs are not lengths");
                assert_eq!(va.color, vb.color);
            }
        }

        assert_eq!(one.nodes.len(), two.nodes.len());
        for (a, b) in one.nodes.iter().zip(&two.nodes) {
            for i in 0..3 {
                assert!(
                    close(b.position[i], a.position[i] * S),
                    "node {:?} translation {:?} should scale, got {:?}",
                    a.name,
                    a.position,
                    b.position
                );
            }
            assert_eq!(a.rotation, b.rotation, "node rotations stay");
            assert_eq!(a.scale, b.scale, "node scales stay");
            assert_eq!(a.parent, b.parent);
        }

        assert_eq!(one.skins.len(), two.skins.len());
        for (a, b) in one.skins.iter().zip(&two.skins) {
            assert_eq!(a.joint_node_indices, b.joint_node_indices);
            for (ma, mb) in a.inverse_bind_matrices.iter().zip(&b.inverse_bind_matrices) {
                for col in 0..3 {
                    assert_eq!(ma[col], mb[col], "the bind matrix's linear part stays");
                }
                for row in 0..3 {
                    assert!(
                        close(mb[3][row], ma[3][row] * S),
                        "bind translation {:?} should scale, got {:?}",
                        &ma[3][..3],
                        &mb[3][..3]
                    );
                }
                assert_eq!(ma[3][3], mb[3][3]);
            }
        }

        assert_eq!(one.animations.len(), two.animations.len());
        for (a, b) in one.animations.iter().zip(&two.animations) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.duration, b.duration, "time is not a length");
            assert_eq!(a.channels.len(), b.channels.len());
            for (ca, cb) in a.channels.iter().zip(&b.channels) {
                assert_eq!(ca.times, cb.times);
                assert_eq!(ca.node_index, cb.node_index);
                match (&ca.values, &cb.values) {
                    (KeyframeValues::Translations(x), KeyframeValues::Translations(y)) => {
                        for (ka, kb) in x.iter().zip(y) {
                            for i in 0..3 {
                                assert!(
                                    close(kb[i], ka[i] * S),
                                    "translation key {ka:?} should scale, got {kb:?}"
                                );
                            }
                        }
                    }
                    (KeyframeValues::Rotations(x), KeyframeValues::Rotations(y)) => {
                        assert_eq!(x, y, "rotation keys stay")
                    }
                    (KeyframeValues::Scales(x), KeyframeValues::Scales(y)) => {
                        assert_eq!(x, y, "scale keys stay")
                    }
                    _ => panic!("channel kinds differ between the two loads"),
                }
            }
        }
    }

    /// Off means no clips at all, with the geometry and skeleton untouched:
    /// what makes a skinned model hold its rest pose is that there is
    /// nothing for the player to find.
    #[test]
    fn import_animations_off_loads_the_geometry_and_skeleton_without_the_clips() {
        let with = fox_with(ModelImportSettings::default());
        let without = fox_with(ModelImportSettings {
            import_animations: false,
            ..Default::default()
        });
        assert!(
            !with.animations.is_empty(),
            "premise: fox.glb has clips to leave out"
        );
        assert!(without.animations.is_empty(), "no clips were imported");
        assert_eq!(with.meshes.len(), without.meshes.len());
        assert_eq!(with.nodes, without.nodes);
        assert_eq!(
            with.skins[0].joint_node_indices,
            without.skins[0].joint_node_indices
        );
    }

    /// A copy of the fixture in a directory of its own, so a sidecar can be
    /// put beside it without touching the committed one.
    struct FoxCopy {
        dir: std::path::PathBuf,
        path: String,
    }

    impl FoxCopy {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "bsengine-model-import-{tag}-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let copy = dir.join("fox.glb");
            std::fs::copy(fox(), &copy).unwrap();
            Self {
                dir,
                path: copy.to_str().unwrap().to_owned(),
            }
        }

        fn with_sidecar(self, import: Option<bsengine_asset::identity::ImportSettings>) -> Self {
            use bsengine_asset::identity::{measure_file, AssetGuid};
            let (hash, size) = measure_file(&self.path).unwrap();
            Sidecar {
                guid: AssetGuid::new(),
                hash,
                size: Some(size),
                former_paths: Vec::new(),
                import,
            }
            .write(sidecar_path(&self.path))
            .unwrap();
            self
        }
    }

    impl Drop for FoxCopy {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    fn first_vertex_x(g: &LoadedGltf) -> f32 {
        g.meshes[0].vertices[0].position[0]
    }

    /// The synchronous entry point reads the sidecar beside the file itself,
    /// so a direct load and an `AssetServer` load of the same file come out
    /// the same size. Three sidecars: a model's settings, which apply; none,
    /// which is the defaults; and a texture's, which is the defaults too --
    /// the model is fine and its sidecar is somebody's hand-edit.
    #[test]
    fn load_full_reads_the_sidecar_beside_the_file() {
        use bsengine_asset::identity::ImportSettings;
        let reference = fox_with(ModelImportSettings::default());
        assert!(
            first_vertex_x(&reference) != 0.0,
            "premise: the first vertex is off the x = 0 plane, so a scale shows"
        );

        let tuned =
            FoxCopy::new("tuned").with_sidecar(Some(ImportSettings::Model(ModelImportSettings {
                scale: 2.0,
                import_animations: false,
            })));
        let loaded = GltfLoader::load_full(&tuned.path).unwrap();
        assert!(
            close(first_vertex_x(&loaded), first_vertex_x(&reference) * 2.0),
            "the sidecar's scale applies: {} vs {}",
            first_vertex_x(&loaded),
            first_vertex_x(&reference)
        );
        assert!(
            loaded.animations.is_empty(),
            "and so does its animation toggle"
        );

        let bare = FoxCopy::new("bare");
        let loaded = GltfLoader::load_full(&bare.path).unwrap();
        assert_eq!(first_vertex_x(&loaded), first_vertex_x(&reference));
        assert_eq!(loaded.animations.len(), reference.animations.len());

        let wrong_kind = FoxCopy::new("wrong-kind").with_sidecar(Some(ImportSettings::Texture(
            bsengine_core::TextureImportSettings::default(),
        )));
        let loaded = GltfLoader::load_full(&wrong_kind.path).unwrap();
        assert_eq!(
            first_vertex_x(&loaded),
            first_vertex_x(&reference),
            "a texture's settings beside a model are the defaults"
        );
    }

    /// A zero scale would collapse the model to a point and a negative one
    /// turn it inside out; both load as authored rather than as nothing.
    #[test]
    fn an_unusable_scale_loads_the_model_as_authored() {
        let reference = fox_with(ModelImportSettings::default());
        for bad in [0.0, -2.0, f32::NAN] {
            let loaded = fox_with(scaled(bad));
            assert_eq!(
                first_vertex_x(&loaded),
                first_vertex_x(&reference),
                "scale {bad} must be read as 1.0"
            );
        }
    }
}
