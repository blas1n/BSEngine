use bsengine_rhi_wgpu::Vertex;
use gltf::image::Format as GltfFormat;

pub struct MeshData {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

pub struct GltfImageData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct LoadedGltf {
    pub meshes: Vec<MeshData>,
    pub images: Vec<GltfImageData>,
    pub mesh_tex_indices: Vec<Option<usize>>,
}

pub struct GltfLoader;

impl GltfLoader {
    pub fn load(path: &str) -> Result<Vec<MeshData>, String> {
        Ok(Self::load_full(path)?.meshes)
    }

    pub fn load_full(path: &str) -> Result<LoadedGltf, String> {
        let (doc, buffers, raw_images) =
            gltf::import(path).map_err(|e| format!("gltf: {e}"))?;

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

                let indices: Vec<u32> = reader
                    .read_indices()
                    .ok_or("primitive has no indices")?
                    .into_u32()
                    .collect();

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
                });
                mesh_tex_indices.push(tex_idx);
            }
        }

        Ok(LoadedGltf {
            meshes,
            images,
            mesh_tex_indices,
        })
    }
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
}
