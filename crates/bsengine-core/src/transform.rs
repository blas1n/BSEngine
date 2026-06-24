use bevy_ecs::prelude::Component;
use glam::{Mat4, Quat, Vec3};

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Default::default()
        }
    }

    pub fn looking_at(mut self, target: Vec3, up: Vec3) -> Self {
        let dir = (target - self.translation).normalize();
        let right = up.cross(dir).normalize();
        let up = dir.cross(right);
        self.rotation = Quat::from_mat3(&glam::Mat3::from_cols(right, up, dir));
        self
    }

    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

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
        assert_eq!(t.translation, Vec3::ZERO);
        assert_eq!(t.rotation, Quat::IDENTITY);
        assert_eq!(t.scale, Vec3::ONE);
    }

    #[test]
    fn to_matrix_identity_for_default() {
        let t = Transform::default();
        assert!(t.to_matrix().abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }

    #[test]
    fn from_translation_sets_position() {
        let t = Transform::from_translation(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(t.translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(t.scale, Vec3::ONE);
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
