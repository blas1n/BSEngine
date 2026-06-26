pub mod plugin;
pub mod types;

pub use plugin::{Name, ScenePlugin};
pub use types::{
    DirectionalLightDescriptor, EntityDescriptor, SceneDescriptor, TransformDescriptor,
};
