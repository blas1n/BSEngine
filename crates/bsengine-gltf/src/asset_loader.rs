use bevy_asset::io::{AsyncReadExt, Reader};
use bevy_asset::{AssetLoader, LoadContext};

use crate::loader::{GltfLoader, LoadedGltf};

/// Extension of a self-contained glTF: geometry, buffers and images in one
/// file, with nothing beside it to resolve.
const SELF_CONTAINED_EXTENSION: &str = "glb";

/// Backs `LoadMode::Async` for glTF via `AssetServer::load`.
///
/// # Two paths, and why both have to exist
///
/// A `.glb` is self-contained, so it is decoded from the bytes `reader` hands
/// over. That is the only thing that works in a packaged build, where the asset
/// lives inside a `.pak` and there is no filesystem path to open — and it is
/// why this loader stopped ignoring its reader.
///
/// Everything else falls back to re-reading the **filesystem path** from
/// `LoadContext::path()`. `gltf-rs` needs a real path to resolve a `.gltf`'s
/// sibling `.bin` and image files, which a byte-only `Reader` cannot replicate
/// without reimplementing that resolution. The consequence is stated rather than
/// hidden: **a `.gltf` with sibling files cannot be served from an archive**,
/// and `bsengine_asset::cook` refuses to pack one rather than letting it fail at
/// run time.
#[derive(Default)]
pub struct GltfSourceLoader;

impl AssetLoader for GltfSourceLoader {
    type Asset = LoadedGltf;
    type Settings = ();
    type Error = String;

    async fn load<'a>(
        &'a self,
        reader: &'a mut Reader<'_>,
        _settings: &'a Self::Settings,
        load_context: &'a mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let path = load_context
            .path()
            .to_str()
            .ok_or_else(|| "glTF asset path is not valid UTF-8".to_string())?
            .to_string();

        let self_contained = load_context
            .path()
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case(SELF_CONTAINED_EXTENSION));

        if self_contained {
            let mut bytes = Vec::new();
            reader
                .read_to_end(&mut bytes)
                .await
                .map_err(|e| format!("gltf: cannot read {path}: {e}"))?;
            return GltfLoader::load_full_from_slice(&bytes);
        }

        GltfLoader::load_full(&path)
    }
}
