use bevy_ecs::prelude::Component;
use glam::Vec3;

#[derive(Component, Debug, Clone)]
pub struct Material {
    pub texture_id: Option<u64>,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: Vec3,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            texture_id: None,
            metallic: 0.0,
            roughness: 0.5,
            emissive: Vec3::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_default_has_no_texture() {
        let m = Material::default();
        assert!(m.texture_id.is_none());
    }

    #[test]
    fn material_default_pbr_values() {
        let m = Material::default();
        assert_eq!(m.metallic, 0.0);
        assert!((m.roughness - 0.5).abs() < 1e-6);
        assert_eq!(m.emissive, Vec3::ZERO);
    }
}
