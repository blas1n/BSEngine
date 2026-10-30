use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;
use bevy_ecs::prelude::Resource;
use std::path::PathBuf;

use crate::types::{HeightmapAsset, TextureAsset};

/// The directory the engine form of a path is relative to -- the working
/// directory, read once, published so nothing else reads it again.
///
/// Components hold the engine form `bsengine_core::resolve_project_path`
/// makes (`<ProjectDir>/assets/x.glb`), and `notify` reports the files it
/// watches CWD-absolutised. Turning one into the other is a `strip_prefix` of
/// this directory joined to the watch root ([`crate::watcher`], and the
/// editor's prefab watcher). Two independent `current_dir()` calls agree only
/// because nothing moves the CWD between them, and if they ever disagreed the
/// symptom would be a watcher whose paths strip to nothing -- hot reload
/// silently doing nothing at all.
///
/// It is not where assets load from: that is the project ([`crate::project_root`]).
#[derive(Resource, Debug, Clone)]
pub struct AssetRoot(pub PathBuf);

/// [`AssetRoot`]: the working directory.
///
/// Once `bevy_asset`'s own root too -- given here as `file_path`, absolute so
/// that `FileAssetReader`'s `get_base_path().join(..)` dropped whatever base
/// it guessed (`BEVY_ASSET_ROOT`, `CARGO_MANIFEST_DIR` or the executable's
/// directory), a guess that under `cargo run` lost `games/mini-arena` its fox
/// mesh. The default source is now [`crate::project_root::ProjectAssetReader`]
/// (or a packaged build's archive), and `bevy_asset` builds no file source of
/// its own, so `file_path` is left at its default.
#[cfg(not(target_arch = "wasm32"))]
fn engine_form_root() -> PathBuf {
    std::env::current_dir()
        .expect("cannot read the working directory, which every engine-form path is relative to")
}

/// A browser build has no working directory, and needs none: its every asset
/// comes out of the archive the page fetched, and nothing is watched.
#[cfg(target_arch = "wasm32")]
fn engine_form_root() -> PathBuf {
    PathBuf::new()
}

/// Installs `bevy_asset`'s `AssetPlugin` and registers every content-asset
/// type this engine's loading systems use. Downstream crates (`bsengine-gltf`,
/// `bsengine-render`, `bsengine-audio`) register their own asset types from
/// their own plugins — this only owns the types defined in this crate.
pub struct AssetPlugin;

/// Serves a packaged build's assets out of its `.pak` instead of from files.
///
/// # It must be added *before* [`AssetPlugin`]
///
/// That is the whole reason this is a separate plugin rather than a field on
/// `AssetPlugin`. `bevy_asset::AssetPlugin::build` builds its sources there and
/// then, so a source registered afterwards is silently ignored; registering
/// here, from a plugin a host adds on the line above, puts the ordering
/// requirement where somebody reading the host can see it.
///
/// It also keeps `AssetPlugin` a unit struct. Around eighty places across eight
/// crates construct it — almost all of them tests that have no opinion about
/// packaging — and giving it a field would have edited every one of them to say
/// so.
///
/// Registering as the **default** source rather than a named one is what leaves
/// every `AssetServer::load` call site alone; see [`crate::pak_reader`].
pub struct PakAssetPlugin {
    /// The opened archive to serve.
    pub pak: std::sync::Arc<crate::pak::Pak>,
    /// The project directory as the engine spells it — the same string
    /// `bsengine_core::resolve_project_path` prepends to every asset path, and
    /// therefore the prefix the reader has to remove to get an archive key.
    pub project_dir: String,
}

