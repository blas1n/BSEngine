use bevy_asset::{Asset, AssetServer, Assets, Handle};

/// How an asset gets from disk into `Assets<T>` — one real, working
/// mechanism per variant (mirrors Unreal's blocking `LoadObject` vs.
/// `FStreamableManager`'s async load, as two things a caller picks between,
/// not a hint):
///
/// `Sync` calls the loader function directly and inserts the result
/// immediately (blocking, zero-latency — matches every asset load in this
/// engine as of item 23). `Async` calls `bevy_asset`'s own
/// `AssetServer::load`, which requires a registered `AssetLoader` for `T` —
/// multi-frame latency, but automatically tracked by the file watcher item
/// 24 will enable. Every item-23 consumer passes `Sync` (see `load()`'s
/// call sites in `bsengine-gltf`/`bsengine-render`/`bsengine-scripting`);
/// `Async` is exercised only by each asset type's own
/// `*_loads_async_and_becomes_available` test, proving the path is real and
/// ready for a future consumer to flip a single call-site argument to use,
/// without needing to re-plumb anything.
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
