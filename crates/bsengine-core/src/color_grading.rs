use bevy_ecs::prelude::Component;

/// Per-camera color grading post-processing.
/// Adjustments are applied after tone-mapping in the order:
/// exposure → contrast → brightness → saturation → hue shift → LUT.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ColorGrading {
    /// Path to a 3D LUT texture applied as the final color transform step.
    /// `None` = no LUT.
    pub lut_path: Option<String>,
    /// Exposure adjustment in EV. 0 = no change; ±1 = one stop brighter/darker.
    pub exposure: f32,
    /// Contrast adjustment. 0 = no change, 1 = maximum contrast, -1 = flat grey.
    pub contrast: f32,
    /// Saturation scale. 1 = no change, 0 = greyscale, >1 = boosted.
    pub saturation: f32,
    /// Hue rotation in degrees in [-180, 180].
    pub hue_shift: f32,
    /// Additive brightness lift applied before LUT. 0 = no change.
    pub brightness: f32,
    pub enabled: bool,
}

impl ColorGrading {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_lut(mut self, path: impl Into<String>) -> Self {
        self.lut_path = Some(path.into());
        self
    }

    pub fn with_exposure(mut self, ev: f32) -> Self {
        self.exposure = ev;
        self
    }

    pub fn with_contrast(mut self, contrast: f32) -> Self {
        self.contrast = contrast.clamp(-1.0, 1.0);
        self
    }

    pub fn with_saturation(mut self, saturation: f32) -> Self {
        self.saturation = saturation.max(0.0);
        self
    }

    pub fn with_hue_shift(mut self, degrees: f32) -> Self {
        self.hue_shift = degrees.clamp(-180.0, 180.0);
        self
    }

    pub fn with_brightness(mut self, brightness: f32) -> Self {
        self.brightness = brightness;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for ColorGrading {
    fn default() -> Self {
        Self {
            lut_path: None,
            exposure: 0.0,
            contrast: 0.0,
            saturation: 1.0,
            hue_shift: 0.0,
            brightness: 0.0,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_grading_defaults() {
        let cg = ColorGrading::default();
        assert!(cg.lut_path.is_none());
        assert_eq!(cg.exposure, 0.0);
        assert_eq!(cg.contrast, 0.0);
        assert!((cg.saturation - 1.0).abs() < 0.001);
        assert_eq!(cg.hue_shift, 0.0);
        assert!(cg.enabled);
    }

    #[test]
    fn contrast_clamped() {
        let cg = ColorGrading::new().with_contrast(5.0);
        assert!((cg.contrast - 1.0).abs() < 0.001);
    }

    #[test]
    fn saturation_clamped() {
        let cg = ColorGrading::new().with_saturation(-2.0);
        assert_eq!(cg.saturation, 0.0);
    }

    #[test]
    fn hue_shift_clamped() {
        let cg = ColorGrading::new().with_hue_shift(270.0);
        assert!((cg.hue_shift - 180.0).abs() < 0.001);
    }

    #[test]
    fn with_lut_sets_path() {
        let cg = ColorGrading::new().with_lut("grades/cinematic.png");
        assert_eq!(cg.lut_path.as_deref(), Some("grades/cinematic.png"));
    }
}