impl Plugin for PakAssetPlugin {
    fn build(&self, app: &mut App) {
        let pak = self.pak.clone();
        let project_dir = self.project_dir.clone();
        app.register_asset_source(
            bevy_asset::io::AssetSourceId::Default,
            bevy_asset::io::AssetSourceBuilder::new(move || {
                crate::pak_reader::erased(pak.clone(), project_dir.clone())
            }),
        );
    }
}

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        // bevy_asset's AssetServer::load spawns its background load task on
        // the IoTaskPool. Upstream Bevy initializes this via
        // bevy_core::TaskPoolPlugin as part of DefaultPlugins/MinimalPlugins
        // — this workspace doesn't depend on bevy_core, so LoadMode::Async
        // (and this plugin's own AssetServer) would panic the first time
        // anything tried to load without this. get_or_init is idempotent,
        // so this is safe even if a future plugin also initializes it.
        bevy_tasks::IoTaskPool::get_or_init(bevy_tasks::TaskPool::default);

        // The default asset source is rooted at the *project*, and an asset is
        // loaded by its project path (`assets/x.glb`): see
        // [`crate::project_root`]. Registered here, before `bevy_asset`'s own
        // plugin builds its sources, and only if nothing (a packaged build's
        // archive) has taken the default already.
        //
        // The components still hold the engine form -- the resolved,
        // CWD-relative `<ProjectDir>/assets/x.glb` that
        // `bsengine_core::resolve_project_path` makes -- and
        // [`crate::project_root::asset_key`] converts it at the load.
        app.add_plugins(crate::project_root::ProjectRootPlugin);

        app.add_plugins(bevy_asset::AssetPlugin {
            // `file_path` stays at its default: it roots the file source
            // `bevy_asset` would build for the default, and the default is
            // taken above. (See `engine_form_root` for what it once was.)
            //
            // `bevy_asset` reserves `<asset>.meta` for its own
            // `AssetMetaMinimal`, and so does [`crate::identity`] for the
            // stable asset identity roadmap item 30 stores. Two formats, one
            // filename, and the collision is not a warning: with the default
            // `AssetMetaCheck::Always`, a sidecar beside `fox.glb` makes the
            // *asset* fail to load —
            //
            //   Failed to deserialize meta for asset .../fox.glb: Failed to
            //   deserialize minimal asset meta: MissingStructField { field:
            //   "asset", outer: Some("AssetMetaMinimal") }
            //
            // — so `games/mini-arena` loses its mesh and its glow shader on
            // the first launch that mints them, which is every launch after
            // `AssetIdentityPlugin` was registered in the hosts. `js` and
            // `ron` never reach `bevy_asset`, so the damage is confined to
            // exactly the extensions it does serve; that is most of
            // `identity::scan`'s list.
            //
            // `Never` is the resolution rather than a workaround because this
            // engine has no use for the thing being switched off. Bevy's meta
            // files carry per-asset loader settings and drive its
            // `AssetProcessor`; this workspace authors none, runs
            // `AssetMode::Unprocessed`, and already suppresses bevy's other
            // path conventions above for the same reason — engine paths are
            // resolved before they ever reach a loader. It also saves a
            // file-existence probe per load.
            //
            // The alternative, renaming `identity::SIDECAR_EXTENSION` away
            // from `meta`, would work too and costs more: `.meta` is what
            // Unity uses and what sub-item A specified, and every identity
            // already minted would be re-minted under the new name — which is
            // every GUID the scenes now store.
            meta_check: bevy_asset::AssetMetaCheck::Never,
            // `unapproved_path_mode` is left at Bevy's default, `Forbid`: a
            // path with a root, a prefix or a `..` that climbs out of the
            // source is refused before any reader sees it. This was `Allow`
            // while every asset was loaded by its engine form -- an absolute
            // path whenever a project was opened by one (the editor's file
            // dialog, every test's temp project) -- and the source was rooted
            // at the working directory, so `Forbid` refused every asset and
            // `Allow` refused nothing. Assets are now loaded by their project
            // path (see [`crate::project_root::asset_key`]), which never has
            // a root, so `Forbid` costs nothing and a path that does have
            // one -- the escape -- is what it stops.
            ..Default::default()
        })
        .insert_resource(AssetRoot(engine_form_root()))
        .init_asset::<TextureAsset>()
        .register_asset_loader(crate::texture_loader::TextureAssetLoader)
        .init_asset::<HeightmapAsset>()
        .register_asset_loader(crate::heightmap_loader::HeightmapAssetLoader);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::{AssetServer, Assets};
    use bsengine_app::new_app;

    /// Set on the re-executed half of
    /// [`an_asset_path_relative_to_the_process_cwd_loads`]. See that test for
    /// why it needs a second process at all.
    const CWD_PROBE_CHILD: &str = "BSENGINE_ASSET_CWD_PROBE_CHILD";

    /// Printed by the child half once the load has actually been asserted.
    /// Without checking for it, a filter that matched no test would exit 0 and
    /// the parent would report a pass having probed nothing.
    const CWD_PROBE_MARKER: &str = "bsengine-asset cwd probe: relative load ok";

    /// `--exact` name of [`an_asset_path_relative_to_the_process_cwd_loads`].
    /// Kept beside it so a rename that misses this fails loudly on the marker
    /// check above rather than silently passing.
    const CWD_PROBE_TEST: &str = "plugin::tests::an_asset_path_relative_to_the_process_cwd_loads";

    /// With no project directory the project is the working directory, and an
    /// asset loads by its path relative to it -- not relative to a base path
    /// `bevy_asset` guesses (`BEVY_ASSET_ROOT`, `CARGO_MANIFEST_DIR`, the
    /// executable's directory), which under `cargo run` once cost
    /// `games/mini-arena` its fox mesh.
    ///
    /// This has to re-execute itself to mean anything. Under `cargo test` the
    /// CWD *and* `CARGO_MANIFEST_DIR` are both the crate directory, so the
    /// broken root and the correct root are the same directory and no
    /// single-process test can tell them apart — which is precisely why that
    /// bug survived review and shipped. The child half runs with its CWD set
    /// to a fresh temp directory while still inheriting the parent's
    /// `CARGO_MANIFEST_DIR`, so the two roots finally differ and the load
    /// either finds the fixture (root == CWD) or does not (root == anything
    /// else).
    #[test]
    fn an_asset_path_relative_to_the_process_cwd_loads() {
        if std::env::var_os(CWD_PROBE_CHILD).is_some() {
            cwd_probe_child();
            return;
        }

        let dir = std::env::temp_dir().join(format!("bsengine_cwd_probe_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        image::RgbaImage::from_pixel(1, 1, image::Rgba([7, 8, 9, 255]))
            .save(dir.join("assets").join("probe.png"))
            .unwrap();

        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CWD_PROBE_TEST, "--nocapture", "--test-threads=1"])
            .env(CWD_PROBE_CHILD, "1")
            .current_dir(&dir)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            output.status.success(),
            "child probe failed ({}):\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
            output.status
        );
        assert!(
            stdout.contains(CWD_PROBE_MARKER),
            "child probe never ran the load — is {CWD_PROBE_TEST} still the right test name?\
             \n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        );
    }

    /// The half that runs in the re-executed process, with a CWD that is not
    /// bevy's base path.
    fn cwd_probe_child() {
        let cwd = std::env::current_dir().unwrap();
        let bevy_base = bevy_asset::io::file::FileAssetReader::get_base_path();
        // Guard against the probe quietly becoming vacuous: if these two ever
        // coincide, a broken root would pass this test.
        assert_ne!(
            bevy_base, cwd,
            "probe is vacuous — bevy's base path must differ from the CWD here"
        );

        let mut app = new_app();
        app.add_plugins(AssetPlugin);

        // Relative, forward slashes: the exact spelling resolve_project_path
        // produces.
        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<TextureAsset>("assets/probe.png")
        };

        let mut loaded = None;
        for _ in 0..200 {
            app.update();
            if let Some(tex) = app.world().resource::<Assets<TextureAsset>>().get(&handle) {
                loaded = Some((tex.width, tex.height, tex.data.clone()));
                break;
            }
        }

        let state = app
            .world()
            .resource::<AssetServer>()
            .get_load_state(handle.id());
        let (w, h, data) = loaded.unwrap_or_else(|| {
            panic!(
                "'assets/probe.png' did not load. cwd={cwd:?} bevy_base={bevy_base:?} \
                 project_root={:?} load_state={state:?}",
                app.world()
                    .resource::<crate::project_root::ProjectRoot>()
                    .get()
            )
        });
        assert_eq!((w, h), (1, 1));
        assert_eq!(data, vec![7, 8, 9, 255]);

        println!("{CWD_PROBE_MARKER}");
    }

    // `bevy_asset` and [`crate::identity`] both spell their per-asset
    // metadata `<asset>.meta`, in two incompatible formats. With bevy's
    // default `AssetMetaCheck::Always` the collision does not degrade the
    // load, it *fails* it: the asset never arrives, and the only trace is one
    // `ERROR` from `bevy_asset::server` about a struct field named `asset`,
    // which says nothing about identity sidecars to whoever reads it.
    //
    // The pairing is what makes this worth a test of its own. Neither half is
    // wrong alone — the scan writes a well-formed sidecar, the loader reads a
    // well-formed asset — and nothing in either crate's tests puts them in the
    // same directory, so the two shipped green and `games/mini-arena` lost its
    // fox mesh and its glow shader on the first launch that minted them.
    // Every extension `bevy_asset` serves is affected; `js` and `ron` are
    // spared only because they never reach it.
    #[test]
    fn an_asset_still_loads_with_an_identity_sidecar_beside_it() {
        use crate::test_support::{unique, ProbeDir};

        // Under the crate root rather than the temp directory: with no
        // `ProjectDir` the project is the CWD, and this has to be a path
        // inside it -- one the engine could really produce. `.gitignore` covers
        // `crates/*/bsengine-watch-probe-*`.
        let name = unique("identity-meta");
        let probe = ProbeDir(PathBuf::from(&name));
        std::fs::create_dir_all(probe.0.join("assets")).unwrap();
        image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]))
            .save(probe.0.join("assets/probe.png"))
            .unwrap();

        // The real scan, so the sidecar under test is the one that ships
        // rather than a hand-written approximation of it.
        crate::identity::scan(&probe.0).expect("scan the probe project");
        assert!(
            probe.0.join("assets/probe.png.meta").exists(),
            "the probe is vacuous unless the scan really wrote a sidecar"
        );

        let mut app = new_app();
        app.add_plugins(AssetPlugin);
        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<TextureAsset>(format!("{name}/assets/probe.png"))
        };

        let mut loaded = None;
        for _ in 0..200 {
            app.update();
            if let Some(tex) = app.world().resource::<Assets<TextureAsset>>().get(&handle) {
                loaded = Some(tex.data.clone());
                break;
            }
        }

        let state = app
            .world()
            .resource::<AssetServer>()
            .get_load_state(handle.id());
        assert_eq!(
            loaded,
            Some(vec![1, 2, 3, 255]),
            "an asset with an identity sidecar beside it must still load; load_state={state:?}"
        );
    }

    #[test]
    fn asset_plugin_registers_asset_server() {
        let mut app = new_app();
        app.add_plugins(AssetPlugin);
        assert!(app.world().get_resource::<AssetServer>().is_some());
    }

    #[test]
    fn asset_plugin_registers_texture_assets() {
        let mut app = new_app();
        app.add_plugins(AssetPlugin);
        assert!(app.world().get_resource::<Assets<TextureAsset>>().is_some());
    }
}
