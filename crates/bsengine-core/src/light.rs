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

#[derive(Component, Debug, Clone)]
pub struct PointLight {
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            intensity: 1.0,
            range: 10.0,
        }
    }
}

#[derive(Component, Debug, Clone)]
pub struct SpotLight {
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    /// Inner cone half-angle (radians) — full brightness inside.
    pub inner_angle: f32,
    /// Outer cone half-angle (radians) — zero brightness outside.
    pub outer_angle: f32,
}

impl Default for SpotLight {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            intensity: 1.0,
            range: 10.0,
            inner_angle: std::f32::consts::PI / 8.0, // 22.5°
            outer_angle: std::f32::consts::PI / 6.0, // 30°
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

    #[test]
    fn point_light_default_values() {
        let pl = PointLight::default();
        assert_eq!(pl.color, Vec3::ONE);
        assert!((pl.intensity - 1.0).abs() < 1e-6);
        assert!((pl.range - 10.0).abs() < 1e-6);
    }

    #[test]
    fn spot_light_inner_angle_less_than_outer() {
        let sl = SpotLight::default();
        assert!(
            sl.inner_angle < sl.outer_angle,
            "inner must be narrower than outer"
        );
    }

    #[test]
    fn spot_light_inner_cos_greater_than_outer_cos() {
        let sl = SpotLight::default();
        assert!(
            sl.inner_angle.cos() > sl.outer_angle.cos(),
            "cos(inner) > cos(outer) because inner < outer"
        );
    }
}
