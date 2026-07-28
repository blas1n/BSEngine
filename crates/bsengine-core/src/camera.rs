use crate::reflect_degrees::ReflectDegrees;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::Mat4;

/// Perspective camera component holding the field of view and clip planes
/// used to build a right-handed projection matrix each frame.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct Camera {
    /// Vertical field of view.
    pub fov_y_degrees: ReflectDegrees,
    /// Viewport width divided by height.
    pub aspect_ratio: f32,
    /// Distance to the near clip plane.
    pub near: f32,
    /// Distance to the far clip plane.
    pub far: f32,
}

impl Camera {
    /// Creates a perspective camera with the given vertical FOV and aspect ratio, and default near/far planes.
    pub fn perspective(fov_y_degrees: f32, aspect_ratio: f32) -> Self {
        Self {
            fov_y_degrees: fov_y_degrees.into(),
            aspect_ratio,
            near: 0.1,
            far: 1000.0,
        }
    }

    /// Builds the right-handed perspective projection matrix for this camera.
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(
            self.fov_y_degrees.to_radians(),
            self.aspect_ratio,
            self.near,
            self.far,
        )
    }

    /// Recomputes `aspect_ratio` from a viewport size, ignoring zero-height viewports.
    pub fn update_aspect_ratio(&mut self, width: u32, height: u32) {
        if height > 0 {
            self.aspect_ratio = width as f32 / height as f32;
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::perspective(60.0, 16.0 / 9.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_camera_has_60_fov() {
        let cam = Camera::default();
        assert!((cam.fov_y_degrees.0 - 60.0).abs() < 1e-6);
    }

    #[test]
    fn projection_matrix_is_non_identity() {
        let cam = Camera::default();
        let proj = cam.projection_matrix();
        assert!(!proj.abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }

    #[test]
    fn projection_matrix_converts_degrees_to_radians_at_point_of_use() {
        let cam = Camera::perspective(90.0, 1.0);
        let expected = Mat4::perspective_rh(90_f32.to_radians(), 1.0, 0.1, 1000.0);
        assert!(cam.projection_matrix().abs_diff_eq(expected, 1e-6));
    }

    #[test]
    fn update_aspect_ratio_ignores_zero_height() {
        let mut cam = Camera::default();
        let original = cam.aspect_ratio;
        cam.update_aspect_ratio(1920, 0);
        assert_eq!(cam.aspect_ratio, original);
    }

    #[test]
    fn camera_is_registered_reflectable() {
        use bevy_reflect::TypeRegistry;
        let mut registry = TypeRegistry::default();
        registry.register::<Camera>();
        let registration = registry
            .get(std::any::TypeId::of::<Camera>())
            .expect("Camera not registered");
        assert_eq!(
            registration.type_info().type_path(),
            "bsengine_core::camera::Camera"
        );
    }
}
