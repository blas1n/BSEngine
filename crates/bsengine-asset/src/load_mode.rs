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
/// Consumers are migrating to `Async` (`bsengine-gltf` already has); the rest
/// still pass `Sync`.
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
/// See `bsengine_gltf::plugin`'s `PendingGltf` for a worked example.
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
