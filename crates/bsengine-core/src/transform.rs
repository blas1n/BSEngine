use crate::{ReflectQuat, ReflectVec3};
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::{Mat4, Quat, Vec3};

/// Local position, rotation, and scale of an entity, relative to its parent (if any).
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct Transform {
    /// Local-space position.
    pub translation: ReflectVec3,
    /// Local-space rotation.
    pub rotation: ReflectQuat,
    /// Local-space scale.
    pub scale: ReflectVec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO.into(),
            rotation: Quat::IDENTITY.into(),
            scale: Vec3::ONE.into(),
        }
    }
}

impl Transform {
    /// Creates a transform at the given position with identity rotation and unit scale.
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation: translation.into(),
            ..Default::default()
        }
    }

    /// Rotates this transform so it faces `target`, using `up` to determine the roll axis.
    pub fn looking_at(mut self, target: Vec3, up: Vec3) -> Self {
        let dir = (target - self.translation.0).normalize();
        let right = up.cross(dir).normalize();
        let up = dir.cross(right);
        self.rotation = Quat::from_mat3(&glam::Mat3::from_cols(right, up, dir)).into();
        self
    }

    /// Builds the local-to-world transform matrix (scale, then rotate, then translate).
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale.0, self.rotation.0, self.translation.0)
    }

    /// Builds the world-to-view matrix, i.e. the inverse of `to_matrix()`.
    pub fn view_matrix(&self) -> Mat4 {
        self.to_matrix().inverse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_identity() {
        let t = Transform::default();
        assert_eq!(t.translation, Vec3::ZERO.into());
        assert_eq!(t.rotation, Quat::IDENTITY.into());
        assert_eq!(t.scale, Vec3::ONE.into());
    }

    #[test]
    fn to_matrix_identity_for_default() {
        let t = Transform::default();
        assert!(t.to_matrix().abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }

    #[test]
    fn from_translation_sets_position() {
        let t = Transform::from_translation(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(t.translation, Vec3::new(1.0, 2.0, 3.0).into());
        assert_eq!(t.scale, Vec3::ONE.into());
    }

    #[test]
    fn view_matrix_is_inverse_of_model() {
        let t = Transform::from_translation(Vec3::new(0.0, 0.0, 5.0));
        let model = t.to_matrix();
        let view = t.view_matrix();
        let product = model * view;
        assert!(product.abs_diff_eq(Mat4::IDENTITY, 1e-5));
    }
}
