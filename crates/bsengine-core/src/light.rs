use bevy_ecs::prelude::Component;
use glam::Vec3;

#[derive(Component, Debug, Clone)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub ambient: Vec3,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.4, -0.8, -0.4).normalize(),
            color: Vec3::ONE,
            ambient: Vec3::splat(0.15),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_light_direction_is_unit() {
        let light = DirectionalLight::default();
        assert!((light.direction.length() - 1.0).abs() < 1e-5);
    }
}
