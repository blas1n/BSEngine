use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Collision impact response component.
///
/// The physics system writes `record()` when a collision whose impulse exceeds
/// `min_force` is detected. Game systems can read `just_impacted` (true for
/// exactly one frame after a new impact) to trigger sound, camera shake, damage,
/// or VFX. `just_impacted` is cleared automatically each frame by calling `tick()`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Impact {
    /// Outward normal of the surface struck (world-space, normalised).
    pub normal: Vec3,
    /// Velocity of the entity at the moment of impact (world-space).
    pub velocity: Vec3,
    /// Magnitude of the collision impulse (N·s).
    pub force: f32,
    /// True for one frame immediately after a new impact above `min_force`.
    pub just_impacted: bool,
    /// Cumulative hit count since this component was added.
    pub impact_count: u32,
    /// Minimum force (N·s) required for an impact to be recorded.
    pub min_force: f32,
    pub enabled: bool,
}

impl Impact {
    pub fn new(min_force: f32) -> Self {
        Self {
            normal: Vec3::Y,
            velocity: Vec3::ZERO,
            force: 0.0,
            just_impacted: false,
            impact_count: 0,
            min_force: min_force.max(0.0),
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Record a new impact. Called by the physics system each collision event.
    /// Returns true if the impact exceeded the threshold and was recorded.
    pub fn record(&mut self, normal: Vec3, velocity: Vec3, force: f32) -> bool {
        if !self.enabled || force < self.min_force {
            return false;
        }
        self.normal = normal.normalize_or_zero();
        self.velocity = velocity;
        self.force = force;
        self.just_impacted = true;
        self.impact_count += 1;
        true
    }

    /// Clear the one-frame `just_impacted` flag. Call once per frame.
    pub fn tick(&mut self) {
        self.just_impacted = false;
    }

    /// True if the last recorded impact was hard enough to cause damage.
    /// `damage_threshold` is separate from `min_force` so a surface can *react*
    /// at a lower force than it takes *damage*.
    pub fn is_hard_impact(&self, damage_threshold: f32) -> bool {
        self.force >= damage_threshold
    }

    /// Fraction of `force` relative to `damage_threshold` (clamped 0–1).
    pub fn impact_fraction(&self, damage_threshold: f32) -> f32 {
        if damage_threshold <= 0.0 {
            return 1.0;
        }
        (self.force / damage_threshold).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_sets_just_impacted() {
        let mut i = Impact::new(5.0);
        let ok = i.record(Vec3::Y, Vec3::new(0.0, -10.0, 0.0), 20.0);
        assert!(ok);
        assert!(i.just_impacted);
        assert_eq!(i.impact_count, 1);
    }

    #[test]
    fn record_below_threshold_ignored() {
        let mut i = Impact::new(10.0);
        let ok = i.record(Vec3::Y, Vec3::ZERO, 5.0);
        assert!(!ok);
        assert!(!i.just_impacted);
        assert_eq!(i.impact_count, 0);
    }

    #[test]
    fn tick_clears_just_impacted() {
        let mut i = Impact::new(0.0);
        i.record(Vec3::Y, Vec3::ZERO, 1.0);
        assert!(i.just_impacted);
        i.tick();
        assert!(!i.just_impacted);
    }

    #[test]
    fn is_hard_impact_threshold() {
        let mut i = Impact::new(0.0);
        i.record(Vec3::Y, Vec3::ZERO, 15.0);
        assert!(i.is_hard_impact(10.0));
        assert!(!i.is_hard_impact(20.0));
    }

    #[test]
    fn impact_fraction_clamps() {
        let mut i = Impact::new(0.0);
        i.record(Vec3::Y, Vec3::ZERO, 5.0);
        assert_eq!(i.impact_fraction(10.0), 0.5);
        assert_eq!(i.impact_fraction(3.0), 1.0); // clamped
    }

    #[test]
    fn impact_count_increments_per_hit() {
        let mut i = Impact::new(0.0);
        i.record(Vec3::Y, Vec3::ZERO, 1.0);
        i.record(Vec3::Y, Vec3::ZERO, 1.0);
        assert_eq!(i.impact_count, 2);
    }
}
