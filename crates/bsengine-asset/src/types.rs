pub struct TextureAsset {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub struct MeshAsset {
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_asset_has_dimensions() {
        let tex = TextureAsset {
            width: 256,
            height: 256,
            data: vec![0u8; 256 * 256 * 4],
        };
        assert_eq!(tex.width, 256);
        assert_eq!(tex.data.len(), 256 * 256 * 4);
    }

    #[test]
    fn mesh_asset_has_vertices() {
        let mesh = MeshAsset {
            vertices: vec![0.0f32; 9],
            indices: vec![0u32, 1, 2],
        };
        assert_eq!(mesh.vertices.len(), 9);
        assert_eq!(mesh.indices.len(), 3);
    }
}
