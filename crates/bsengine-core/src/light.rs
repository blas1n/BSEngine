use crate::reflect_degrees::ReflectDegrees;
use crate::reflect_glam::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::Vec3;

// Direction is intentionally not stored here: it's derived from the
// entity's Transform.rotation (rotation * -Z), the same way SpotLight
// already derives its cone direction. This keeps a single source of truth
// for "which way is this thing facing" across all light/entity types, so
// the existing move/rotate gizmos, Inspector Rot fields, and undo/redo all
// work on directional lights for free.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct DirectionalLight {
    pub color: ReflectVec3,
    pub ambient: ReflectVec3,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            color: Vec3::ONE.into(),
            ambient: Vec3::splat(0.15).into(),
        }
    }
}

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct PointLight {
    pub color: ReflectVec3,
    pub intensity: f32,
    pub range: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            color: Vec3::ONE.into(),
            intensity: 1.0,
            range: 10.0,
        }
    }
}

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct SpotLight {
    pub color: ReflectVec3,
    pub intensity: f32,
    pub range: f32,
    /// Inner cone half-angle (degrees) — full brightness inside.
    pub inner_angle_degrees: ReflectDegrees,
    /// Outer cone half-angle (degrees) — zero brightness outside.
    pub outer_angle_degrees: ReflectDegrees,
}

impl Default for SpotLight {
    fn default() -> Self {
        Self {
            color: Vec3::ONE.into(),
            intensity: 1.0,
            range: 10.0,
            inner_angle_degrees: 22.5.into(),
            outer_angle_degrees: 30.0.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_light_has_white_color_and_dim_ambient() {
        let light = DirectionalLight::default();
        assert_eq!(light.color, Vec3::ONE.into());
        assert!(light.ambient.x > 0.0 && light.ambient.x < 1.0);
    }

    #[test]
    fn directional_light_is_registered_reflectable() {
        use bevy_reflect::TypeRegistry;
        let mut registry = TypeRegistry::default();
        registry.register::<DirectionalLight>();
        let registration = registry
            .get(std::any::TypeId::of::<DirectionalLight>())
            .expect("DirectionalLight not registered");
        assert_eq!(
            registration.type_info().type_path(),
            "bsengine_core::light::DirectionalLight"
        );
    }

    #[test]
    fn point_light_default_values() {
        let pl = PointLight::default();
        assert_eq!(pl.color, Vec3::ONE.into());
        assert!((pl.intensity - 1.0).abs() < 1e-6);
        assert!((pl.range - 10.0).abs() < 1e-6);
    }

    #[test]
    fn point_light_is_registered_reflectable() {
        use bevy_reflect::TypeRegistry;
        let mut registry = TypeRegistry::default();
        registry.register::<PointLight>();
        let registration = registry
            .get(std::any::TypeId::of::<PointLight>())
            .expect("PointLight not registered");
        assert_eq!(
            registration.type_info().type_path(),
            "bsengine_core::light::PointLight"
        );
    }

    #[test]
    fn spot_light_inner_angle_less_than_outer() {
        let sl = SpotLight::default();
        assert!(
            sl.inner_angle_degrees.0 < sl.outer_angle_degrees.0,
            "inner must be narrower than outer"
        );
    }

    #[test]
    fn spot_light_inner_cos_greater_than_outer_cos() {
        let sl = SpotLight::default();
        assert!(
            sl.inner_angle_degrees.to_radians().cos() > sl.outer_angle_degrees.to_radians().cos(),
            "cos(inner) > cos(outer) because inner < outer"
        );
    }

    #[test]
    fn spot_light_is_registered_reflectable() {
        use bevy_reflect::TypeRegistry;
        let mut registry = TypeRegistry::default();
        registry.register::<SpotLight>();
        let registration = registry
            .get(std::any::TypeId::of::<SpotLight>())
            .expect("SpotLight not registered");
        assert_eq!(
            registration.type_info().type_path(),
            "bsengine_core::light::SpotLight"
        );
    }
}
