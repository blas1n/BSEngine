//! Asset loading for BSEngine — glTF, textures, shaders, and audio share this
//! crate's `LoadMode`-dispatched `load()` helper and `bevy_asset`'s `Handle<T>`/
//! `Assets<T>`. See `docs/superpowers/specs/2026-07-31-bevy-asset-pipeline-design.md`.
//!
//! # The asset root, and the key an asset is loaded by
//!
//! The default asset source is [`project_root::ProjectAssetReader`], rooted at
//! the **project directory** (`ProjectDir`), and an asset is loaded -- and
//! keyed, in `bevy_asset`'s tables, in [`AssetStatuses`], in the watcher and in
//! an archive -- by its **project path**: `assets/x.glb`, relative to the
//! project, forward slashes. Unity (`Assets/...`), Unreal (`/Game/...`) and
//! Godot (`res://`) all key an asset that way and turn it into a file only where
//! the disk is read. Nothing outside the project is read: the reader refuses a
//! path that climbs out, has a root or a drive, or leads out through a link, and
//! `bevy_asset`'s `UnapprovedPathMode` is its default, `Forbid`.
//!
//! Components still hold the *engine form* `bsengine_core::resolve_project_path`
//! makes (`<ProjectDir>/assets/x.glb`, relative to the working directory, which
//! scenes, `project.toml` and scripts are read against with plain `std::fs`).
//! [`asset_key`] converts it at the load -- [`load_async`] does, and so must
//! anything that hands a path to `AssetServer` directly. [`plugin::AssetRoot`] is
//! still the working directory: it is what that engine form is relative to.
//!
//! `BEVY_ASSET_ROOT` has no effect, on purpose: the root is the project, which
//! is where every other read the engine makes for that project resolves too.
//! Letting `bevy_asset` pick its own root (`BEVY_ASSET_ROOT`, then
//! `CARGO_MANIFEST_DIR`, then the executable's directory) would split the two --
//! and that is not hypothetical: resolving assets under `CARGO_MANIFEST_DIR` is a
//! bug this engine actually shipped, and because a failed asset load is only a
//! `WARN`, it ran a whole game without its meshes or its shaders for as long as
//! it stood. To move where assets come from, change `ProjectDir`.
#![warn(missing_docs)]

/// Collects exactly the assets a project reaches and writes a build that runs
/// without the editor. Backs `bsengine-runtime --package`.
pub mod cook;
/// An archive embedded in the executable itself: the single-file build.
pub mod embed;
/// `AssetLoader` backing `LoadMode::Async` for `HeightmapAsset`.
pub mod heightmap_loader;
/// `AssetGuid` and the `.meta` sidecar that gives an asset an identity
/// independent of its path, plus the `AssetIdentityPlugin` that publishes a
/// scan of them as an `AssetIndex`. Registered by all three hosts, and its
/// docs explain why registering it is only half of making it work.
pub mod identity;
/// IES light profiles, imported as light cookies.
pub mod ies;
/// Chooses between synchronous (blocking, zero-latency) and asynchronous
/// (`AssetServer`-driven) loading, plus the dispatch helper itself.
pub mod load_mode;
/// The `.pak` container a packaged build's assets are stored in.
pub mod pak;
/// Serving a packaged build's assets to `bevy_asset` out of a `.pak`.
pub mod pak_reader;
/// Reading scene RON out of a `.pak` -- the three `std::fs` sites
/// `pak_reader` cannot reach.
pub mod pak_source;
/// Wires `bevy_asset`'s `AssetPlugin` into the app and registers this
/// crate's own asset types.
pub mod plugin;
/// The default asset source rooted at the project, and `asset_key`, the
/// project-relative key every asset is loaded and tracked by.
pub mod project_root;
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
#[cfg(not(target_arch = "wasm32"))]
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
pub use plugin::{AssetPlugin, PakAssetPlugin};
pub use project_root::{
    asset_key, asset_key_or_raw, register_project, ProjectAssetReader, ProjectRoot,
};
pub use slot::{AssetSlot, Polled};
pub use status::{AssetStatus, AssetStatusPlugin, AssetStatuses};
pub use texture_loader::TextureAssetLoader;
pub use types::{HeightmapAsset, TextureAsset};
#[cfg(not(target_arch = "wasm32"))]
pub use watcher::AssetWatcherPlugin;
