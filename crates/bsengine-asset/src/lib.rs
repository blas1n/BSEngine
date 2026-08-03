//! Asset loading for BSEngine — glTF, textures, shaders, and audio share this
//! crate's `LoadMode`-dispatched `load()` helper and `bevy_asset`'s `Handle<T>`/
//! `Assets<T>`. See `docs/superpowers/specs/2026-07-31-bevy-asset-pipeline-design.md`.
#![warn(missing_docs)]

/// Chooses between synchronous (blocking, zero-latency) and asynchronous
/// (`AssetServer`-driven) loading, plus the dispatch helper itself.
pub mod load_mode;
/// Wires `bevy_asset`'s `AssetPlugin` into the app and registers this
/// crate's own asset types.
pub mod plugin;
/// `AssetLoader` backing `LoadMode::Async` for `TextureAsset`.
pub mod texture_loader;
/// Concrete asset data types owned by this crate (currently: `TextureAsset`).
pub mod types;
// Filesystem watching for hot reload. Private while it holds only the
// measurement the watcher will be built on.
mod watcher;

pub use bevy_asset::{Asset, AssetServer, Assets, Handle};
pub use load_mode::{load, LoadMode};
pub use plugin::AssetPlugin;
pub use texture_loader::TextureAssetLoader;
pub use types::TextureAsset;
