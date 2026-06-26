use bevy_ecs::prelude::Component;

/// Surface material that determines which footstep sounds and VFX to trigger.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SurfaceKind {
    #[default]
    Concrete,
    Grass,
    Wood,
    Metal,
    Sand,
    Water,
    Gravel,
    Custom(String),
}

/// Drives footstep sound and particle events for a character.
/// The movement system writes `distance_accumulated` each frame;
/// the audio/VFX system reads this and fires events at `step_interval`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Footstep {
    /// Distance in metres between each footstep event.
    pub step_interval: f32,
    /// Metres walked since the last step event fired.
    pub distance_accumulated: f32,
    /// Sound volume multiplier for footstep audio (0 = silent).
    pub volume: f32,
    /// Base path for footstep audio assets (e.g. `"sounds/footsteps/"`).
    pub audio_prefix: String,
    /// Current surface under the entity.
    pub surface: SurfaceKind,
    /// Minimum entity speed (m/s) below which steps are suppressed.
    pub min_speed: f32,
    pub enabled: bool,
}

impl Footstep {
    pub fn new(step_interval: f32) -> Self {
        Self {
            step_interval: step_interval.max(0.01),
            distance_accumulated: 0.0,
            volume: 1.0,
            audio_prefix: String::from("sounds/footsteps/"),
            surface: SurfaceKind::Concrete,
            min_speed: 0.1,
            enabled: true,
        }
    }

    pub fn with_surface(mut self, surface: SurfaceKind) -> Self {
        self.surface = surface;
        self
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.max(0.0);
        self
    }

    pub fn silent(mut self) -> Self {
        self.volume = 0.0;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance the step accumulator by `distance` metres at the given `speed`.
    /// Returns `true` each time a step event should fire.
    pub fn advance(&mut self, distance: f32, speed: f32) -> bool {
        if !self.enabled || speed < self.min_speed {
            return false;
        }
        self.distance_accumulated += distance.max(0.0);
        if self.distance_accumulated >= self.step_interval {
            self.distance_accumulated -= self.step_interval;
            return true;
        }
        false
    }

    /// Reset accumulated distance (e.g. on teleport or spawn).
    pub fn reset(&mut self) {
        self.distance_accumulated = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footstep_defaults() {
        let f = Footstep::new(0.6);
        assert!((f.step_interval - 0.6).abs() < 0.001);
        assert_eq!(f.distance_accumulated, 0.0);
        assert!(f.enabled);
    }

    #[test]
    fn advance_fires_at_interval() {
        let mut f = Footstep::new(1.0);
        assert!(!f.advance(0.5, 2.0));
        assert!(f.advance(0.5, 2.0));
    }

    #[test]
    fn advance_suppressed_below_min_speed() {
        let mut f = Footstep::new(0.5);
        assert!(!f.advance(1.0, 0.05)); // below min_speed 0.1
    }

    #[test]
    fn reset_clears_accumulator() {
        let mut f = Footstep::new(1.0);
        f.advance(0.8, 2.0);
        f.reset();
        assert_eq!(f.distance_accumulated, 0.0);
    }

    #[test]
    fn disabled_never_fires() {
        let mut f = Footstep::new(0.5).disabled();
        assert!(!f.advance(10.0, 5.0));
    }
}
