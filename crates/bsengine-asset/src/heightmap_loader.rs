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

/// Encodes `data` (row-major, `width * height` long) as a 16-bit grayscale
/// PNG -- the inverse of `decode_heightmap_png`. Used by the terrain brush
/// tool to persist an edited heightmap back to the same file
/// `Terrain::heightmap_path` already points to.
pub fn encode_heightmap_png(width: u32, height: u32, data: &[u16]) -> Result<Vec<u8>, String> {
    if data.len() as u32 != width * height {
        return Err(format!(
            "encode: expected {} samples ({width}x{height}), got {}",
            width * height,
            data.len()
        ));
    }
    let img: image::ImageBuffer<image::Luma<u16>, Vec<u16>> =
        image::ImageBuffer::from_raw(width, height, data.to_vec())
            .ok_or_else(|| "encode: failed to build image buffer".to_string())?;
    let mut bytes = Vec::new();
    image::DynamicImage::ImageLuma16(img)
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(|e| format!("encode: {e}"))?;
    Ok(bytes)
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

    #[test]
    fn encode_then_decode_round_trips_exactly() {
        let width = 4;
        let height = 3;
        let values: Vec<u16> = (0..width * height).map(|i| (i as u16) * 1000).collect();

        let png_bytes =
            encode_heightmap_png(width, height, &values).expect("encode should succeed");
        let decoded = decode_heightmap_png(&png_bytes).expect("decode should succeed");

        assert_eq!(decoded.width, width);
        assert_eq!(decoded.height, height);
        assert_eq!(decoded.data, values);
    }

    #[test]
    fn encode_rejects_a_length_mismatch() {
        let err = encode_heightmap_png(4, 4, &[0u16; 3])
            .expect_err("width*height must match data.len()");
        assert!(
            err.contains("16") && err.contains("3"),
            "error should name both the expected and actual length: {err}"
        );
    }
}
