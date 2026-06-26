use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Behaviour when the patrol reaches the end of its waypoint list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatrolMode {
    /// Reverse direction and walk back through the waypoints.
    PingPong,
    /// Jump back to the first waypoint and repeat.
    Loop,
    /// Stop at the last waypoint.
    Once,
}

/// AI waypoint-patrol component.
///
/// The AI/navigation system reads `current_target()` each frame and moves the
/// entity toward it. Call `tick(position, dt)` once the entity has reached
/// the waypoint to advance to the next one.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Patrol {
    pub waypoints: Vec<Vec3>,
    pub mode: PatrolMode,
    /// Index into `waypoints` of the waypoint the entity is heading toward.
    pub index: usize,
    /// Direction of traversal: +1 = forward, -1 = backward (PingPong only).
    pub direction: i32,
    /// How close the entity must be to a waypoint to count as reached (units).
    pub arrival_radius: f32,
    /// Time in seconds the entity waits at each waypoint. 0.0 = no wait.
    pub wait_duration: f32,
    /// Remaining wait time at the current waypoint.
    pub wait_timer: f32,
    /// Whether the patrol is finished (Only mode reached last waypoint).
    pub finished: bool,
    pub enabled: bool,
}

impl Patrol {
    pub fn new(waypoints: Vec<Vec3>) -> Self {
        Self {
            waypoints,
            mode: PatrolMode::Loop,
            index: 0,
            direction: 1,
            arrival_radius: 0.5,
            wait_duration: 0.0,
            wait_timer: 0.0,
            finished: false,
            enabled: true,
        }
    }

    pub fn with_mode(mut self, mode: PatrolMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_arrival_radius(mut self, radius: f32) -> Self {
        self.arrival_radius = radius.max(0.0);
        self
    }

    pub fn with_wait(mut self, seconds: f32) -> Self {
        self.wait_duration = seconds.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Current waypoint the entity should move toward, or `None` if finished/disabled/empty.
    pub fn current_target(&self) -> Option<Vec3> {
        if !self.enabled || self.finished || self.waypoints.is_empty() {
            return None;
        }
        self.waypoints.get(self.index).copied()
    }

    /// Call every frame. Decrements wait timer or checks arrival.
    /// Returns `true` when the patrol advances to a new waypoint.
    pub fn tick(&mut self, position: Vec3, _dt: f32) -> bool {
        if !self.enabled || self.finished || self.waypoints.is_empty() {
            return false;
        }

        // Waiting at waypoint.
        if self.wait_timer > 0.0 {
            self.wait_timer -= _dt;
            return false;
        }

        let Some(target) = self.waypoints.get(self.index).copied() else {
            return false;
        };

        if position.distance(target) <= self.arrival_radius {
            self.advance();
            return true;
        }
        false
    }

    fn advance(&mut self) {
        if self.waypoints.is_empty() {
            return;
        }

        // Start wait if configured.
        if self.wait_duration > 0.0 {
            self.wait_timer = self.wait_duration;
        }

        let len = self.waypoints.len();
        match self.mode {
            PatrolMode::Loop => {
                self.index = (self.index + 1) % len;
            }
            PatrolMode::Once => {
                if self.index + 1 < len {
                    self.index += 1;
                } else {
                    self.finished = true;
                }
            }
            PatrolMode::PingPong => {
                let next = self.index as i32 + self.direction;
                if next < 0 || next >= len as i32 {
                    self.direction = -self.direction;
                    self.index =
                        (self.index as i32 + self.direction).clamp(0, len as i32 - 1) as usize;
                } else {
                    self.index = next as usize;
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.index = 0;
        self.direction = 1;
        self.finished = false;
        self.wait_timer = 0.0;
    }
}

impl Default for Patrol {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patrol_at(positions: &[(f32, f32, f32)]) -> Patrol {
        Patrol::new(
            positions
                .iter()
                .map(|&(x, y, z)| Vec3::new(x, y, z))
                .collect(),
        )
    }

    #[test]
    fn patrol_loops_through_waypoints() {
        let mut p = patrol_at(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (2.0, 0.0, 0.0)])
            .with_mode(PatrolMode::Loop);
        assert_eq!(p.current_target(), Some(Vec3::new(0.0, 0.0, 0.0)));
        p.tick(Vec3::new(0.0, 0.0, 0.0), 0.016);
        assert_eq!(p.current_target(), Some(Vec3::new(1.0, 0.0, 0.0)));
        p.tick(Vec3::new(1.0, 0.0, 0.0), 0.016);
        p.tick(Vec3::new(2.0, 0.0, 0.0), 0.016);
        assert_eq!(p.current_target(), Some(Vec3::new(0.0, 0.0, 0.0)));
    }

    #[test]
    fn patrol_once_finishes() {
        let mut p = patrol_at(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)]).with_mode(PatrolMode::Once);
        p.tick(Vec3::ZERO, 0.016);
        p.tick(Vec3::new(1.0, 0.0, 0.0), 0.016);
        assert!(p.finished);
        assert_eq!(p.current_target(), None);
    }

    #[test]
    fn patrol_ping_pong_reverses() {
        let mut p = patrol_at(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (2.0, 0.0, 0.0)])
            .with_mode(PatrolMode::PingPong);
        p.tick(Vec3::ZERO, 0.016);
        p.tick(Vec3::new(1.0, 0.0, 0.0), 0.016);
        p.tick(Vec3::new(2.0, 0.0, 0.0), 0.016);
        assert_eq!(p.current_target(), Some(Vec3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn patrol_disabled_no_target() {
        let p = patrol_at(&[(0.0, 0.0, 0.0)]).disabled();
        assert_eq!(p.current_target(), None);
    }

    #[test]
    fn patrol_wait_delays_advance() {
        let mut p = patrol_at(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)]).with_wait(1.0);
        // Arrive at waypoint 0 — triggers wait, does not advance yet.
        p.tick(Vec3::ZERO, 0.016);
        assert_eq!(p.index, 1);
        // Still on wait timer.
        let advanced = p.tick(Vec3::ZERO, 0.5);
        assert!(!advanced);
    }
}
