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

/// Chooses between synchronous (blocking, zero-latency) and asynchronous
/// (`AssetServer`-driven) loading, plus the dispatch helper itself.
pub mod load_mode;
/// Wires `bevy_asset`'s `AssetPlugin` into the app and registers this
/// crate's own asset types.
pub mod plugin;
/// `AssetStatusPlugin`: records what became of each asset the engine tried to
/// load, so "what happened to this asset?" is answerable from code rather than
/// from a log line.
pub mod status;
/// `AssetLoader` backing `LoadMode::Async` for `TextureAsset`.
pub mod texture_loader;
/// Concrete asset data types owned by this crate (currently: `TextureAsset`).
pub mod types;
/// `AssetWatcherPlugin`: watches `<ProjectDir>/assets` and reloads assets
/// edited on disk while the game runs.
pub mod watcher;

pub use bevy_asset::{Asset, AssetServer, Assets, Handle};
pub use load_mode::{load, LoadMode};
pub use plugin::AssetPlugin;
pub use status::{AssetStatus, AssetStatusPlugin, AssetStatuses};
pub use texture_loader::TextureAssetLoader;
pub use types::TextureAsset;
pub use watcher::AssetWatcherPlugin;
