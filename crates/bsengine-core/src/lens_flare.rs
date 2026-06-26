use bevy_ecs::prelude::Component;

/// Controls the lens-flare post-effect produced by a bright light source.
/// Attach to a light or camera entity; the render system samples the element list
/// and composites flare sprites along the screen-space axis from light to screen center.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct LensFlare {
    /// Overall brightness of the flare effect. 0 = invisible, 1 = full.
    pub intensity: f32,
    /// Scale applied to all flare element sizes uniformly.
    pub scale: f32,
    /// Minimum dot-product between the camera forward vector and the light direction
    /// at which the flare starts to fade in. Range [0, 1]; 0 = always visible.
    pub fade_threshold: f32,
    /// Whether to test occlusion (hide flare when the source is behind geometry).
    pub check_occlusion: bool,
    pub enabled: bool,
}

impl LensFlare {
    pub fn new(intensity: f32) -> Self {
        Self {
            intensity: intensity.max(0.0),
            scale: 1.0,
            fade_threshold: 0.0,
            check_occlusion: true,
            enabled: true,
        }
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale.max(0.0);
        self
    }

    pub fn with_fade_threshold(mut self, threshold: f32) -> Self {
        self.fade_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn without_occlusion(mut self) -> Self {
        self.check_occlusion = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for LensFlare {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lens_flare_defaults() {
        let lf = LensFlare::default();
        assert!((lf.intensity - 1.0).abs() < 0.001);
        assert!((lf.scale - 1.0).abs() < 0.001);
        assert_eq!(lf.fade_threshold, 0.0);
        assert!(lf.check_occlusion);
        assert!(lf.enabled);
    }

    #[test]
    fn intensity_clamped() {
        let lf = LensFlare::new(-3.0);
        assert_eq!(lf.intensity, 0.0);
    }

    #[test]
    fn scale_clamped() {
        let lf = LensFlare::new(1.0).with_scale(-0.5);
        assert_eq!(lf.scale, 0.0);
    }

    #[test]
    fn fade_threshold_clamped() {
        let lf = LensFlare::new(1.0).with_fade_threshold(2.0);
        assert!((lf.fade_threshold - 1.0).abs() < 0.001);
    }

    #[test]
    fn without_occlusion() {
        let lf = LensFlare::new(0.8).without_occlusion();
        assert!(!lf.check_occlusion);
    }
}
