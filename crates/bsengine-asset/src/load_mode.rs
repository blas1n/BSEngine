use bevy_asset::{Asset, AssetServer, Assets, Handle};

/// How an asset gets from disk into `Assets<T>` — one real, working
/// mechanism per variant (mirrors Unreal's blocking `LoadObject` vs.
/// `FStreamableManager`'s async load, as two things a caller picks between,
/// not a hint):
///
/// `Sync` calls the loader function directly and inserts the result
/// immediately (blocking, zero-latency). `Async` calls `bevy_asset`'s own
/// `AssetServer::load`, which requires a registered `AssetLoader` for `T` —
/// multi-frame latency, but automatically tracked by the file watcher.
///
/// Nothing in the engine passes `Sync` any more. All four consumers (glTF,
/// custom shaders, skybox, audio) load asynchronously, and the only `Sync`
/// call sites left in the workspace are this module's own unit tests. The
/// variant is kept, working and supported, because blocking-vs-streaming is
/// the caller's choice to make: a game that wants to stall on a small asset
/// behind a loading screen should not have to reimplement it.
///
/// # Polling an `Async` load correctly
///
/// `Async` returns a `Handle` unconditionally, so a missing or malformed file
/// is indistinguishable from a slow one at the call site. A consumer must
/// therefore:
///
/// 1. **Request once and retain the handle**, rather than re-requesting each
///    frame while polling. Re-calling `AssetServer::load` for a path whose
///    state is `Failed` resets it to `Loading` and restarts the load
///    (`bevy_asset` 0.14.2, `server/info.rs:216-221`). Because `Failed` is set
///    in `PreUpdate` and consumers poll in `Update`, a re-requesting loop
///    erases the failure before it can observe it — retrying forever and
///    spawning a fresh filesystem task every frame. Retaining the handle also
///    keeps it strong, which nothing else does between frames.
/// 2. **Treat "absent from `Assets<T>`" as inconclusive** and ask
///    `asset_server.load_state(&handle)`; on `LoadState::Failed(e)`, warn and
///    stop. Otherwise a bad path fails silently and permanently, which is
///    worse than `Sync`'s warn-once-and-give-up.
///
/// The resulting shape — request once, retain the handle, poll the retained
/// handle — is what every consumer in this engine implements:
///
/// ```ignore
/// // First frame for this path: request, and keep what `load` returns
/// // somewhere that outlives the frame (a component, or a resource keyed
/// // by path).
/// let handle = bsengine_asset::load(
///     LoadMode::Async, &asset_server, &mut assets, path, sync_loader,
/// )?;
/// pending.insert(path.to_owned(), Pending::Loading(handle));
///
/// // Every later frame: poll the handle that was stored. Never call
/// // `load` (or `AssetServer::load`) for this path again.
/// match assets.get(&handle) {
///     Some(asset) => { /* use it, and drop the pending entry */ }
///     None => {
///         if let LoadState::Failed(e) = asset_server.load_state(&handle) {
///             warn!("{path}: {e}");
///             // Record "gave up", so the next frame does not re-request
///             // it and reset the failure back to `Loading`.
///             pending.insert(path.to_owned(), Pending::GaveUp);
///         }
///     }
/// }
/// ```
///
/// The live implementations are `bsengine_gltf::GltfPlugin` (a per-entity
/// pending component), `bsengine_render::RenderPlugin` (a path-keyed map for
/// custom shaders, a single slot for the skybox) and `bsengine_scripting`'s
/// sound loading. Their pending state is private to each system on purpose:
/// `CustomShader.path`, `SkyboxPath` and friends stay plain `String`s, so
/// scene RON, the scripting API and the MCP tools are unaffected by how a
/// load is tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadMode {
    /// Load and insert synchronously, right now, on the calling thread.
    Sync,
    /// Queue an asynchronous load via `AssetServer`. Requires an
    /// `AssetLoader` for `T` to already be registered (via
    /// `app.register_asset_loader(...)`) — see each asset type's own
    /// `*Loader` for the concrete implementation.
    Async,
}

/// Loads `path` as a `T`, dispatching on `mode`. `Sync` runs `sync_loader`
/// directly and inserts via `Assets::add` (no caching/deduplication — every
/// call re-runs `sync_loader`, matching this engine's pre-item-23 behavior,
/// since none of the four migrated call sites cached before either).
/// `Async` calls `AssetServer::load::<T>(path)` and ignores `sync_loader`
/// entirely (the registered `AssetLoader` is used instead).
pub fn load<T, E>(
    mode: LoadMode,
    asset_server: &AssetServer,
    assets: &mut Assets<T>,
    path: &str,
    sync_loader: impl FnOnce(&str) -> Result<T, E>,
) -> Result<Handle<T>, String>
where
    T: Asset,
    E: std::fmt::Display,
{
    match mode {
        LoadMode::Sync => {
            let value = sync_loader(path).map_err(|e| format!("{path}: {e}"))?;
            Ok(assets.add(value))
        }
        LoadMode::Async => Ok(asset_server.load::<T>(path.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::{Asset, AssetPlugin, Assets};
    use bevy_reflect::TypePath;
    use bsengine_app::new_app;

    #[derive(Asset, TypePath, Debug, PartialEq)]
    struct DummyAsset(String);

    #[test]
    fn load_sync_inserts_and_returns_handle() {
        let mut app = new_app();
        app.add_plugins(AssetPlugin::default());
        app.init_resource::<Assets<DummyAsset>>();
        let server = app.world().resource::<AssetServer>().clone();
        let mut assets = app.world_mut().resource_mut::<Assets<DummyAsset>>();
        let handle = load(LoadMode::Sync, &server, &mut assets, "fake/path.txt", |p| {
            Ok::<_, String>(DummyAsset(format!("loaded:{p}")))
        })
        .unwrap();
        assert_eq!(
            assets.get(&handle),
            Some(&DummyAsset("loaded:fake/path.txt".to_string()))
        );
    }

    #[test]
    fn load_sync_propagates_loader_error() {
        let mut app = new_app();
        app.add_plugins(AssetPlugin::default());
        app.init_resource::<Assets<DummyAsset>>();
        let server = app.world().resource::<AssetServer>().clone();
        let mut assets = app.world_mut().resource_mut::<Assets<DummyAsset>>();
        let err = load(LoadMode::Sync, &server, &mut assets, "bad/path.txt", |_| {
            Err::<DummyAsset, _>("boom".to_string())
        })
        .unwrap_err();
        assert!(err.contains("bad/path.txt"));
        assert!(err.contains("boom"));
    }
}
