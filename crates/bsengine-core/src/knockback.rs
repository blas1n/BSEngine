use bevy_ecs::prelude::Component;
use glam::Vec3;

/// How the knockback impulse direction is determined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KnockbackSource {
    /// Impulse is applied in a fixed world-space direction.
    Direction(Vec3),
    /// Impulse is applied away from a world-space origin point.
    FromPoint(Vec3),
}

/// Pending knockback impulse on an entity.
/// The physics system consumes this each frame: applies the impulse, decrements `hits_remaining`,
/// and removes the component when `hits_remaining` reaches zero.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Knockback {
    pub source: KnockbackSource,
    /// Force magnitude in Newtons (or m/s² depending on integrator convention).
    pub force: f32,
    /// Additional upward force applied regardless of horizontal direction.
    pub vertical_boost: f32,
    /// How many physics frames to apply the impulse. 1 = single-frame hit.
    pub hits_remaining: u32,
    /// Whether the entity is immune to further knockback while this is active.
    pub blocks_new: bool,
    pub enabled: bool,
}

impl Knockback {
    pub fn from_direction(direction: Vec3, force: f32) -> Self {
        Self {
            source: KnockbackSource::Direction(direction.normalize_or_zero()),
            force: force.max(0.0),
            vertical_boost: 0.0,
            hits_remaining: 1,
            blocks_new: false,
            enabled: true,
        }
    }

    pub fn from_point(origin: Vec3, force: f32) -> Self {
        Self {
            source: KnockbackSource::FromPoint(origin),
            force: force.max(0.0),
            vertical_boost: 0.0,
            hits_remaining: 1,
            blocks_new: false,
            enabled: true,
        }
    }

    pub fn with_vertical_boost(mut self, boost: f32) -> Self {
        self.vertical_boost = boost;
        self
    }

    pub fn with_hits(mut self, hits: u32) -> Self {
        self.hits_remaining = hits.max(1);
        self
    }

    pub fn blocking(mut self) -> Self {
        self.blocks_new = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Compute the final impulse vector given the entity's current `position`.
    /// The caller adds `external_impulse` to this result each frame.
    pub fn impulse_at(&self, position: Vec3) -> Vec3 {
        if !self.enabled {
            return Vec3::ZERO;
        }
        let horizontal = match self.source {
            KnockbackSource::Direction(d) => d,
            KnockbackSource::FromPoint(origin) => (position - origin).normalize_or_zero(),
        };
        (horizontal * self.force) + Vec3::Y * self.vertical_boost
    }

    /// Consume one hit. Returns `true` when `hits_remaining` reaches zero.
    pub fn consume(&mut self) -> bool {
        if self.hits_remaining > 0 {
            self.hits_remaining -= 1;
        }
        self.hits_remaining == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knockback_direction_impulse() {
        let kb = Knockback::from_direction(Vec3::X, 10.0);
        let impulse = kb.impulse_at(Vec3::ZERO);
        assert!((impulse.x - 10.0).abs() < 0.001);
        assert!(impulse.y.abs() < 0.001);
    }

    #[test]
    fn knockback_from_point() {
        let kb = Knockback::from_point(Vec3::ZERO, 5.0);
        let impulse = kb.impulse_at(Vec3::new(1.0, 0.0, 0.0));
        assert!(impulse.x > 0.0);
    }

    #[test]
    fn knockback_vertical_boost() {
        let kb = Knockback::from_direction(Vec3::X, 5.0).with_vertical_boost(3.0);
        let impulse = kb.impulse_at(Vec3::ZERO);
        assert!((impulse.y - 3.0).abs() < 0.001);
    }

    #[test]
    fn knockback_consume_hits() {
        let mut kb = Knockback::from_direction(Vec3::X, 1.0).with_hits(2);
        assert!(!kb.consume());
        assert!(kb.consume());
    }

    #[test]
    fn knockback_disabled_zero_impulse() {
        let kb = Knockback::from_direction(Vec3::X, 10.0).disabled();
        assert_eq!(kb.impulse_at(Vec3::ZERO), Vec3::ZERO);
    }
}
