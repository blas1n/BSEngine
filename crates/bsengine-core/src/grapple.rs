use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// State of a grapple hook in flight or attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrappleState {
    /// Hook is ready to fire; nothing attached.
    Idle,
    /// Hook projectile is in flight toward `target`.
    InFlight,
    /// Hook has latched onto an anchor point.
    Attached,
    /// Being retracted back to the owning entity.
    Retracting,
}

/// A grapple-hook component — fires a hook toward a point and pulls the entity toward it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Grapple {
    pub state: GrappleState,
    /// Entity the hook has latched to (`None` = latched to a fixed world point).
    pub anchor_entity: Option<Entity>,
    /// World-space position of the anchor point (set when `Attached`).
    pub anchor_point: Vec3,
    /// Maximum range in metres the hook can reach.
    pub max_range: f32,
    /// Speed at which the hook projectile travels (m/s).
    pub hook_speed: f32,
    /// Pull force applied toward the anchor while attached (N).
    pub pull_force: f32,
    /// Current rope length (distance from owner to anchor at latch time).
    pub rope_length: f32,
    pub enabled: bool,
}

impl Grapple {
    pub fn new(max_range: f32, hook_speed: f32, pull_force: f32) -> Self {
        Self {
            state: GrappleState::Idle,
            anchor_entity: None,
            anchor_point: Vec3::ZERO,
            max_range: max_range.max(0.0),
            hook_speed: hook_speed.max(0.0),
            pull_force: pull_force.max(0.0),
            rope_length: 0.0,
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Transition to `InFlight`. Returns `false` if not idle or not enabled.
    pub fn fire(&mut self) -> bool {
        if !self.enabled || self.state != GrappleState::Idle {
            return false;
        }
        self.state = GrappleState::InFlight;
        true
    }

    /// Called when the hook reaches a target. Records the anchor.
    pub fn attach(&mut self, point: Vec3, entity: Option<Entity>, distance: f32) {
        self.state = GrappleState::Attached;
        self.anchor_point = point;
        self.anchor_entity = entity;
        self.rope_length = distance.max(0.0);
    }

    /// Begin retraction (e.g. missed target or manual release).
    pub fn retract(&mut self) {
        self.state = GrappleState::Retracting;
        self.anchor_entity = None;
    }

    /// Reset to idle (called when the hook has fully returned).
    pub fn reset(&mut self) {
        self.state = GrappleState::Idle;
        self.anchor_entity = None;
        self.anchor_point = Vec3::ZERO;
        self.rope_length = 0.0;
    }

    pub fn is_attached(&self) -> bool {
        self.state == GrappleState::Attached
    }

    pub fn is_idle(&self) -> bool {
        self.state == GrappleState::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grapple_defaults() {
        let g = Grapple::new(20.0, 30.0, 50.0);
        assert_eq!(g.state, GrappleState::Idle);
        assert!(g.is_idle());
        assert!(g.enabled);
    }

    #[test]
    fn fire_transitions_state() {
        let mut g = Grapple::new(20.0, 30.0, 50.0);
        assert!(g.fire());
        assert_eq!(g.state, GrappleState::InFlight);
        assert!(!g.fire()); // cannot fire while in flight
    }

    #[test]
    fn attach_sets_anchor() {
        let mut g = Grapple::new(20.0, 30.0, 50.0);
        g.fire();
        g.attach(Vec3::new(1.0, 5.0, 0.0), None, 10.0);
        assert!(g.is_attached());
        assert_eq!(g.rope_length, 10.0);
    }

    #[test]
    fn retract_and_reset() {
        let mut g = Grapple::new(20.0, 30.0, 50.0);
        g.fire();
        g.retract();
        assert_eq!(g.state, GrappleState::Retracting);
        g.reset();
        assert!(g.is_idle());
    }

    #[test]
    fn disabled_grapple_cannot_fire() {
        let mut g = Grapple::new(20.0, 30.0, 50.0).disabled();
        assert!(!g.fire());
        assert!(g.is_idle());
    }
}
