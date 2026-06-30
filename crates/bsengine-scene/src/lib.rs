pub mod plugin;
pub mod types;

pub use plugin::{Name, ScenePlugin, spawn_scene_entities};
pub use types::{
    ColliderDesc, ColliderShapeDesc, DirectionalLightDescriptor, EntityDescriptor,
    PendingSceneLoad, PhysicsBodyDesc, Primitive, PrimitiveMesh, RigidBodyDesc, SceneDescriptor,
    ScriptPath, TransformDescriptor,
};
