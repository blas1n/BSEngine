use crate::reflect_color::ReflectColor;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::Vec3;

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct Material {
    pub texture_id: Option<u64>,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: ReflectColor,
    pub base_color: ReflectColor,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            texture_id: None,
            metallic: 0.0,
            roughness: 0.5,
            emissive: Vec3::ZERO.into(),
            base_color: Vec3::ONE.into(),
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
        assert_eq!(m.emissive, Vec3::ZERO.into());
    }

    #[test]
    fn material_is_registered_reflectable() {
        use bevy_reflect::TypeRegistry;
        let mut registry = TypeRegistry::default();
        registry.register::<Material>();
        let registration = registry
            .get(std::any::TypeId::of::<Material>())
            .expect("Material not registered");
        assert_eq!(
            registration.type_info().type_path(),
            "bsengine_core::material::Material"
        );
    }
}
