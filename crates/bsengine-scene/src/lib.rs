//! Scene graph, entity transforms, and scene file I/O for BSEngine.
//!
//! `ScenePlugin`/`spawn_scene_entities` load a `SceneDescriptor` (RON
//! format) into the ECS world, deserializing `EntityDescriptor` and its
//! nested light/physics/collider/transform descriptor types
//! (`PointLightDescriptor`, `RigidBodyDesc`, `TransformDescriptor`, ...).
#![warn(missing_docs)]

pub mod plugin;
pub mod types;

pub use plugin::{spawn_scene_entities, Name, ScenePlugin};
pub use types::{
    ColliderDesc, ColliderShapeDesc, DirectionalLightDescriptor, EntityDescriptor,
    PendingSceneLoad, PhysicsBodyDesc, PointLightDescriptor, Primitive, PrimitiveMesh,
    RigidBodyDesc, SceneDescriptor, ScriptPath, SpotLightDescriptor, TransformDescriptor,
};
