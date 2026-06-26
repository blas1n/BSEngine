use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Category of the force source, used for selective filtering or visualisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceMode {
    /// Continuous world-space acceleration (not mass-scaled, e.g. gravity override).
    Acceleration,
    /// Continuous force scaled by the rigidbody mass (F = m·a style).
    Force,
    /// Velocity change per second (impulse-like but applied over time).
    VelocityChange,
}

/// A single named force entry in the accumulator.
#[derive(Debug, Clone, PartialEq)]
pub struct ForceEntry {
    /// Descriptive tag (e.g. "wind", "magnet", "thruster").
    pub tag: String,
    /// World-space direction and magnitude.
    pub vector: Vec3,
    pub mode: ForceMode,
    /// Remaining lifetime (seconds). `None` = permanent until explicitly removed.
    pub lifetime: Option<f32>,
}

impl ForceEntry {
    pub fn new(tag: impl Into<String>, vector: Vec3, mode: ForceMode) -> Self {
        Self {
            tag: tag.into(),
            vector,
            mode,
            lifetime: None,
        }
    }

    pub fn with_lifetime(mut self, secs: f32) -> Self {
        self.lifetime = Some(secs.max(0.0));
        self
    }
}

/// Persistent external-force accumulator for rigidbodies.
///
/// Unlike `ExternalImpulse` (single-frame), forces here persist until removed or timed out.
/// The physics system reads `acceleration()` / `force()` / `velocity_change()` each frame,
/// applies them to the rigidbody, then calls `tick(dt)` to age timed entries.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Force {
    pub entries: Vec<ForceEntry>,
    pub enabled: bool,
}

impl Force {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Add or replace a force entry by tag.
    pub fn set(&mut self, entry: ForceEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.tag == entry.tag) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// Remove the entry with the given tag. No-op if not found.
    pub fn remove(&mut self, tag: &str) {
        self.entries.retain(|e| e.tag != tag);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Advance lifetimes; drop expired entries.
    pub fn tick(&mut self, dt: f32) {
        for e in &mut self.entries {
            if let Some(t) = &mut e.lifetime {
                *t -= dt;
            }
        }
        self.entries
            .retain(|e| e.lifetime.map_or(true, |t| t > 0.0));
    }

    fn sum_by_mode(&self, mode: ForceMode) -> Vec3 {
        if !self.enabled {
            return Vec3::ZERO;
        }
        self.entries
            .iter()
            .filter(|e| e.mode == mode)
            .map(|e| e.vector)
            .fold(Vec3::ZERO, |acc, v| acc + v)
    }

    pub fn acceleration(&self) -> Vec3 {
        self.sum_by_mode(ForceMode::Acceleration)
    }

    pub fn force(&self) -> Vec3 {
        self.sum_by_mode(ForceMode::Force)
    }

    pub fn velocity_change(&self) -> Vec3 {
        self.sum_by_mode(ForceMode::VelocityChange)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Force {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_sum_acceleration() {
        let mut f = Force::new();
        f.set(ForceEntry::new(
            "wind",
            Vec3::new(1.0, 0.0, 0.0),
            ForceMode::Acceleration,
        ));
        f.set(ForceEntry::new(
            "magnet",
            Vec3::new(0.0, 2.0, 0.0),
            ForceMode::Acceleration,
        ));
        assert_eq!(f.acceleration(), Vec3::new(1.0, 2.0, 0.0));
    }

    #[test]
    fn replace_existing_tag() {
        let mut f = Force::new();
        f.set(ForceEntry::new(
            "wind",
            Vec3::new(1.0, 0.0, 0.0),
            ForceMode::Acceleration,
        ));
        f.set(ForceEntry::new(
            "wind",
            Vec3::new(5.0, 0.0, 0.0),
            ForceMode::Acceleration,
        ));
        assert_eq!(f.entries.len(), 1);
        assert_eq!(f.acceleration(), Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn remove_by_tag() {
        let mut f = Force::new();
        f.set(ForceEntry::new("wind", Vec3::X, ForceMode::Acceleration));
        f.remove("wind");
        assert!(f.is_empty());
    }

    #[test]
    fn timed_entry_expires() {
        let mut f = Force::new();
        f.set(ForceEntry::new("gust", Vec3::X, ForceMode::Acceleration).with_lifetime(0.1));
        f.tick(0.2);
        assert!(f.is_empty());
    }

    #[test]
    fn permanent_entry_survives_tick() {
        let mut f = Force::new();
        f.set(ForceEntry::new(
            "gravity_override",
            Vec3::NEG_Y * 5.0,
            ForceMode::Acceleration,
        ));
        f.tick(100.0);
        assert!(!f.is_empty());
    }

    #[test]
    fn disabled_returns_zero() {
        let mut f = Force::new().disabled();
        f.set(ForceEntry::new(
            "wind",
            Vec3::X * 10.0,
            ForceMode::Acceleration,
        ));
        assert_eq!(f.acceleration(), Vec3::ZERO);
    }
}
