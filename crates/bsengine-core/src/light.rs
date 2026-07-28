use crate::reflect_color::ReflectColor;
use crate::reflect_degrees::ReflectDegrees;
use crate::reflect_validate::{ReflectValidate, Validate};
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
/// Parallel-ray light source (e.g. sunlight) with no position or falloff;
/// its direction comes from the entity's `Transform` rotation.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct DirectionalLight {
    /// Light color, applied to all surfaces facing the light direction.
    pub color: ReflectColor,
    /// Constant ambient light color added regardless of surface orientation.
    pub ambient: ReflectColor,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            color: Vec3::ONE.into(),
            ambient: Vec3::splat(0.15).into(),
        }
    }
}

/// Omnidirectional light source radiating equally in all directions from the
/// entity's position, falling off to zero at `range`.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct PointLight {
    /// Light color.
    pub color: ReflectColor,
    /// Overall brightness multiplier.
    pub intensity: f32,
    /// Distance at which the light's contribution falls off to zero.
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

/// Cone-shaped light source radiating from the entity's position in the
/// direction of its `Transform` rotation, falling off between the inner and
/// outer cone angles and to zero at `range`.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default, Validate)]
pub struct SpotLight {
    /// Light color.
    pub color: ReflectColor,
    /// Overall brightness multiplier.
    pub intensity: f32,
    /// Distance at which the light's contribution falls off to zero.
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

impl Validate for SpotLight {
    fn validate(&mut self) {
        // Matches the original hand-built Inspector widget's bounds exactly
        // (DragValue::range(0.0..=outer) for inner, range(inner..=89.0) for
        // outer) — clamp outer to its absolute bound first, then clamp inner
        // against outer's now-final value, replicating the two-way
        // constraint the old widgets enforced live while dragging.
        self.outer_angle_degrees.0 = self.outer_angle_degrees.0.clamp(0.0, 89.0);
        self.inner_angle_degrees.0 = self
            .inner_angle_degrees
            .0
            .clamp(0.0, self.outer_angle_degrees.0);
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

    #[test]
    fn spot_light_validate_clamps_inner_to_outer() {
        let mut sl = SpotLight {
            inner_angle_degrees: 50.0.into(),
            outer_angle_degrees: 30.0.into(),
            ..SpotLight::default()
        };
        sl.validate();
        assert!(
            (sl.inner_angle_degrees.0 - 30.0).abs() < 1e-6,
            "inner should be pulled down to outer when it starts out larger"
        );
        assert!(
            (sl.outer_angle_degrees.0 - 30.0).abs() < 1e-6,
            "outer should be untouched when already within [0, 89]"
        );
    }

    #[test]
    fn spot_light_validate_clamps_outer_to_89_and_inner_follows() {
        let mut sl = SpotLight {
            inner_angle_degrees: 50.0.into(),
            outer_angle_degrees: 120.0.into(),
            ..SpotLight::default()
        };
        sl.validate();
        assert!(
            (sl.outer_angle_degrees.0 - 89.0).abs() < 1e-6,
            "outer should be clamped to the 89 degree ceiling"
        );
        assert!(
            (sl.inner_angle_degrees.0 - 50.0).abs() < 1e-6,
            "inner (50) is already <= outer's clamped value (89), so it stays unchanged"
        );
    }

    #[test]
    fn spot_light_validate_leaves_already_valid_angles_unchanged() {
        let mut sl = SpotLight::default();
        let (inner_before, outer_before) = (sl.inner_angle_degrees.0, sl.outer_angle_degrees.0);
        sl.validate();
        assert_eq!(sl.inner_angle_degrees.0, inner_before);
        assert_eq!(sl.outer_angle_degrees.0, outer_before);
    }
}
