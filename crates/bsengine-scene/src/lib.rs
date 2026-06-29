pub mod plugin;
pub mod types;

pub use plugin::{Name, ScenePlugin};
pub use types::{
    DirectionalLightDescriptor, EntityDescriptor, Primitive, PrimitiveMesh, SceneDescriptor,
    ScriptPath, TransformDescriptor,
};
