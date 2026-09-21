//! Scene graph, entity transforms, and scene file I/O for BSEngine.
//!
//! `ScenePlugin`/`spawn_scene_entities` load a `SceneDescriptor` (RON
//! format) into the ECS world, deserializing `EntityDescriptor` and its
//! nested light/physics/collider/transform descriptor types
//! (`PointLightDescriptor`, `RigidBodyDesc`, `TransformDescriptor`, ...).
#![warn(missing_docs)]

/// `ScenePlugin` and the entity-spawning logic that turns a `SceneDescriptor` into ECS entities.
pub mod plugin;
/// Single-prefab instantiation: turns a `PrefabDescriptor` into spawned entities by
/// delegating to `spawn_scene_entities`.
pub mod prefab;
/// Deciding whether a streamed scene should be in the world, from the camera's
/// distance to it. Pure; `bsengine-runtime` owns the system that acts on it.
pub mod streaming;
/// Serde/RON descriptor types that make up the on-disk scene file format.
pub mod types;

pub use plugin::{
    instantiate_prefab_from_path, instantiate_prefab_reference, register_gameplay_reflect_types,
    resolve_asset_ref_for_field, spawn_scene_entities, Name, ScenePlugin,
};
pub use prefab::{instantiate_prefab, next_instance_suffix, validate_prefab_descriptor};
pub use types::{
    AssetRef, Cloth, ColliderDesc, ColliderShapeDesc, DirectionalLightDescriptor, EntityDescriptor,
    JointDescriptor, JointKindDesc, LoadedScenes, PendingSceneLoad, PendingSceneStream,
    PhysicsBodyDesc, PointLightDescriptor, Primitive, PrimitiveMesh, RigidBodyDesc,
    SceneDescriptor, SceneStreamOp, ScriptPath, SpotLightDescriptor, StreamedScene, Terrain,
    TransformDescriptor,
};
