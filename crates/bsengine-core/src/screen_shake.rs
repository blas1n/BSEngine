use bevy_ecs::prelude::Component;

/// Applies a decaying positional shake to the attached camera.
/// The shake system reads this component each frame, offsets the camera,
/// and decrements `remaining` until the trauma reaches zero.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ScreenShake {
    /// Current trauma in [0, 1]. Camera offset = `amplitude * trauma^2`.
    pub trauma: f32,
    /// Maximum translational offset in world units at full trauma.
    pub amplitude: f32,
    /// How fast trauma decays per second.
    pub decay_rate: f32,
    /// Frequency of the shake noise oscillation (Hz).
    pub frequency: f32,
}

impl ScreenShake {
    pub fn new(amplitude: f32) -> Self {
        Self {
            trauma: 0.0,
            amplitude: amplitude.max(0.0),
            decay_rate: 1.5,
            frequency: 8.0,
        }
    }

    /// Convenience: create a ScreenShake pre-loaded with the given trauma amount.
    pub fn with_trauma(amplitude: f32, trauma: f32) -> Self {
        Self::new(amplitude).add_trauma(trauma)
    }

    pub fn with_decay_rate(mut self, rate: f32) -> Self {
        self.decay_rate = rate.max(0.0);
        self
    }

    pub fn with_frequency(mut self, hz: f32) -> Self {
        self.frequency = hz.max(0.0);
        self
    }

    /// Adds trauma, clamped so total does not exceed 1.
    #[must_use]
    pub fn add_trauma(mut self, amount: f32) -> Self {
        self.trauma = (self.trauma + amount.max(0.0)).min(1.0);
        self
    }

    /// Advance trauma decay by `dt` seconds.
    /// Call each frame from the camera update system.
    pub fn tick(&mut self, dt: f32) {
        self.trauma = (self.trauma - self.decay_rate * dt).max(0.0);
    }

    /// Returns `true` when the shake has fully decayed.
    pub fn is_done(&self) -> bool {
        self.trauma == 0.0
    }

    /// Current displacement intensity — `trauma²` so the curve feels snappy.
    pub fn intensity(&self) -> f32 {
        self.trauma * self.trauma
    }
}

impl Default for ScreenShake {
    fn default() -> Self {
        Self::new(0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_shake_defaults() {
        let ss = ScreenShake::default();
        assert_eq!(ss.trauma, 0.0);
        assert!((ss.amplitude - 0.3).abs() < 0.001);
        assert!(ss.is_done());
    }

    #[test]
    fn trauma_clamped_to_one() {
        let ss = ScreenShake::with_trauma(0.5, 2.0);
        assert!((ss.trauma - 1.0).abs() < 0.001);
    }

    #[test]
    fn intensity_is_trauma_squared() {
        let mut ss = ScreenShake::new(1.0);
        ss.trauma = 0.5;
        assert!((ss.intensity() - 0.25).abs() < 0.001);
    }

    #[test]
    fn tick_decays_trauma() {
        let mut ss = ScreenShake::with_trauma(1.0, 1.0);
        ss.tick(0.5);
        assert!((ss.trauma - 0.25).abs() < 0.01);
    }

    #[test]
    fn trauma_never_goes_negative() {
        let mut ss = ScreenShake::with_trauma(1.0, 0.1);
        ss.tick(10.0);
        assert_eq!(ss.trauma, 0.0);
        assert!(ss.is_done());
    }
}
