pub mod plugin;
pub mod types;

pub use plugin::{spawn_scene_entities, Name, ScenePlugin};
pub use types::{
    ColliderDesc, ColliderShapeDesc, DirectionalLightDescriptor, EntityDescriptor,
    PendingSceneLoad, PhysicsBodyDesc, PointLightDescriptor, Primitive, PrimitiveMesh,
    RigidBodyDesc, SceneDescriptor, ScriptPath, SpotLightDescriptor, TransformDescriptor,
};
