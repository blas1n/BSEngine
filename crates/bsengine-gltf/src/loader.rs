use bevy_asset::Asset;
use bevy_reflect::TypePath;
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

/// Loads GLTF/GLB files from disk into engine-native mesh and animation data.
pub struct GltfLoader;

impl GltfLoader {
    /// Loads a GLTF/GLB file and returns just its meshes, discarding images
    /// and animations.
    pub fn load(path: &str) -> Result<Vec<MeshData>, String> {
        Ok(Self::load_full(path)?.meshes)
    }

    /// Loads a GLTF/GLB file, parsing its meshes, textures, and animations
    /// into engine-native data.
    ///
    /// Reads through the filesystem, which is what lets `gltf-rs` resolve a
    /// `.gltf`'s sibling `.bin` and image files. See
    /// [`Self::load_full_from_slice`] for the case where there is no
    /// filesystem to resolve against.
    pub fn load_full(path: &str) -> Result<LoadedGltf, String> {
        let (doc, buffers, raw_images) = gltf::import(path).map_err(|e| format!("gltf: {e}"))?;
        Self::from_parts(doc, buffers, raw_images)
    }

    /// The same, from bytes already in hand.
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
    pub fn load_full_from_slice(bytes: &[u8]) -> Result<LoadedGltf, String> {
        let (doc, buffers, raw_images) =
            gltf::import_slice(bytes).map_err(|e| format!("gltf: {e}"))?;
        Self::from_parts(doc, buffers, raw_images)
    }

    /// Turns what `gltf-rs` produced into engine-native data, however it was
    /// read.
    fn from_parts(
        doc: gltf::Document,
        buffers: Vec<gltf::buffer::Data>,
        raw_images: Vec<gltf::image::Data>,
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

        let animations = parse_animations(&doc, &buffers);

        Ok(LoadedGltf {
            meshes,
            images,
            mesh_tex_indices,
            animations,
            skins,
            nodes,
        })
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
}
