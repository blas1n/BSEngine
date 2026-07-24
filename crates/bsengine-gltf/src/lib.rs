//! GLTF/GLB asset import for BSEngine.
//!
//! `GltfLoader` parses a glTF file into `LoadedGltf` (mesh + `MeshData`),
//! with `AnimationClip`/`AnimationChannel`/`Interpolation` covering
//! skeletal animation data. `GltfPlugin` wires loading into the app;
//! `GltfAsset` is the resulting ECS-facing handle.
#![warn(missing_docs)]

/// Animation clip/channel/interpolation types for skeletal animation data.
pub mod animation;
/// GLTF/GLB file parsing into mesh, image, and animation data.
pub mod loader;
/// The Bevy plugin that spawns loaded GLTF assets into the ECS world.
pub mod plugin;

pub use animation::{AnimationChannel, AnimationClip, Interpolation, KeyframeValues};
pub use loader::{GltfImageData, GltfLoader, LoadedGltf, MeshData};
pub use plugin::{GltfAsset, GltfPlugin};
