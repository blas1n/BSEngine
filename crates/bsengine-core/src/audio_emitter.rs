use bevy_ecs::prelude::Component;

/// Spatial audio emitter properties. Attach alongside a `Transform` so the
/// audio system can compute distance-based attenuation.
///
/// Volume at distance `d` from listener:
///   `effective = volume * (1 - clamp((d - min_distance) / (max_distance - min_distance), 0, 1))`
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AudioEmitter {
    /// Linear volume scale at `min_distance` or closer (0.0 = silent, 1.0 = full).
    pub volume: f32,
    /// Distance at which the sound is heard at full volume.
    pub min_distance: f32,
    /// Distance beyond which the sound is completely inaudible.
    pub max_distance: f32,
}

impl AudioEmitter {
    pub fn new(volume: f32, min_distance: f32, max_distance: f32) -> Self {
        Self {
            volume: volume.clamp(0.0, 1.0),
            min_distance: min_distance.max(0.0),
            max_distance: max_distance.max(min_distance),
        }
    }

    /// Compute the effective (attenuated) volume at `distance` from the listener.
    pub fn volume_at(&self, distance: f32) -> f32 {
        if distance <= self.min_distance {
            return self.volume;
        }
        if distance >= self.max_distance {
            return 0.0;
        }
        let range = self.max_distance - self.min_distance;
        let t = (distance - self.min_distance) / range;
        self.volume * (1.0 - t)
    }
}

impl Default for AudioEmitter {
    fn default() -> Self {
        Self {
            volume: 1.0,
            min_distance: 1.0,
            max_distance: 20.0,
        }
    }
}

/// Marks the entity that acts as the listener for spatial audio.
/// Only one `AudioListener` should be active at a time; if multiple exist,
/// the audio system picks the first one found.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AudioListener;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_at_min_distance_is_full() {
        let e = AudioEmitter::new(1.0, 5.0, 20.0);
        assert!((e.volume_at(5.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn volume_at_max_distance_is_zero() {
        let e = AudioEmitter::new(1.0, 5.0, 20.0);
        assert_eq!(e.volume_at(20.0), 0.0);
    }

    #[test]
    fn volume_beyond_max_is_zero() {
        let e = AudioEmitter::new(1.0, 5.0, 20.0);
        assert_eq!(e.volume_at(100.0), 0.0);
    }

    #[test]
    fn volume_at_midpoint_is_half() {
        let e = AudioEmitter::new(1.0, 0.0, 20.0);
        assert!((e.volume_at(10.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn volume_clamped_to_range() {
        let e = AudioEmitter::new(2.0, 1.0, 10.0);
        assert_eq!(e.volume, 1.0);
    }

    #[test]
    fn max_distance_ge_min() {
        let e = AudioEmitter::new(1.0, 10.0, 5.0); // max < min → clamped
        assert!(e.max_distance >= e.min_distance);
    }

    #[test]
    fn default_emitter_full_volume_at_origin() {
        let e = AudioEmitter::default();
        assert!((e.volume_at(0.0) - 1.0).abs() < 0.001);
    }
}
