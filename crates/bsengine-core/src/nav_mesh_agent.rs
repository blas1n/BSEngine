use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::Vec3;

/// State of the navigation agent's movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum NavAgentState {
    /// No destination set. Agent is idle.
    #[default]
    Idle,
    /// Moving toward the current destination.
    Moving,
    /// Close enough to the destination (within `stopping_distance`).
    Arrived,
    /// Path to destination could not be found.
    NoPath,
}

/// Pathfinding agent that steers an entity along a navigation mesh.
/// The navigation system reads `destination`, computes a path, and moves the entity each frame.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct NavMeshAgent {
    /// Reflected as `Option<ReflectVec3>` -- `bevy_reflect` represents
    /// `Option` via its `Enum` trait, and the generic Inspector field editor
    /// (`draw_reflect_ui`) has no `Enum`-editing branch yet (same disclosed,
    /// pre-existing gap as `Material.texture_id: Option<u64>` from #1702),
    /// so this field is reflected but not editable through the generic UI
    /// for now.
    pub destination: Option<ReflectVec3>,
    /// Maximum movement speed in world units per second.
    pub speed: f32,
    /// Maximum rotation speed in radians per second.
    pub angular_speed: f32,
    /// Acceleration in world units per second squared.
    pub acceleration: f32,
    /// Distance from destination at which the agent is considered arrived.
    pub stopping_distance: f32,
    /// Capsule radius used for obstacle avoidance.
    pub radius: f32,
    /// Capsule height used for obstacle avoidance.
    pub height: f32,
    /// Current state written back by the navigation system each frame.
    pub state: NavAgentState,
    /// Whether the navigation system processes this agent at all.
    pub enabled: bool,
}

impl NavMeshAgent {
    /// Creates an agent with the given max speed (clamped to non-negative) and default tuning.
    pub fn new(speed: f32) -> Self {
        Self {
            destination: None,
            speed: speed.max(0.0),
            angular_speed: 120.0f32.to_radians(),
            acceleration: 8.0,
            stopping_distance: 0.1,
            radius: 0.3,
            height: 1.8,
            state: NavAgentState::Idle,
            enabled: true,
        }
    }

    /// Sets the maximum rotation speed, in radians per second (clamped to non-negative).
    pub fn with_angular_speed(mut self, radians_per_second: f32) -> Self {
        self.angular_speed = radians_per_second.max(0.0);
        self
    }

    /// Sets the arrival distance threshold, clamped to non-negative.
    pub fn with_stopping_distance(mut self, distance: f32) -> Self {
        self.stopping_distance = distance.max(0.0);
        self
    }

    /// Sets the avoidance capsule radius, clamped to non-negative.
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(0.0);
        self
    }

    /// Sets the target destination the agent should path toward.
    pub fn with_destination(mut self, destination: Vec3) -> Self {
        self.destination = Some(destination.into());
        self
    }

    /// Clears the current destination and resets state to idle.
    pub fn clear_destination(&mut self) {
        self.destination = None;
        self.state = NavAgentState::Idle;
    }

    /// Returns true if the agent is currently moving toward its destination.
    pub fn is_moving(&self) -> bool {
        self.state == NavAgentState::Moving
    }

    /// Returns true if the agent has reached its destination.
    pub fn has_arrived(&self) -> bool {
        self.state == NavAgentState::Arrived
    }
}

impl Default for NavMeshAgent {
    // speed=0.0, not the constructor's usual positive default -- an agent
    // added via the Inspector's "Add Component" dropdown with no configured
    // speed shouldn't start silently trying to move once a destination is
    // set some other way; Idle/stationary is the safer neutral default.
    fn default() -> Self {
        Self::new(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_agent_defaults() {
        let agent = NavMeshAgent::new(5.0);
        assert!((agent.speed - 5.0).abs() < 0.001);
        assert!(agent.destination.is_none());
        assert_eq!(agent.state, NavAgentState::Idle);
        assert!(agent.enabled);
    }

    #[test]
    fn speed_clamped() {
        let agent = NavMeshAgent::new(-3.0);
        assert_eq!(agent.speed, 0.0);
    }

    #[test]
    fn with_destination() {
        let agent = NavMeshAgent::new(5.0).with_destination(Vec3::new(10.0, 0.0, 5.0));
        assert!(agent.destination.is_some());
        assert_eq!(agent.destination.unwrap(), Vec3::new(10.0, 0.0, 5.0).into());
    }

    #[test]
    fn clear_destination_resets_state() {
        let mut agent = NavMeshAgent::new(5.0);
        agent.destination = Some(Vec3::ZERO.into());
        agent.state = NavAgentState::Moving;
        agent.clear_destination();
        assert!(agent.destination.is_none());
        assert_eq!(agent.state, NavAgentState::Idle);
    }

    #[test]
    fn state_helpers() {
        let mut agent = NavMeshAgent::new(5.0);
        agent.state = NavAgentState::Moving;
        assert!(agent.is_moving());
        assert!(!agent.has_arrived());
        agent.state = NavAgentState::Arrived;
        assert!(agent.has_arrived());
    }
}
