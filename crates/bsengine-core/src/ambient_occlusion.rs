use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Screen-space ambient occlusion (SSAO) post-processing applied by a camera.
/// Darkens surfaces in crevices and corners where indirect lighting is blocked.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct AmbientOcclusion {
    /// World-space radius used to sample occluders.
    pub radius: f32,
    /// Depth bias that prevents self-occlusion artifacts on flat surfaces.
    pub bias: f32,
    /// Strength of the darkening effect. 0 = off, 1 = full.
    pub intensity: f32,
    /// Number of hemisphere samples per pixel. Higher = quality; lower = performance.
    pub sample_count: u32,
    /// Whether the effect is applied at all.
    pub enabled: bool,
}

impl AmbientOcclusion {
    /// Creates an ambient occlusion setting with the default radius, bias, intensity, and sample count.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the occluder sampling radius, clamped to be non-negative.
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(0.0);
        self
    }

    /// Sets the depth bias, clamped to be non-negative.
    pub fn with_bias(mut self, bias: f32) -> Self {
        self.bias = bias.max(0.0);
        self
    }

    /// Sets the effect intensity, clamped to `[0, 1]`.
    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    /// Sets the hemisphere sample count, clamped to at least 1.
    pub fn with_sample_count(mut self, count: u32) -> Self {
        self.sample_count = count.max(1);
        self
    }

    /// Turns the effect off while keeping its other settings.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for AmbientOcclusion {
    fn default() -> Self {
        Self {
            radius: 0.5,
            bias: 0.025,
            intensity: 1.0,
            sample_count: 8,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambient_occlusion_defaults() {
        let ao = AmbientOcclusion::default();
        assert!((ao.radius - 0.5).abs() < 0.001);
        assert!((ao.bias - 0.025).abs() < 0.001);
        assert!((ao.intensity - 1.0).abs() < 0.001);
        assert_eq!(ao.sample_count, 8);
        assert!(ao.enabled);
    }

    #[test]
    fn intensity_clamped() {
        let ao = AmbientOcclusion::new().with_intensity(5.0);
        assert!((ao.intensity - 1.0).abs() < 0.001);
    }

    #[test]
    fn radius_clamped() {
        let ao = AmbientOcclusion::new().with_radius(-1.0);
        assert_eq!(ao.radius, 0.0);
    }

    #[test]
    fn sample_count_minimum_one() {
        let ao = AmbientOcclusion::new().with_sample_count(0);
        assert_eq!(ao.sample_count, 1);
    }

    #[test]
    fn disabled() {
        let ao = AmbientOcclusion::new().disabled();
        assert!(!ao.enabled);
    }
}
