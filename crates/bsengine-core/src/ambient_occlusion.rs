use bevy_ecs::prelude::Component;

/// Screen-space ambient occlusion (SSAO) post-processing applied by a camera.
/// Darkens surfaces in crevices and corners where indirect lighting is blocked.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AmbientOcclusion {
    /// World-space radius used to sample occluders.
    pub radius: f32,
    /// Depth bias that prevents self-occlusion artifacts on flat surfaces.
    pub bias: f32,
    /// Strength of the darkening effect. 0 = off, 1 = full.
    pub intensity: f32,
    /// Number of hemisphere samples per pixel. Higher = quality; lower = performance.
    pub sample_count: u32,
    pub enabled: bool,
}

impl AmbientOcclusion {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(0.0);
        self
    }

    pub fn with_bias(mut self, bias: f32) -> Self {
        self.bias = bias.max(0.0);
        self
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    pub fn with_sample_count(mut self, count: u32) -> Self {
        self.sample_count = count.max(1);
        self
    }

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
