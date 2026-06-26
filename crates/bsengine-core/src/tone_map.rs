use bevy_ecs::prelude::Component;

/// How high-dynamic-range luminance values are mapped to display range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToneMappingMode {
    /// No mapping — values above 1.0 are clipped.
    None,
    /// Simple Reinhard: `color / (color + 1)`. Perceptually soft rolloff.
    Reinhard,
    /// Luminance-based Reinhard — preserves hue better at high exposure.
    ReinhardLuminance,
    /// ACES film curve — cinematic contrast and pleasing rolloff.
    #[default]
    Aces,
    /// Generic S-curve with configurable toe/shoulder (filmic look).
    Filmic,
}

/// Tone-mapping post-processing applied by a camera.
/// Maps HDR render output to the display's LDR range before final output.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ToneMap {
    pub mode: ToneMappingMode,
    /// Camera exposure in EV (exposure value). 0.0 = no change; positive = brighter.
    pub exposure: f32,
    pub enabled: bool,
}

impl ToneMap {
    pub fn new(mode: ToneMappingMode) -> Self {
        Self {
            mode,
            exposure: 0.0,
            enabled: true,
        }
    }

    pub fn with_exposure(mut self, ev: f32) -> Self {
        self.exposure = ev;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns the linear multiplier that corresponds to the current EV exposure.
    /// EV +1 = 2× brighter, EV -1 = 0.5× brighter.
    pub fn exposure_multiplier(&self) -> f32 {
        2.0_f32.powf(self.exposure)
    }
}

impl Default for ToneMap {
    fn default() -> Self {
        Self::new(ToneMappingMode::Aces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tonemap_defaults() {
        let t = ToneMap::default();
        assert_eq!(t.mode, ToneMappingMode::Aces);
        assert_eq!(t.exposure, 0.0);
        assert!(t.enabled);
    }

    #[test]
    fn exposure_multiplier_zero() {
        let t = ToneMap::default();
        assert!((t.exposure_multiplier() - 1.0).abs() < 0.001);
    }

    #[test]
    fn exposure_multiplier_one_ev() {
        let t = ToneMap::default().with_exposure(1.0);
        assert!((t.exposure_multiplier() - 2.0).abs() < 0.001);
    }

    #[test]
    fn exposure_multiplier_minus_one_ev() {
        let t = ToneMap::default().with_exposure(-1.0);
        assert!((t.exposure_multiplier() - 0.5).abs() < 0.001);
    }

    #[test]
    fn tonemap_disabled() {
        let t = ToneMap::new(ToneMappingMode::Reinhard).disabled();
        assert!(!t.enabled);
        assert_eq!(t.mode, ToneMappingMode::Reinhard);
    }
}
