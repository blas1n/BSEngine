use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// Cone-shaped field-of-view awareness component, typically attached to NPCs or turrets.
///
/// The perception system queries `Vision` to decide whether a candidate entity is "seen".
/// It checks:
///   1. Distance ≤ `range`
///   2. Angle between forward and direction-to-target ≤ `fov_half_angle`
///   3. Optional line-of-sight (managed externally; the system sets `los_blocked`)
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vision {
    /// Full cone range (metres).
    pub range: f32,
    /// Half-angle of the field-of-view cone (radians). 0 = pencil-beam, π = full sphere.
    pub fov_half_angle: f32,
    /// Layer mask — only entities whose layers overlap this mask are considered.
    pub detection_mask: u32,
    /// Last entity the vision cone detected. Updated by the perception system.
    pub last_seen: Option<Entity>,
    /// World-space position where `last_seen` was last observed.
    pub last_seen_position: Option<Vec3>,
    /// Set by the physics/raycasting system: true when LOS to the current target is blocked.
    pub los_blocked: bool,
    /// Accumulated time (seconds) the current target has been continuously in view.
    pub in_view_duration: f32,
    /// Minimum seconds a target must be in view before `last_seen` is committed.
    pub detection_delay: f32,
    pub enabled: bool,
}

impl Vision {
    /// `fov_degrees` is the full cone angle in degrees (e.g. 120 for 60° each side).
    pub fn new(range: f32, fov_degrees: f32) -> Self {
        Self {
            range: range.max(0.0),
            fov_half_angle: (fov_degrees * 0.5).to_radians(),
            detection_mask: u32::MAX,
            last_seen: None,
            last_seen_position: None,
            los_blocked: false,
            in_view_duration: 0.0,
            detection_delay: 0.0,
            enabled: true,
        }
    }

    pub fn with_mask(mut self, mask: u32) -> Self {
        self.detection_mask = mask;
        self
    }

    pub fn with_detection_delay(mut self, secs: f32) -> Self {
        self.detection_delay = secs.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns true if `candidate` is within the vision cone.
    /// `forward` must be normalised. `candidate_layer` is the layer bit of the candidate.
    pub fn can_see(
        &self,
        observer_pos: Vec3,
        forward: Vec3,
        candidate_pos: Vec3,
        candidate_layer: u32,
    ) -> bool {
        if !self.enabled || (self.detection_mask & candidate_layer) == 0 {
            return false;
        }
        let to_target = candidate_pos - observer_pos;
        let dist_sq = to_target.length_squared();
        if dist_sq > self.range * self.range {
            return false;
        }
        if dist_sq < 1e-10 {
            return true;
        }
        let dir = to_target / dist_sq.sqrt();
        let cos_angle = forward.dot(dir);
        cos_angle >= self.fov_half_angle.cos()
    }

    /// Called by the perception system each frame when a target is in the cone.
    /// Returns true when the detection delay has been satisfied.
    pub fn tick_in_view(&mut self, dt: f32, target: Entity, pos: Vec3) -> bool {
        self.in_view_duration += dt;
        if self.in_view_duration >= self.detection_delay {
            self.last_seen = Some(target);
            self.last_seen_position = Some(pos);
            true
        } else {
            false
        }
    }

    /// Called when the target leaves the cone or LOS is broken.
    pub fn reset_view_timer(&mut self) {
        self.in_view_duration = 0.0;
    }

    pub fn has_target(&self) -> bool {
        self.last_seen.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::World;

    fn dummy_entity() -> Entity {
        World::new().spawn_empty().id()
    }

    #[test]
    fn target_within_cone() {
        let v = Vision::new(10.0, 90.0);
        let obs = Vec3::ZERO;
        let fwd = Vec3::Z;
        let target = Vec3::new(0.0, 0.0, 5.0);
        assert!(v.can_see(obs, fwd, target, u32::MAX));
    }

    #[test]
    fn target_behind_observer() {
        let v = Vision::new(10.0, 90.0);
        let obs = Vec3::ZERO;
        let fwd = Vec3::Z;
        let target = Vec3::new(0.0, 0.0, -5.0);
        assert!(!v.can_see(obs, fwd, target, u32::MAX));
    }

    #[test]
    fn target_beyond_range() {
        let v = Vision::new(4.0, 90.0);
        let obs = Vec3::ZERO;
        let fwd = Vec3::Z;
        let target = Vec3::new(0.0, 0.0, 5.0);
        assert!(!v.can_see(obs, fwd, target, u32::MAX));
    }

    #[test]
    fn mask_filters_wrong_layer() {
        let v = Vision::new(10.0, 90.0).with_mask(0b0001);
        let obs = Vec3::ZERO;
        let fwd = Vec3::Z;
        let target = Vec3::new(0.0, 0.0, 5.0);
        assert!(!v.can_see(obs, fwd, target, 0b0010));
        assert!(v.can_see(obs, fwd, target, 0b0001));
    }

    #[test]
    fn detection_delay_prevents_early_commit() {
        let mut v = Vision::new(10.0, 90.0).with_detection_delay(0.5);
        let e = dummy_entity();
        let pos = Vec3::new(0.0, 0.0, 5.0);
        let detected = v.tick_in_view(0.3, e, pos);
        assert!(!detected);
        assert!(!v.has_target());
        let detected = v.tick_in_view(0.3, e, pos);
        assert!(detected);
        assert!(v.has_target());
    }
}
