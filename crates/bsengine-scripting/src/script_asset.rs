//! JavaScript source as a `bevy_asset` asset.
//!
//! Scripts used to be read with `std::fs::read_to_string` at `PostStartup` and
//! never looked at again. Going through `AssetServer` instead is what buys them
//! an identity (`Handle<ScriptSource>`), an `AssetEvent::Modified` when the file
//! changes on disk, and therefore hot reload — the reload that matters most
//! here, since scripts are the bulk of what a scene references and every E2E
//! recording is script-driven.
//!
//! `assets/scripts/*.js` files carry `crate`-external identity sidecars
//! (`player.js.meta`, minted by `bsengine_asset::identity::scan`), which spell
//! their metadata with the same filename `bevy_asset` reserves for its own
//! `AssetMetaMinimal`. That collision is already neutralised for every asset
//! type at once: `bsengine_asset::AssetPlugin` builds `bevy_asset` with
//! `AssetMetaCheck::Never`, so the sidecar is never opened as bevy meta.

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext};
use bevy_reflect::TypePath;

/// Raw JavaScript source text for one script, loaded from disk. Executing it
/// happens separately in [`crate::runtime`] — a V8 isolate is neither `Send`
/// nor `Sync` and so cannot live inside an asset, which every ECS thread may
/// read.
///
/// `Clone` because `Assets<T>` hands out `&ScriptSource` and the runtime needs
/// an owned `String` to evaluate; the source is held in memory for the life of
/// the handle, which is what makes reload a diff against the previous text
/// rather than a second read of the file. Game scripts are a few kilobytes each
/// and there are tens of them, so that is a rounding error next to the meshes
/// and textures already resident.
#[derive(Asset, TypePath, Debug, Clone)]
pub struct ScriptSource(pub String);

/// Reads a `.js` file into a [`ScriptSource`] for `AssetServer::load`.
///
/// Declares no [`AssetLoader::extensions`], like every other loader in this
/// engine: loads are type-directed (`AssetServer::load::<ScriptSource>(path)`),
/// so nothing here claims the `.js` extension or competes for it.
#[derive(Default)]
pub struct ScriptSourceLoader;

impl AssetLoader for ScriptSourceLoader {
    type Asset = ScriptSource;
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
        // Deliberately not `from_utf8_lossy`: a script that loads with U+FFFD
        // where a character used to be is a script that runs and misbehaves,
        // and V8 would report the resulting syntax error at a line that looks
        // fine in the editor. Failing the load names the file instead.
        String::from_utf8(bytes)
            .map(ScriptSource)
            .map_err(|e| format!("not valid UTF-8: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::ScriptingPlugin;
    use bevy_asset::{AssetServer, Assets, LoadState};
    use bsengine_app::new_app;

    /// A `.js` path in the temp directory, unique per test and per process so
    /// two of these can run in parallel and a stale file from an earlier run
    /// cannot be mistaken for this one's.
    fn temp_script(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("bsengine_test_{name}_{}.js", std::process::id()))
    }

    /// A real app with the real plugins, so what is under test is the
    /// registration `ScriptingPlugin::build` performs, not a hand-rolled
    /// `init_asset` an integrator would have to remember to repeat.
    fn app_with_scripting() -> bevy_app::App {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin::default());
        app
    }

    #[test]
    fn asset_server_loads_a_js_file_into_a_script_source() {
        let path = temp_script("script_source_loads");
        // Non-ASCII on purpose: the point of this type is that the runtime
        // gets back exactly the bytes the author wrote, and a lossy read would
        // still produce a `ScriptSource` of plausible length.
        let source = "function onUpdate(name) { /* 안녕 — こんにちは */ }\n";
        std::fs::write(&path, source).unwrap();

        let mut app = app_with_scripting();
        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<ScriptSource>(path.to_str().unwrap().to_owned())
        };

        let mut loaded = None;
        for _ in 0..200 {
            app.update();
            if let Some(src) = app.world().resource::<Assets<ScriptSource>>().get(&handle) {
                loaded = Some(src.0.clone());
                break;
            }
        }
        let state = app
            .world()
            .resource::<AssetServer>()
            .get_load_state(handle.id());
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            loaded.as_deref(),
            Some(source),
            "the asset must carry the file's exact contents; load_state={state:?}"
        );
    }

    #[test]
    fn a_non_utf8_js_file_fails_the_load_and_names_the_file() {
        let path = temp_script("non_utf8_script");
        // 0xFF cannot appear anywhere in valid UTF-8, so this is invalid no
        // matter how the rest is chunked.
        std::fs::write(&path, b"function onUpdate() { /* \xff\xfe */ }\n").unwrap();
        let file_name = path.file_name().unwrap().to_string_lossy().into_owned();

        let mut app = app_with_scripting();
        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<ScriptSource>(path.to_str().unwrap().to_owned())
        };

        // No `catch_unwind` here: a panic in the loader would abort this test
        // outright, which is the reporting we want — the assertion below only
        // has to separate "failed" from "silently arrived".
        let mut failure = None;
        for _ in 0..200 {
            app.update();
            if let Some(LoadState::Failed(err)) = app
                .world()
                .resource::<AssetServer>()
                .get_load_state(handle.id())
            {
                failure = Some(err.to_string());
                break;
            }
        }
        let arrived = app
            .world()
            .resource::<Assets<ScriptSource>>()
            .get(&handle)
            .map(|s| s.0.clone());
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            arrived, None,
            "a non-UTF-8 script must not arrive at all; a lossily converted one \
             would run with U+FFFD where the author's characters were"
        );
        let failure = failure.expect("a non-UTF-8 script must reach LoadState::Failed");
        assert!(
            failure.contains("not valid UTF-8"),
            "the failure must say what is wrong with the file: {failure}"
        );
        assert!(
            failure.contains(&file_name),
            "the failure must name the file that could not be read, or a project \
             with fifty scripts learns only that one of them is bad: {failure}"
        );
    }
}
