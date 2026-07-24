//! Asset loading for BSEngine — textures and meshes.
//!
//! `AssetServer` loads and caches assets behind a `Handle<T>`, backed by
//! `MeshAsset`/`TextureAsset`. `AssetPlugin` wires the server into the app
//! as the `AssetServerResource`.
#![warn(missing_docs)]

/// Typed, cloneable references to loaded assets.
pub mod handle;
/// Wires the asset server into the app as a resource.
pub mod plugin;
/// Loads and caches raw asset bytes from disk.
pub mod server;
/// Concrete asset data types (textures, meshes).
pub mod types;

pub use handle::Handle;
pub use plugin::{AssetPlugin, AssetServerResource};
pub use server::AssetServer;
pub use types::{MeshAsset, TextureAsset};
