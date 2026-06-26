use bevy_ecs::prelude::Component;

/// Enables volumetric/god-ray scattering for an attached light.
/// The render system marches rays through the scene volume and accumulates
/// scattering based on this component's parameters.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct VolumetricLight {
    /// Scattering coefficient — how much light scatters per unit distance. >= 0.
    pub scattering: f32,
    /// Absorption coefficient — how much light is absorbed per unit distance. >= 0.
    pub absorption: f32,
    /// Anisotropy of the scattering phase function in [-1, 1].
    /// 0 = isotropic, +1 = strong forward scatter (sunbeams), -1 = backscatter.
    pub anisotropy: f32,
    /// Number of ray-march steps. Higher = better quality, higher cost.
    pub step_count: u32,
    /// Maximum distance along the ray that is marched for this light.
    pub max_distance: f32,
    pub enabled: bool,
}

impl VolumetricLight {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_scattering(mut self, scattering: f32) -> Self {
        self.scattering = scattering.max(0.0);
        self
    }

    pub fn with_absorption(mut self, absorption: f32) -> Self {
        self.absorption = absorption.max(0.0);
        self
    }

    pub fn with_anisotropy(mut self, anisotropy: f32) -> Self {
        self.anisotropy = anisotropy.clamp(-1.0, 1.0);
        self
    }

    pub fn with_step_count(mut self, count: u32) -> Self {
        self.step_count = count.max(1);
        self
    }

    pub fn with_max_distance(mut self, distance: f32) -> Self {
        self.max_distance = distance.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for VolumetricLight {
    fn default() -> Self {
        Self {
            scattering: 0.1,
            absorption: 0.01,
            anisotropy: 0.5,
            step_count: 32,
            max_distance: 50.0,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volumetric_light_defaults() {
        let vl = VolumetricLight::default();
        assert!((vl.scattering - 0.1).abs() < 0.001);
        assert!((vl.absorption - 0.01).abs() < 0.001);
        assert!((vl.anisotropy - 0.5).abs() < 0.001);
        assert_eq!(vl.step_count, 32);
        assert!((vl.max_distance - 50.0).abs() < 0.001);
        assert!(vl.enabled);
    }

    #[test]
    fn scattering_clamped() {
        let vl = VolumetricLight::new().with_scattering(-1.0);
        assert_eq!(vl.scattering, 0.0);
    }

    #[test]
    fn anisotropy_clamped() {
        let vl = VolumetricLight::new().with_anisotropy(2.0);
        assert!((vl.anisotropy - 1.0).abs() < 0.001);
    }

    #[test]
    fn step_count_min_one() {
        let vl = VolumetricLight::new().with_step_count(0);
        assert_eq!(vl.step_count, 1);
    }

    #[test]
    fn disabled_flag() {
        let vl = VolumetricLight::new().disabled();
        assert!(!vl.enabled);
    }
}
