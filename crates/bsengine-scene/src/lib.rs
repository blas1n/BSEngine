pub mod plugin;
pub mod types;

pub use plugin::{spawn_scene_entities, Name, ScenePlugin};
pub use types::{
    ColliderDesc, ColliderShapeDesc, DirectionalLightDescriptor, EntityDescriptor,
    PendingSceneLoad, PhysicsBodyDesc, Primitive, PrimitiveMesh, RigidBodyDesc, SceneDescriptor,
    ScriptPath, TransformDescriptor,
};
