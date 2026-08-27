//! Asset loading for BSEngine — glTF, textures, shaders, and audio share this
//! crate's `LoadMode`-dispatched `load()` helper and `bevy_asset`'s `Handle<T>`/
//! `Assets<T>`. See `docs/superpowers/specs/2026-07-31-bevy-asset-pipeline-design.md`.
//!
//! # The asset root, and why `BEVY_ASSET_ROOT` is ignored
//!
//! Every asset path in this engine is produced by
//! `bsengine_core::resolve_project_path`, which joins `ProjectDir` with a
//! scene-relative path — and the result is **relative to the process working
//! directory**, exactly like every other path the engine reads. Scenes,
//! `project.toml` and scripts all go through plain `std::fs` and resolve that
//! way, so [`AssetPlugin`] pins `bevy_asset`'s root to the working directory
//! too, and publishes it as [`plugin::AssetRoot`].
//!
//! `bevy_asset` would otherwise pick that root itself, preferring the
//! `BEVY_ASSET_ROOT` environment variable, then `CARGO_MANIFEST_DIR`, then the
//! executable's own directory. **Pinning the root to the working directory
//! means `BEVY_ASSET_ROOT` has no effect here, on purpose.** Honouring it
//! would let `bevy_asset` resolve from one directory while every `std::fs`
//! read in the engine resolved from another — an opt-in switch for a
//! split-brain in which meshes, textures and shaders load from somewhere the
//! scene that references them does not. That is not a hypothetical: resolving
//! assets under `CARGO_MANIFEST_DIR` instead of the working directory is a bug
//! this engine actually shipped, and because a failed asset load is only a
//! `WARN`, it ran a whole game without its meshes or its shaders for as long
//! as it stood.
//!
//! To move where assets come from, change the process working directory (or
//! `ProjectDir`); that moves the engine's other reads with it, which is the
//! point.
#![warn(missing_docs)]

/// `AssetGuid` and the `.meta` sidecar that gives an asset an identity
/// independent of its path, plus the `AssetIdentityPlugin` that publishes a
/// scan of them as an `AssetIndex`. Registered by all three hosts, and its
/// docs explain why registering it is only half of making it work.
pub mod identity;
/// `AssetLoader` backing `LoadMode::Async` for `HeightmapAsset`.
pub mod heightmap_loader;
/// Chooses between synchronous (blocking, zero-latency) and asynchronous
/// (`AssetServer`-driven) loading, plus the dispatch helper itself.
pub mod load_mode;
/// Wires `bevy_asset`'s `AssetPlugin` into the app and registers this
/// crate's own asset types.
pub mod plugin;
/// `AssetSlot`: the request-once / poll / never-re-request-a-failure state
/// machine every asynchronous asset consumer in this engine needs.
pub mod slot;
/// `AssetStatusPlugin`: records what became of each asset the engine tried to
/// load, so "what happened to this asset?" is answerable from code rather than
/// from a log line.
pub mod status;
/// `AssetLoader` backing `LoadMode::Async` for `TextureAsset`.
pub mod texture_loader;
/// Concrete asset data types owned by this crate (`TextureAsset`,
/// `HeightmapAsset`).
pub mod types;
/// `AssetWatcherPlugin`: watches `<ProjectDir>/assets` and reloads assets
/// edited on disk while the game runs.
pub mod watcher;

/// Probe directories and log capture, shared by the watcher's tests, the
/// identity scan's, and — through the `test-support` feature — by
/// `bsengine-scene`'s resolution tests. Never compiled into a shipped build.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use bevy_asset::{Asset, AssetServer, Assets, Handle};
pub use heightmap_loader::HeightmapAssetLoader;
pub use identity::{AssetGuid, AssetIdentityPlugin, AssetIndex};
pub use load_mode::{load, load_async, LoadMode};
pub use plugin::AssetPlugin;
pub use slot::{AssetSlot, Polled};
pub use status::{AssetStatus, AssetStatusPlugin, AssetStatuses};
pub use texture_loader::TextureAssetLoader;
pub use types::{HeightmapAsset, TextureAsset};
pub use watcher::AssetWatcherPlugin;
