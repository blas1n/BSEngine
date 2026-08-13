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
/// Serde/RON descriptor types that make up the on-disk scene file format.
pub mod types;

pub use plugin::{
    instantiate_prefab_from_path, register_gameplay_reflect_types, spawn_scene_entities, Name,
    ScenePlugin,
};
pub use prefab::{instantiate_prefab, next_instance_suffix};
pub use types::{
    AssetRef, ColliderDesc, ColliderShapeDesc, DirectionalLightDescriptor, EntityDescriptor,
    PendingSceneLoad, PhysicsBodyDesc, PointLightDescriptor, Primitive, PrimitiveMesh,
    RigidBodyDesc, SceneDescriptor, ScriptPath, SpotLightDescriptor, TransformDescriptor,
};
