use crate::ReflectMat4;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::Mat4;

/// World-space transform matrix computed by [`crate::propagate_global_transforms`]
/// from an entity's local [`crate::Transform`] and its parent chain.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct GlobalTransform(pub ReflectMat4);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Mat4::IDENTITY.into())
    }
}

impl GlobalTransform {
    /// Returns the underlying world-space transform matrix.
    pub fn to_matrix(&self) -> Mat4 {
        self.0 .0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_transform_default_is_identity() {
        let gt = GlobalTransform::default();
        assert_eq!(gt.0 .0, Mat4::IDENTITY);
    }
}
