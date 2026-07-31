use bevy_asset::io::Reader;
use bevy_asset::{AssetLoader, LoadContext};

use crate::loader::{GltfLoader, LoadedGltf};

/// Backs `LoadMode::Async` for glTF via `AssetServer::load`. Ignores the
/// byte `reader` entirely and re-runs `GltfLoader::load_full` against the
/// real filesystem path from `LoadContext::path()` — gltf-rs needs a real
/// path (not just bytes) to resolve sibling .bin/image files for non-.glb
/// assets, which `bevy_asset`'s byte-only `Reader` can't replicate without
/// reimplementing gltf-rs's own resolution logic.
#[derive(Default)]
pub struct GltfSourceLoader;

impl AssetLoader for GltfSourceLoader {
    type Asset = LoadedGltf;
    type Settings = ();
    type Error = String;

    async fn load<'a>(
        &'a self,
        _reader: &'a mut Reader<'_>,
        _settings: &'a Self::Settings,
        load_context: &'a mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let path = load_context
            .path()
            .to_str()
            .ok_or_else(|| "glTF asset path is not valid UTF-8".to_string())?;
        GltfLoader::load_full(path)
    }
}
