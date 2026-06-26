use bevy_ecs::prelude::Component;

/// Chromatic aberration (color fringing) post-processing applied by a camera.
/// Simulates the lens distortion where different wavelengths of light focus at different points,
/// producing red/green/blue fringing at high-contrast edges.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ChromaticAberration {
    /// Radial intensity of the RGB channel separation in UV-space units.
    /// 0 = off, 0.01–0.05 = subtle film look, >0.1 = very distorted.
    pub intensity: f32,
    pub enabled: bool,
}

impl ChromaticAberration {
    pub fn new(intensity: f32) -> Self {
        Self {
            intensity: intensity.max(0.0),
            enabled: true,
        }
    }

    pub fn subtle() -> Self {
        Self::new(0.02)
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for ChromaticAberration {
    fn default() -> Self {
        Self::subtle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtle_defaults() {
        let ca = ChromaticAberration::default();
        assert!((ca.intensity - 0.02).abs() < 0.001);
        assert!(ca.enabled);
    }

    #[test]
    fn intensity_clamped() {
        let ca = ChromaticAberration::new(-1.0);
        assert_eq!(ca.intensity, 0.0);
    }

    #[test]
    fn builder_intensity() {
        let ca = ChromaticAberration::default().with_intensity(0.05);
        assert!((ca.intensity - 0.05).abs() < 0.001);
    }

    #[test]
    fn disabled_flag() {
        let ca = ChromaticAberration::subtle().disabled();
        assert!(!ca.enabled);
    }
}
