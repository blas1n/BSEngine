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
    /// How opaque the surface is: 1.0 is solid, 0.0 fully invisible.
    ///
    /// Anything below 1.0 takes the object out of the opaque pass and into the
    /// sorted transparent one. It is a material property alongside `metallic`
    /// and `roughness` rather than a fourth channel on `base_color`, because
    /// [`ReflectColor`] is shared with `emissive` and with light colours, where
    /// an alpha channel would mean nothing.
    pub opacity: f32,
}

/// The image this entity wants as its base color texture, named by path.
///
/// Lives beside [`Material`] rather than in the scene crate because it is what
/// a material is *asking for*: the scene writes it, the renderer reads it, and
/// the editor round-trips it, and all three already depend on this crate. Put
/// in the scene crate it would have made the renderer depend on the scene file
/// format, which is not a thing rendering should know about.
///
/// Kept after the texture loads rather than removed. `Material` records only
/// the id it ended up with, and an id cannot be turned back into a path, so
/// removing this would mean the editor could not write the reference back out
/// when saving a scene.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct TexturePath(pub String);

impl Default for Material {
    fn default() -> Self {
        Self {
            texture_id: None,
            metallic: 0.0,
            roughness: 0.5,
            emissive: Vec3::ZERO.into(),
            base_color: Vec3::ONE.into(),
            opacity: 1.0,
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
