use bevy_ecs::prelude::Component;
use glam::Mat4;

#[derive(Component, Debug, Clone)]
pub struct Camera {
    pub fov_y_radians: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn perspective(fov_y_degrees: f32, aspect_ratio: f32) -> Self {
        Self {
            fov_y_radians: fov_y_degrees.to_radians(),
            aspect_ratio,
            near: 0.1,
            far: 1000.0,
        }
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y_radians, self.aspect_ratio, self.near, self.far)
    }

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
        assert!((cam.fov_y_radians - 60_f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn projection_matrix_is_non_identity() {
        let cam = Camera::default();
        let proj = cam.projection_matrix();
        assert!(!proj.abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }

    #[test]
    fn update_aspect_ratio_ignores_zero_height() {
        let mut cam = Camera::default();
        let original = cam.aspect_ratio;
        cam.update_aspect_ratio(1920, 0);
        assert_eq!(cam.aspect_ratio, original);
    }
}
