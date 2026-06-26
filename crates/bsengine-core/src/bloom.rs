use bevy_ecs::prelude::Component;

/// Post-processing bloom effect applied by a camera.
/// Bright areas above `threshold` glow outward by `radius` pixels.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Bloom {
    /// Overall bloom brightness multiplier.
    pub intensity: f32,
    /// Luminance threshold — only pixels brighter than this contribute to bloom.
    pub threshold: f32,
    /// How far the bloom spreads in screen pixels.
    pub radius: f32,
    /// Controls the softness of the threshold knee.
    /// 0.0 = hard cutoff, 1.0 = smooth transition.
    pub softness: f32,
    pub enabled: bool,
}

impl Bloom {
    pub fn new(intensity: f32) -> Self {
        Self {
            intensity: intensity.max(0.0),
            threshold: 1.0,
            radius: 4.0,
            softness: 0.5,
            enabled: true,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.max(0.0);
        self
    }

    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(0.0);
        self
    }

    pub fn with_softness(mut self, softness: f32) -> Self {
        self.softness = softness.clamp(0.0, 1.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for Bloom {
    fn default() -> Self {
        Self::new(0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bloom_defaults() {
        let b = Bloom::default();
        assert!((b.intensity - 0.3).abs() < 0.001);
        assert!((b.threshold - 1.0).abs() < 0.001);
        assert!((b.radius - 4.0).abs() < 0.001);
        assert!((b.softness - 0.5).abs() < 0.001);
        assert!(b.enabled);
    }

    #[test]
    fn bloom_negative_intensity_clamped() {
        let b = Bloom::new(-1.0);
        assert_eq!(b.intensity, 0.0);
    }

    #[test]
    fn bloom_builder_fields() {
        let b = Bloom::new(1.0)
            .with_threshold(0.8)
            .with_radius(8.0)
            .with_softness(0.2);
        assert!((b.threshold - 0.8).abs() < 0.001);
        assert!((b.radius - 8.0).abs() < 0.001);
        assert!((b.softness - 0.2).abs() < 0.001);
    }

    #[test]
    fn bloom_softness_clamped() {
        let b = Bloom::new(1.0).with_softness(2.0);
        assert!((b.softness - 1.0).abs() < 0.001);
    }

    #[test]
    fn bloom_disabled() {
        let b = Bloom::default().disabled();
        assert!(!b.enabled);
    }
}
