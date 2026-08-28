//! GLTF/GLB asset import for BSEngine.
//!
//! `GltfLoader` parses a glTF file into `LoadedGltf` (mesh + `MeshData`),
//! with `AnimationClip`/`AnimationChannel`/`Interpolation` covering
//! skeletal animation data. `GltfPlugin` wires loading into the app;
//! `GltfAsset` is the resulting ECS-facing handle.
#![warn(missing_docs)]
// Bevy `Query` types with several optional components and a filter are
// unavoidably verbose; matches bsengine-render/-rhi-wgpu/-editor/-scripting.
#![allow(clippy::type_complexity)]

/// Animation clip/channel/interpolation types for skeletal animation data.
pub mod animation;
/// `AssetLoader` backing `LoadMode::Async` for glTF/GLB files.
pub mod asset_loader;
/// GLTF/GLB file parsing into mesh, image, and animation data.
pub mod loader;
/// The Bevy plugin that spawns loaded GLTF assets into the ECS world.
pub mod plugin;
/// Bind-pose geometry, skin data, and clip library for CPU-side skeletal skinning.
pub mod skinned_mesh;

pub use animation::{AnimationChannel, AnimationClip, Interpolation, KeyframeValues};
pub use asset_loader::GltfSourceLoader;
pub use loader::{
    GltfImageData, GltfLoader, LoadedGltf, MeshData, NodeTransform, SkinData, VertexSkin,
};
pub use plugin::{GltfAsset, GltfPlugin, LodRequest};
pub use skinned_mesh::{AnimationClipLibrary, SkinnedMesh, SkinnedMeshPlugin};
