use bevy_ecs::prelude::Component;

/// What kind of DSP filter to apply to an audio source.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FilterType {
    /// Attenuates frequencies above `cutoff_hz`. Default choice for muffled sound.
    #[default]
    LowPass,
    /// Attenuates frequencies below `cutoff_hz`. Useful for radio or wall-muffled effects.
    HighPass,
    /// Passes a band around `cutoff_hz`; `bandwidth_hz` sets the width.
    BandPass,
    /// Notch (band-reject) filter — removes a narrow frequency band.
    Notch,
}

/// Real-time DSP filter applied to a specific `AudioEmitter` entity.
/// Attach alongside `AudioEmitter` to shape the tonal character of a sound.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AudioFilter {
    pub filter_type: FilterType,
    /// Center/cutoff frequency in Hz. Must be > 0.
    pub cutoff_hz: f32,
    /// Resonance (Q factor). Higher values create a more pronounced peak at the cutoff.
    /// Clamped to [0.01, 100.0].
    pub resonance: f32,
    /// For BandPass/Notch: frequency bandwidth in Hz.
    pub bandwidth_hz: f32,
    /// Output gain applied after filtering [0, 4]. 1.0 = unity.
    pub gain: f32,
    pub enabled: bool,
}

impl AudioFilter {
    pub fn low_pass(cutoff_hz: f32) -> Self {
        Self {
            filter_type: FilterType::LowPass,
            cutoff_hz: cutoff_hz.max(1.0),
            ..Self::default()
        }
    }

    pub fn high_pass(cutoff_hz: f32) -> Self {
        Self {
            filter_type: FilterType::HighPass,
            cutoff_hz: cutoff_hz.max(1.0),
            ..Self::default()
        }
    }

    pub fn with_resonance(mut self, q: f32) -> Self {
        self.resonance = q.clamp(0.01, 100.0);
        self
    }

    pub fn with_gain(mut self, gain: f32) -> Self {
        self.gain = gain.clamp(0.0, 4.0);
        self
    }

    pub fn with_bandwidth(mut self, hz: f32) -> Self {
        self.bandwidth_hz = hz.max(1.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for AudioFilter {
    fn default() -> Self {
        Self {
            filter_type: FilterType::LowPass,
            cutoff_hz: 1000.0,
            resonance: 0.707,
            bandwidth_hz: 100.0,
            gain: 1.0,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_filter_defaults() {
        let f = AudioFilter::default();
        assert_eq!(f.filter_type, FilterType::LowPass);
        assert!((f.cutoff_hz - 1000.0).abs() < 0.001);
        assert!((f.gain - 1.0).abs() < 0.001);
        assert!(f.enabled);
    }

    #[test]
    fn low_pass_constructor() {
        let f = AudioFilter::low_pass(500.0);
        assert_eq!(f.filter_type, FilterType::LowPass);
        assert!((f.cutoff_hz - 500.0).abs() < 0.001);
    }

    #[test]
    fn resonance_clamped() {
        let f = AudioFilter::low_pass(1000.0).with_resonance(200.0);
        assert!((f.resonance - 100.0).abs() < 0.001);
    }

    #[test]
    fn gain_clamped() {
        let f = AudioFilter::high_pass(200.0).with_gain(-1.0);
        assert_eq!(f.gain, 0.0);
    }

    #[test]
    fn disabled_flag() {
        let f = AudioFilter::low_pass(800.0).disabled();
        assert!(!f.enabled);
    }
}
