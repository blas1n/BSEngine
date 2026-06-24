use bevy_ecs::prelude::Component;
use glam::Mat4;

#[derive(Component, Debug, Clone)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Mat4::IDENTITY)
    }
}

impl GlobalTransform {
    pub fn to_matrix(&self) -> Mat4 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_transform_default_is_identity() {
        let gt = GlobalTransform::default();
        assert_eq!(gt.0, Mat4::IDENTITY);
    }
}
