use bevy_asset::Asset;
use bevy_reflect::TypePath;

/// Decoded texture pixel data, ready to be uploaded to the GPU.
#[derive(Asset, TypePath)]
pub struct TextureAsset {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Raw pixel data, laid out row by row, RGBA8.
    pub data: Vec<u8>,
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
}
