use crate::reflect_color::ReflectColor;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::Vec3;

/// PBR (physically-based rendering) surface material properties.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct Material {
    /// Id of the base color texture asset, or `None` for a flat-colored surface.
    pub texture_id: Option<u64>,
    /// How metallic the surface is, from 0 (dielectric) to 1 (metal).
    pub metallic: f32,
    /// Surface microfacet roughness, from 0 (mirror-smooth) to 1 (fully diffuse).
    pub roughness: f32,
    /// Self-emitted light color, added regardless of scene lighting.
    pub emissive: ReflectColor,
    /// Base (albedo) color of the surface.
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
