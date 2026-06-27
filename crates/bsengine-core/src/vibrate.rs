use bevy_ecs::prelude::Component;

/// Per-entity positional oscillation effect.
///
/// Unlike `ScreenShake` (camera-space trembling), `Vibrate` is a world-space
/// offset applied directly to the entity's transform — useful for bomb countdowns,
/// hit stagger on a character model, proximity rumble, or resonating obstacles.
///
/// The rendering system reads `offset(t)` each frame (passing elapsed game time)
/// to compute a sine-wave displacement along `axis`. The displacement decays
/// linearly at `decay_rate` per second, letting vibrations fade out naturally.
///
/// `trigger(amplitude, frequency, duration)` starts or resets a vibration.
/// `tick(dt)` advances the timer and applies decay.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vibrate {
    /// Peak displacement in world units along `axis`.
    pub amplitude: f32,
    /// Oscillation frequency in Hz.
    pub frequency: f32,
    /// Remaining vibration duration in seconds.
    pub duration: f32,
    /// Counts up; used externally for phase calculation.
    pub elapsed: f32,
    /// How fast amplitude decays per second (0 = constant amplitude).
    pub decay_rate: f32,
    /// Normalized displacement axis (default X).
    pub axis: [f32; 3],
    /// True on the first frame a vibration begins.
    pub just_started: bool,
    /// True on the first frame the vibration stops.
    pub just_stopped: bool,
    pub enabled: bool,
}

impl Vibrate {
    pub fn new() -> Self {
        Self {
            amplitude: 0.0,
            frequency: 10.0,
            duration: 0.0,
            elapsed: 0.0,
            decay_rate: 0.0,
            axis: [1.0, 0.0, 0.0],
            just_started: false,
            just_stopped: false,
            enabled: true,
        }
    }

    /// Start or reset a vibration.
    pub fn trigger(&mut self, amplitude: f32, frequency: f32, duration: f32) {
        if !self.enabled {
            return;
        }
        self.amplitude = amplitude.max(0.0);
        self.frequency = frequency.max(0.0);
        self.duration = duration.max(0.0);
        self.elapsed = 0.0;
        self.just_started = true;
    }

    pub fn with_decay(mut self, rate: f32) -> Self {
        self.decay_rate = rate.max(0.0);
        self
    }

    pub fn with_axis(mut self, x: f32, y: f32, z: f32) -> Self {
        let len = (x * x + y * y + z * z).sqrt().max(f32::EPSILON);
        self.axis = [x / len, y / len, z / len];
        self
    }

    /// Advance the vibration timer and apply amplitude decay.
    pub fn tick(&mut self, dt: f32) {
        self.just_started = false;
        self.just_stopped = false;

        if !self.is_active() {
            return;
        }

        self.elapsed += dt;
        self.duration -= dt;

        if self.decay_rate > 0.0 {
            self.amplitude = (self.amplitude - self.decay_rate * dt).max(0.0);
        }

        if self.duration <= 0.0 || self.amplitude <= 0.0 {
            self.duration = 0.0;
            self.amplitude = 0.0;
            self.just_stopped = true;
        }
    }

    /// Stop the vibration immediately.
    pub fn stop(&mut self) {
        if self.is_active() {
            self.duration = 0.0;
            self.amplitude = 0.0;
            self.just_stopped = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.duration > 0.0 && self.amplitude > 0.0
    }

    /// Current displacement offset as `[f32; 3]` (axis * amplitude * sin(phase)).
    ///
    /// Pass in the current elapsed time for phase continuity across frames.
    pub fn offset(&self, time: f32) -> [f32; 3] {
        if !self.is_active() {
            return [0.0, 0.0, 0.0];
        }
        let d = self.amplitude * (2.0 * std::f32::consts::PI * self.frequency * time).sin();
        [self.axis[0] * d, self.axis[1] * d, self.axis[2] * d]
    }
}

impl Default for Vibrate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_starts_vibration() {
        let mut v = Vibrate::new();
        v.trigger(0.5, 10.0, 2.0);
        assert!(v.is_active());
        assert!(v.just_started);
        assert!((v.amplitude - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tick_advances_elapsed() {
        let mut v = Vibrate::new();
        v.trigger(0.5, 10.0, 2.0);
        v.tick(0.5);
        assert!((v.elapsed - 0.5).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_at_duration() {
        let mut v = Vibrate::new();
        v.trigger(0.5, 10.0, 1.0);
        v.tick(1.1);
        assert!(!v.is_active());
        assert!(v.just_stopped);
    }

    #[test]
    fn decay_reduces_amplitude() {
        let mut v = Vibrate::new().with_decay(0.5);
        v.trigger(1.0, 10.0, 5.0);
        v.tick(1.0);
        assert!((v.amplitude - 0.5).abs() < 1e-4);
    }

    #[test]
    fn decay_stops_at_zero_amplitude() {
        let mut v = Vibrate::new().with_decay(2.0);
        v.trigger(1.0, 10.0, 5.0);
        v.tick(1.0); // amplitude → 0
        assert!(!v.is_active());
        assert!(v.just_stopped);
    }

    #[test]
    fn stop_ends_vibration() {
        let mut v = Vibrate::new();
        v.trigger(0.5, 10.0, 5.0);
        v.stop();
        assert!(!v.is_active());
        assert!(v.just_stopped);
    }

    #[test]
    fn disabled_ignores_trigger() {
        let mut v = Vibrate::new();
        v.enabled = false;
        v.trigger(0.5, 10.0, 2.0);
        assert!(!v.is_active());
    }

    #[test]
    fn offset_zero_when_inactive() {
        let v = Vibrate::new();
        let off = v.offset(0.5);
        assert_eq!(off, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn axis_normalization() {
        let v = Vibrate::new().with_axis(0.0, 1.0, 0.0);
        assert!((v.axis[1] - 1.0).abs() < 1e-5);
    }
}
