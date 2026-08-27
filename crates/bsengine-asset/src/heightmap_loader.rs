//! Heightmap import: a 16-bit grayscale PNG decoded into a `HeightmapAsset`,
//! for terrain chunk generation. Mirrors `texture_loader.rs`'s
//! `TextureAssetLoader` structure for an 8-bit RGBA texture, except this
//! preserves full 16-bit height precision rather than downsampling to RGBA8.

use bevy_asset::io::Reader;
use bevy_asset::{AssetLoader, LoadContext};

use crate::types::HeightmapAsset;

/// Decodes PNG bytes into raw 16-bit height values. A non-16-bit-grayscale
/// source image is still accepted -- `image`'s `to_luma16()` converts any
/// decoded format (8-bit grayscale, RGB, etc.) into 16-bit grayscale, so a
/// lower-precision source just loses precision it never had rather than
/// failing to load.
pub(crate) fn decode_heightmap_png(bytes: &[u8]) -> Result<HeightmapAsset, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("decode: {e}"))?
        .to_luma16();
    let (width, height) = img.dimensions();
    Ok(HeightmapAsset {
        width,
        height,
        data: img.into_raw(),
    })
}

/// Backs `LoadMode::Async` for heightmaps via `AssetServer::load` --
/// see `load_mode.rs` for how `TextureAssetLoader` is wired the same way.
#[derive(Default)]
pub struct HeightmapAssetLoader;

impl AssetLoader for HeightmapAssetLoader {
    type Asset = HeightmapAsset;
    type Settings = ();
    type Error = String;

    async fn load<'a>(
        &'a self,
        reader: &'a mut Reader<'_>,
        _settings: &'a Self::Settings,
        _load_context: &'a mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        use bevy_asset::io::AsyncReadExt;
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(|e| format!("read: {e}"))?;
        decode_heightmap_png(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_png_16bit_gray(width: u32, height: u32, values: &[u16]) -> Vec<u8> {
        let img: image::ImageBuffer<image::Luma<u16>, Vec<u16>> =
            image::ImageBuffer::from_raw(width, height, values.to_vec())
                .expect("test fixture dimensions must match values.len()");
        let mut bytes = Vec::new();
        image::DynamicImage::ImageLuma16(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encoding the test fixture PNG failed");
        bytes
    }

    #[test]
    fn decodes_a_16_bit_grayscale_png_into_the_expected_height_values() {
        let values: Vec<u16> = vec![0, 32768, 65535, 16384];
        let bytes = make_test_png_16bit_gray(2, 2, &values);
        let decoded = decode_heightmap_png(&bytes).expect("decode failed");
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.data, values);
    }

    #[test]
    fn decode_fails_cleanly_on_non_image_bytes() {
        let err = decode_heightmap_png(b"not a png").unwrap_err();
        assert!(!err.is_empty());
    }
}
