//! Minimal glTF parsing + a small dedicated render pipeline for the Asset
//! Browser's mesh thumbnails. Deliberately does not use `bsengine-gltf`'s
//! full loader: `bsengine-gltf` depends on this crate (for `Vertex`), so
//! calling into it from here would be a dependency cycle. This module only
//! extracts what a thumbnail needs -- base color + geometry -- using the
//! raw `gltf` crate directly. See
//! `docs/superpowers/specs/2026-08-27-mesh-3d-thumbnails-design.md`.

use crate::mesh::Vertex;
use std::path::Path;

/// A decoded RGBA8 image, already resolved from whatever format glTF stored
/// it in (matches `bsengine-gltf`'s own `GltfImageData` shape, duplicated
/// rather than imported for the same reason the whole module exists).
pub struct DecodedImage {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Raw pixel data in RGBA8 order, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// One primitive's geometry plus its base color: a decoded texture when the
/// primitive's material has one, or just the flat `base_color_factor`
/// otherwise.
pub struct ThumbnailPrimitive {
    /// Vertex buffer data (position, color, normal, uv).
    pub vertices: Vec<Vertex>,
    /// Index buffer data, referencing into `vertices`.
    pub indices: Vec<u32>,
    /// The material's flat base color, RGBA in 0..=1. Used directly when
    /// `base_color_texture` is `None`, and as a tint on top of it otherwise.
    pub base_color_factor: [f32; 4],
    /// The material's base color texture, decoded to RGBA8, if it has one.
    pub base_color_texture: Option<DecodedImage>,
}

/// Every primitive belonging to a glTF document's first mesh, ready to
/// render as a thumbnail. Only the first `mesh` entry in the document is
/// used -- see the design doc's "Out of scope" section.
pub struct ThumbnailMesh {
    /// This mesh's primitives, each with its own geometry and material.
    pub primitives: Vec<ThumbnailPrimitive>,
}

/// Loads `path` and extracts its first mesh's geometry + base color
/// material for thumbnail rendering. `None` on any failure: the file
/// doesn't parse, or the document has no meshes, or a primitive is missing
/// position data. Skinning, animation, and every non-base-color material
/// slot are ignored -- see the design doc.
pub fn load_thumbnail_mesh(path: &Path) -> Option<ThumbnailMesh> {
    let (doc, buffers, images) = gltf::import(path).ok()?;
    let mesh = doc.meshes().next()?;

    let mut primitives = Vec::new();
    for primitive in mesh.primitives() {
        let reader = primitive.reader(|b| Some(&buffers[b.index()]));
        let positions: Vec<[f32; 3]> = reader.read_positions()?.collect();
        let indices: Vec<u32> = match reader.read_indices() {
            Some(indices) => indices.into_u32().collect(),
            None => (0..positions.len() as u32).collect(),
        };
        let normals: Vec<[f32; 3]> = reader
            .read_normals()
            .map(|n| n.collect())
            .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
        let uvs: Vec<[f32; 2]> = reader
            .read_tex_coords(0)
            .map(|t| t.into_f32().collect())
            .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

        let vertices: Vec<Vertex> = positions
            .into_iter()
            .zip(normals)
            .zip(uvs)
            .map(|((position, normal), uv)| Vertex {
                position,
                color: [1.0, 1.0, 1.0],
                normal,
                uv,
            })
            .collect();

        let pbr = primitive.material().pbr_metallic_roughness();
        let base_color_factor = pbr.base_color_factor();
        let base_color_texture = pbr.base_color_texture().and_then(|info| {
            let image = images.get(info.texture().source().index())?;
            Some(DecodedImage {
                width: image.width,
                height: image.height,
                rgba: gltf_pixels_to_rgba(&image.pixels, image.format, image.width, image.height),
            })
        });

        primitives.push(ThumbnailPrimitive {
            vertices,
            indices,
            base_color_factor,
            base_color_texture,
        });
    }

    if primitives.is_empty() {
        return None;
    }
    Some(ThumbnailMesh { primitives })
}

/// Mirrors `bsengine-gltf`'s private helper of the same shape (`loader.rs`'s
/// `gltf_pixels_to_rgba`) -- duplicated rather than shared, since sharing it
/// would mean depending on `bsengine-gltf`, which is exactly the cycle this
/// module exists to avoid.
fn gltf_pixels_to_rgba(
    pixels: &[u8],
    format: gltf::image::Format,
    width: u32,
    height: u32,
) -> Vec<u8> {
    match format {
        gltf::image::Format::R8G8B8A8 => pixels.to_vec(),
        gltf::image::Format::R8G8B8 => {
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
    use std::path::PathBuf;

    /// Writes a minimal, valid `.gltf` (JSON, not binary `.glb`) file
    /// containing exactly one mesh with one triangle primitive, using
    /// `material_index` as its `material` reference. The vertex buffer is
    /// built and base64-encoded at test-run time rather than hand-typed, to
    /// avoid an unverifiable hand-computed base64 string.
    fn write_test_gltf_triangle(path: &Path, base_color_factor: [f32; 4]) {
        let positions: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let indices: [u32; 3] = [0, 1, 2];

        let mut buf = Vec::new();
        for p in &positions {
            buf.extend_from_slice(&p[0].to_le_bytes());
            buf.extend_from_slice(&p[1].to_le_bytes());
            buf.extend_from_slice(&p[2].to_le_bytes());
        }
        let position_bytes = buf.len();
        for i in &indices {
            buf.extend_from_slice(&i.to_le_bytes());
        }
        let index_bytes = buf.len() - position_bytes;

        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&buf);
        let [r, g, b, a] = base_color_factor;

        let json = format!(
            r#"{{
  "asset": {{"version": "2.0"}},
  "scenes": [{{"nodes": [0]}}],
  "nodes": [{{"mesh": 0}}],
  "meshes": [{{
    "primitives": [{{
      "attributes": {{"POSITION": 0}},
      "indices": 1,
      "material": 0
    }}]
  }}],
  "materials": [{{
    "pbrMetallicRoughness": {{"baseColorFactor": [{r}, {g}, {b}, {a}]}}
  }}],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0]}},
    {{"bufferView": 1, "componentType": 5125, "count": 3, "type": "SCALAR"}}
  ],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": {position_bytes}, "target": 34962}},
    {{"buffer": 0, "byteOffset": {position_bytes}, "byteLength": {index_bytes}, "target": 34963}}
  ],
  "buffers": [
    {{"byteLength": {total}, "uri": "data:application/octet-stream;base64,{encoded}"}}
  ]
}}"#,
            total = position_bytes + index_bytes,
        );
        std::fs::write(path, json).unwrap();
    }

    #[test]
    fn load_thumbnail_mesh_parses_a_minimal_triangle() {
        let tmp = std::env::temp_dir().join("bse_mesh_thumb_test_triangle.gltf");
        write_test_gltf_triangle(&tmp, [1.0, 0.0, 0.0, 1.0]);

        let mesh = load_thumbnail_mesh(&tmp).expect("a valid minimal glTF should parse");
        assert_eq!(mesh.primitives.len(), 1);
        let prim = &mesh.primitives[0];
        assert_eq!(prim.vertices.len(), 3);
        assert_eq!(prim.indices, vec![0, 1, 2]);
        assert_eq!(prim.base_color_factor, [1.0, 0.0, 0.0, 1.0]);
        assert!(
            prim.base_color_texture.is_none(),
            "this fixture has no texture, only a flat factor"
        );

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_thumbnail_mesh_fills_in_default_normals_and_uvs_when_absent() {
        let tmp = std::env::temp_dir().join("bse_mesh_thumb_test_defaults.gltf");
        write_test_gltf_triangle(&tmp, [0.5, 0.5, 0.5, 1.0]);

        let mesh = load_thumbnail_mesh(&tmp).unwrap();
        let prim = &mesh.primitives[0];
        for v in &prim.vertices {
            assert_eq!(v.normal, [0.0, 1.0, 0.0], "missing NORMAL accessor should default to up");
            assert_eq!(v.uv, [0.0, 0.0], "missing TEXCOORD_0 accessor should default to origin");
        }

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_thumbnail_mesh_returns_none_for_a_nonexistent_file() {
        assert!(load_thumbnail_mesh(Path::new("definitely_does_not_exist.gltf")).is_none());
    }

    #[test]
    fn load_thumbnail_mesh_returns_none_for_a_document_with_no_meshes() {
        let tmp = std::env::temp_dir().join("bse_mesh_thumb_test_no_meshes.gltf");
        std::fs::write(&tmp, r#"{"asset": {"version": "2.0"}}"#).unwrap();

        assert!(load_thumbnail_mesh(&tmp).is_none());

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_thumbnail_mesh_parses_the_real_fox_fixture() {
        let path = PathBuf::from(format!(
            "{}/../../games/mini-arena/assets/models/fox.glb",
            env!("CARGO_MANIFEST_DIR")
        ));
        let mesh = load_thumbnail_mesh(&path)
            .expect("the real fox.glb fixture used elsewhere in this workspace should parse");
        assert!(
            !mesh.primitives.is_empty(),
            "fox.glb should have at least one primitive"
        );
        assert!(
            mesh.primitives.iter().any(|p| !p.vertices.is_empty()),
            "at least one primitive should have real geometry"
        );
    }
}
