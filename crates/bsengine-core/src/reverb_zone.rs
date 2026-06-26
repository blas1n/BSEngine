use bevy_ecs::prelude::Component;

/// Spatial audio reverb zone — applies room-acoustic preset when the listener
/// enters the bounding volume (sphere of `radius` around the entity).
/// Zones are blended by the audio system when multiple overlap; `priority`
/// decides which contributes most.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ReverbZone {
    /// Influence radius in world units. The reverb blends in as the listener enters.
    pub radius: f32,
    /// Distance from the surface at which blending begins. Must be <= `radius`.
    pub blend_distance: f32,
    /// Reverb preset name. Common values: `"none"`, `"room"`, `"hallway"`,
    /// `"cave"`, `"concert_hall"`, `"underwater"`.
    pub preset: String,
    /// Room size in [0, 1]. 0 = tiny, 1 = enormous cavern.
    pub room_size: f32,
    /// Decay time in seconds. Longer = more echo.
    pub decay_time: f32,
    /// Diffusion amount in [0, 1]. Higher = more scattered reflections.
    pub diffusion: f32,
    /// Zone priority — higher wins when multiple zones overlap.
    pub priority: i32,
    pub enabled: bool,
}

impl ReverbZone {
    pub fn new(radius: f32, preset: impl Into<String>) -> Self {
        Self {
            radius: radius.max(0.0),
            blend_distance: (radius * 0.1).max(0.0),
            preset: preset.into(),
            room_size: 0.5,
            decay_time: 1.5,
            diffusion: 1.0,
            priority: 0,
            enabled: true,
        }
    }

    pub fn with_room_size(mut self, size: f32) -> Self {
        self.room_size = size.clamp(0.0, 1.0);
        self
    }

    pub fn with_decay_time(mut self, seconds: f32) -> Self {
        self.decay_time = seconds.max(0.0);
        self
    }

    pub fn with_diffusion(mut self, diffusion: f32) -> Self {
        self.diffusion = diffusion.clamp(0.0, 1.0);
        self
    }

    pub fn with_blend_distance(mut self, distance: f32) -> Self {
        self.blend_distance = distance.clamp(0.0, self.radius);
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverb_zone_defaults() {
        let rz = ReverbZone::new(10.0, "room");
        assert!((rz.radius - 10.0).abs() < 0.001);
        assert_eq!(rz.preset, "room");
        assert!((rz.room_size - 0.5).abs() < 0.001);
        assert_eq!(rz.priority, 0);
        assert!(rz.enabled);
    }

    #[test]
    fn radius_clamped() {
        let rz = ReverbZone::new(-5.0, "cave");
        assert_eq!(rz.radius, 0.0);
    }

    #[test]
    fn room_size_clamped() {
        let rz = ReverbZone::new(5.0, "hall").with_room_size(2.0);
        assert!((rz.room_size - 1.0).abs() < 0.001);
    }

    #[test]
    fn blend_distance_clamped_to_radius() {
        let rz = ReverbZone::new(5.0, "hall").with_blend_distance(100.0);
        assert!(rz.blend_distance <= rz.radius);
    }

    #[test]
    fn priority_stored() {
        let rz = ReverbZone::new(8.0, "cave").with_priority(5);
        assert_eq!(rz.priority, 5);
    }
}
