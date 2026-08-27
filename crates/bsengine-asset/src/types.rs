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

/// A decoded heightmap: 16-bit grayscale values, row-major, `width * height`
/// long. Full precision is kept (unlike [`TextureAsset`]'s RGBA8) because
/// terrain chunk generation needs the source PNG's full height resolution.
///
/// `Debug` (beyond `TextureAsset`'s derive list) is needed so
/// `decode_heightmap_png`'s `Result<HeightmapAsset, String>` satisfies
/// `unwrap_err`'s `T: Debug` bound in `heightmap_loader`'s own tests.
#[derive(Asset, TypePath, Debug)]
pub struct HeightmapAsset {
    /// Width in samples.
    pub width: u32,
    /// Height in samples.
    pub height: u32,
    /// Raw 16-bit height values, laid out row by row.
    pub data: Vec<u16>,
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
