//! GLTF/GLB asset import for BSEngine.
//!
//! `GltfLoader` parses a glTF file into `LoadedGltf` (mesh + `MeshData`),
//! with `AnimationClip`/`AnimationChannel`/`Interpolation` covering
//! skeletal animation data. `GltfPlugin` wires loading into the app;
//! `GltfAsset` is the resulting ECS-facing handle.
#![warn(missing_docs)]

pub mod animation;
pub mod loader;
pub mod plugin;

pub use animation::{AnimationChannel, AnimationClip, Interpolation, KeyframeValues};
pub use loader::{GltfImageData, GltfLoader, LoadedGltf, MeshData};
pub use plugin::{GltfAsset, GltfPlugin};
