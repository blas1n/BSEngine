use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;

use crate::types::TextureAsset;

/// The directory `bevy_asset` resolves every asset path against.
///
/// # Why this is not just `""`
///
/// `AssetPlugin::file_path` is *not* the asset root — it is a path joined
/// **under** a root `bevy_asset` picks by itself. In `bevy_asset-0.14.2` the
/// chain is `AssetPlugin::build` -> `init_default_source(&self.file_path, ..)`
/// -> `AssetSourceBuilder::platform_default` -> `FileAssetReader::new(path)`,
/// and that last one is
///
/// ```ignore
/// let root_path = Self::get_base_path().join(path.as_ref());
/// ```
///
/// where `get_base_path()` is `BEVY_ASSET_ROOT`, else `CARGO_MANIFEST_DIR`,
/// else the executable's own directory. So `file_path: ""` does not mean "no
/// root" — it means "the root bevy guessed, unchanged". Under `cargo run` that
/// guess is `CARGO_MANIFEST_DIR`: the *package* directory of the binary being
/// run, e.g. `<repo>/crates/bsengine-runtime`.
///
/// That silently broke every asset load. Paths reaching `AssetServer::load`
/// come from `bsengine_core::resolve_project_path`, which joins `ProjectDir`
/// with a scene-relative path and is **relative to the process CWD** — e.g.
/// `games/mini-arena/assets/models/fox.glb`. Resolved under
/// `<repo>/crates/bsengine-runtime` instead of the CWD, that file simply is not
/// there, and a failed load is only a `WARN`, so `games/mini-arena` ran without
/// its fox mesh and without its glow shader for as long as the mistake stood.
///
/// Returning the **absolute** CWD fixes it by making bevy's guess irrelevant:
/// `Path::join` with an absolute path discards the base entirely, so whatever
/// `get_base_path()` returned is replaced by the CWD.
///
/// The CWD is the right root because it is already the root every *other* path
/// in the engine is relative to — `bsengine-runtime` reads
/// `<project_dir>/project.toml` with plain `std::fs`, scenes and scripts load
/// the same way. This only makes `bevy_asset` agree with them.
///
/// Read once, at plugin build time. A later `set_current_dir` would not move
/// the asset root — which is the behaviour we want, since it would not move
/// the engine's other CWD-relative reads either.
///
/// Deliberately, this also stops `BEVY_ASSET_ROOT` from having any effect.
/// Honouring it would mean `bevy_asset` resolving from one directory while
/// every `std::fs` read in the engine resolved from another — i.e. an opt-in
/// switch for exactly the split-brain being fixed here. Point the CWD at the
/// project instead; that moves all of it at once.
fn asset_source_root() -> String {
    std::env::current_dir()
        .expect("cannot read the working directory, which every engine asset path is relative to")
        .into_os_string()
        .into_string()
        .expect("the working directory is not valid UTF-8, which bevy_asset's file_path requires")
}

/// Installs `bevy_asset`'s `AssetPlugin` and registers every content-asset
/// type this engine's loading systems use. Downstream crates (`bsengine-gltf`,
/// `bsengine-render`, `bsengine-audio`) register their own asset types from
/// their own plugins — this only owns the types defined in this crate.
pub struct AssetPlugin;

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

        // Every asset path in this engine (scene RON `gltf:` fields,
        // Bsengine.setShader() paths, playSound() paths, ...) is already
        // fully resolved via bsengine_core::resolve_project_path before it
        // reaches any loader — it is a CWD-relative filesystem path, not a
        // path meant to be joined under bevy_asset's own "assets/" root
        // convention. Passing the absolute CWD as `file_path` both suppresses
        // that convention and pins the root to the CWD; see
        // [`asset_source_root`] for why the obvious-looking `""` does neither.
        app.add_plugins(bevy_asset::AssetPlugin {
            file_path: asset_source_root(),
            ..Default::default()
        })
        .init_asset::<TextureAsset>()
        .register_asset_loader(crate::texture_loader::TextureAssetLoader);
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

    #[test]
    fn asset_source_root_replaces_whatever_base_path_bevy_picked() {
        // The one property the fix rests on, stated as bevy applies it:
        // FileAssetReader::new does `get_base_path().join(file_path)`, and
        // `get_base_path()` may be any of BEVY_ASSET_ROOT, CARGO_MANIFEST_DIR
        // or the exe's directory. Joining an absolute path throws all three
        // away; joining `""` (the previous value) keeps whichever one it was.
        let cwd = std::env::current_dir().unwrap();
        let root = asset_source_root();

        for guess in [
            // What CARGO_MANIFEST_DIR gave under `cargo run`, and roughly what
            // a built binary's own directory would give.
            cwd.join("crates").join("bsengine-runtime"),
            cwd.join("target").join("debug"),
        ] {
            assert_eq!(
                guess.join(&root),
                cwd,
                "joining the asset root under bevy's own base path must yield the CWD"
            );
        }
    }

    /// Loads an asset by a path spelled **relative to the process CWD**, which
    /// is what every path out of `bsengine_core::resolve_project_path` is.
    ///
    /// This has to re-execute itself to mean anything. Under `cargo test` the
    /// CWD *and* `CARGO_MANIFEST_DIR` are both the crate directory, so the
    /// broken root and the correct root are the same directory and no
    /// single-process test can tell them apart — which is precisely why this
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
                 asset_root={:?} load_state={state:?}",
                asset_source_root()
            )
        });
        assert_eq!((w, h), (1, 1));
        assert_eq!(data, vec![7, 8, 9, 255]);

        println!("{CWD_PROBE_MARKER}");
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
