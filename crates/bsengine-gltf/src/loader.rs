use bsengine_rhi_wgpu::Vertex;

pub struct MeshData {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

pub struct GltfLoader;

impl GltfLoader {
    pub fn load(path: &str) -> Result<Vec<MeshData>, String> {
        let (doc, buffers, _) = gltf::import(path).map_err(|e| format!("gltf: {e}"))?;
        let mut result = Vec::new();
        for mesh in doc.meshes() {
            let name = mesh.name().unwrap_or("mesh").to_string();
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|b| Some(&buffers[b.index()]));

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

                let vertices: Vec<Vertex> = positions
                    .into_iter()
                    .zip(colors)
                    .zip(normals)
                    .map(|((position, color), normal)| Vertex {
                        position,
                        color,
                        normal,
                    })
                    .collect();

                result.push(MeshData {
                    name: name.clone(),
                    vertices,
                    indices,
                });
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_nonexistent_file_returns_error() {
        let result = GltfLoader::load("nonexistent.gltf");
        assert!(result.is_err());
    }
}
