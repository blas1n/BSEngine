use bevy_asset::io::Reader;
use bevy_asset::{AssetLoader, LoadContext};

use crate::types::TextureAsset;

/// Decodes a texture file (PNG/JPEG/HDR — matches this workspace's `image`
/// crate features) into a [`TextureAsset`]. Backs `LoadMode::Async` for
/// textures via `AssetServer::load`; `LoadMode::Sync` does not use this —
/// see `load_mode.rs`.
#[derive(Default)]
pub struct TextureAssetLoader;

impl AssetLoader for TextureAssetLoader {
    type Asset = TextureAsset;
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
        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("decode: {e}"))?
            .to_rgba8();
        let (width, height) = img.dimensions();
        Ok(TextureAsset {
            width,
            height,
            data: img.into_raw(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::{AssetServer, Assets};
    use bsengine_app::new_app;

    #[test]
    fn texture_asset_loads_async_and_becomes_available() {
        // A minimal valid 1x1 PNG, generated at test time rather than
        // checked in as a binary fixture.
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([10, 20, 30, 255]));
        let path = std::env::temp_dir().join("bsengine_test_texture.png");
        img.save(&path).unwrap();

        let mut app = new_app();
        app.add_plugins(crate::plugin::AssetPlugin);

        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<TextureAsset>(path.to_str().unwrap().to_owned())
        };

        // AssetServer.load is asynchronous — poll a bounded number of
        // updates until the background task finishes, rather than assuming
        // one app.update() is enough.
        let mut loaded = None;
        for _ in 0..200 {
            app.update();
            if let Some(tex) = app.world().resource::<Assets<TextureAsset>>().get(&handle) {
                loaded = Some((tex.width, tex.height, tex.data.clone()));
                break;
            }
        }
        let (w, h, data) = loaded.expect("texture did not finish loading within 200 frames");
        assert_eq!((w, h), (1, 1));
        assert_eq!(data, vec![10, 20, 30, 255]);
    }
}
