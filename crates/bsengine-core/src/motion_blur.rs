use bevy_ecs::prelude::Component;

/// Per-camera motion blur post-processing.
/// Simulates the blur of a real camera shutter by accumulating samples along velocity vectors.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MotionBlur {
    /// Shutter angle in degrees. 0 = no blur; 360 = one full frame of exposure.
    /// Film typically uses 180°.
    pub shutter_angle: f32,
    /// Number of samples taken along each pixel's velocity vector.
    /// More samples = smoother result but higher cost.
    pub sample_count: u32,
    pub enabled: bool,
}

impl MotionBlur {
    pub fn new(shutter_angle: f32) -> Self {
        Self {
            shutter_angle: shutter_angle.clamp(0.0, 360.0),
            sample_count: 8,
            enabled: true,
        }
    }

    pub fn cinematic() -> Self {
        Self::new(180.0)
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

impl Default for MotionBlur {
    fn default() -> Self {
        Self::cinematic()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cinematic_defaults() {
        let mb = MotionBlur::default();
        assert!((mb.shutter_angle - 180.0).abs() < 0.001);
        assert_eq!(mb.sample_count, 8);
        assert!(mb.enabled);
    }

    #[test]
    fn shutter_angle_clamped() {
        let mb = MotionBlur::new(720.0);
        assert!((mb.shutter_angle - 360.0).abs() < 0.001);
        let mb2 = MotionBlur::new(-10.0);
        assert_eq!(mb2.shutter_angle, 0.0);
    }

    #[test]
    fn sample_count_minimum_one() {
        let mb = MotionBlur::cinematic().with_sample_count(0);
        assert_eq!(mb.sample_count, 1);
    }

    #[test]
    fn custom_sample_count() {
        let mb = MotionBlur::cinematic().with_sample_count(16);
        assert_eq!(mb.sample_count, 16);
    }

    #[test]
    fn disabled_flag() {
        let mb = MotionBlur::cinematic().disabled();
        assert!(!mb.enabled);
    }
}
