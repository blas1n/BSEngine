//! Asset loading for BSEngine — textures and meshes.
//!
//! `AssetServer` loads and caches assets behind a `Handle<T>`, backed by
//! `MeshAsset`/`TextureAsset`. `AssetPlugin` wires the server into the app
//! as the `AssetServerResource`.
#![warn(missing_docs)]

pub mod handle;
pub mod plugin;
pub mod server;
pub mod types;

pub use handle::Handle;
pub use plugin::{AssetPlugin, AssetServerResource};
pub use server::AssetServer;
pub use types::{MeshAsset, TextureAsset};
