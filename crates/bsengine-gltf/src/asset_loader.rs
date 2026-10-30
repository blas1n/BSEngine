use bevy_asset::io::Reader;
use bevy_asset::{AssetLoader, LoadContext};
use bsengine_asset::identity::sidecar::Sidecar;
use bsengine_core::ModelImportSettings;

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
// `TypePath`: Bevy 0.18 makes it a supertrait of `AssetLoader`.
#[derive(Default, bevy_reflect::TypePath)]
pub struct GltfSourceLoader {
    /// Where the default asset source reads from -- the project directory --
    /// for the one read that does not go through the source: a `.gltf` whose
    /// buffers and images are files beside it, which `gltf::import` opens
    /// itself. `None` in an app with no `bsengine_asset::AssetPlugin` (added
    /// before `GltfPlugin`), where the path is taken as the working
    /// directory's.
    pub root: Option<bsengine_asset::ProjectRoot>,
}

impl AssetLoader for GltfSourceLoader {
    type Asset = LoadedGltf;
    type Settings = ();
    type Error = String;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let path = load_context
            .path()
            .path()
            .to_str()
            .ok_or_else(|| "glTF asset path is not valid UTF-8".to_string())?
            .to_string();

        let self_contained = load_context
            .path()
            .path()
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case(SELF_CONTAINED_EXTENSION));

        // Read once, here, for both paths below: the `.glb` path has only
        // bytes and no sidecar to find beside them, and the `.gltf` path
        // would otherwise read the same sidecar a second time from `std::fs`
        // inside `load_full`.
        let settings = import_settings(load_context).await;

        if self_contained {
            let mut bytes = Vec::new();
            reader
                .read_to_end(&mut bytes)
                .await
                .map_err(|e| format!("gltf: cannot read {path}: {e}"))?;
            return GltfLoader::load_full_from_slice(&bytes, &settings);
        }

        // `path` is the asset's project path (`assets/x.gltf`), which is not
        // where the file is on disk: `gltf::import` opens it, and every
        // buffer and image beside it, with `std::fs`, so it is given the
        // project directory's copy. Before assets were loaded by project path
        // this was the engine form, a working-directory path, and the join
        // was not needed -- a `.gltf` with external files is what found it.
        let file = match &self.root {
            Some(root) => root.get().join(&path).to_string_lossy().into_owned(),
            None => path,
        };
        GltfLoader::load_full_with(&file, &settings)
    }
}

/// The import settings recorded in the sidecar beside the model being loaded,
/// or the defaults when there is no sidecar, it records none, or it records
/// another kind's -- said out loud, since `Texture(..)` beside a `.glb` is a
/// hand-edit whose every field is about to be ignored. The texture loader
/// has the mirror of this; the reading itself is shared in
/// [`Sidecar::read_beside_loading_asset`].
async fn import_settings(load_context: &mut LoadContext<'_>) -> ModelImportSettings {
    let Some(sidecar) = Sidecar::read_beside_loading_asset(load_context).await else {
        return ModelImportSettings::default();
    };
    if sidecar.import_is_another_kind("Model") {
        tracing::warn!(
            "model import: the sidecar beside {} records {} import settings, which a \
             model has no use for; loading it with the defaults",
            load_context.path().path().display(),
            sidecar.import.map(|i| i.kind()).unwrap_or_default()
        );
    }
    sidecar.model_import()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::{AssetServer, Assets};
    use bsengine_app::new_app;
    use bsengine_asset::identity::{measure_file, sidecar_path, AssetGuid, ImportSettings};

    /// The asset-server path -- the one the runtime and the editor load
    /// through -- reads the sidecar beside the model, for a `.glb`, whose
    /// bytes arrive with no path to look beside. Compared against a direct
    /// load of the committed fixture, which has no tuned sidecar, so the
    /// difference is the sidecar and nothing else.
    #[test]
    fn the_asset_server_loads_a_glb_with_the_settings_in_its_sidecar() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../games/mini-arena/assets/models/fox.glb");
        let reference =
            GltfLoader::load_full_with(fixture.to_str().unwrap(), &Default::default()).unwrap();
        let reference_x = reference.meshes[0].vertices[0].position[0];
        assert!(
            reference_x != 0.0 && !reference.animations.is_empty(),
            "premise: a scale shows on the first vertex, and there are clips to leave out"
        );

        let dir =
            std::env::temp_dir().join(format!("bsengine-gltf-sidecar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).ok();
            }
        }
        let _cleanup = Cleanup(dir.clone());
        let copy = dir.join("fox.glb");
        std::fs::copy(&fixture, &copy).unwrap();
        let (hash, size) = measure_file(&copy).unwrap();
        Sidecar {
            guid: AssetGuid::new(),
            hash,
            size: Some(size),
            former_paths: Vec::new(),
            import: Some(ImportSettings::Model(ModelImportSettings {
                scale: 2.0,
                import_animations: false,
            })),
        }
        .write(sidecar_path(&copy))
        .unwrap();

        let mut app = new_app();
        // The temp directory is the project, and the copy is `fox.glb` in it:
        // assets load by their project path.
        app.insert_resource(bsengine_core::ProjectDir(
            dir.to_string_lossy().into_owned(),
        ));
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(crate::GltfPlugin);
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load::<LoadedGltf>("fox.glb");
        let mut loaded = None;
        for _ in 0..200 {
            app.update();
            if let Some(g) = app.world().resource::<Assets<LoadedGltf>>().get(&handle) {
                loaded = Some((g.meshes[0].vertices[0].position[0], g.animations.len()));
                break;
            }
        }
        let (x, clips) = loaded.expect("the copy did not finish loading within 200 frames");
        assert!(
            (x - reference_x * 2.0).abs() <= 1e-4 * reference_x.abs(),
            "the sidecar's scale of 2.0 applies through the asset server: {x} vs {reference_x}"
        );
        assert_eq!(clips, 0, "and so does its animation toggle");
    }
}
