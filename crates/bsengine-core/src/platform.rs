use bevy_ecs::prelude::Component;
use glam::Vec3;

/// How the platform behaves when it reaches the end of its waypoint list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformMode {
    /// Stops at the last waypoint forever.
    OneShot,
    /// Teleports back to the first waypoint and repeats.
    Loop,
    /// Reverses direction at each end.
    PingPong,
}

/// Moving platform component.
///
/// The movement system reads `target_position()` each frame and moves the
/// entity toward it. Passengers (entities standing on the platform) should
/// have their positions updated by the same system to ride correctly.
///
/// Waypoints are stored as world-space positions. The system calls `tick(dt)`
/// to advance the platform along segments at `speed` m/s, handling pauses and
/// mode transitions automatically.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Platform {
    pub mode: PlatformMode,
    /// World-space waypoints in traversal order.
    pub waypoints: Vec<Vec3>,
    /// Movement speed in metres per second.
    pub speed: f32,
    /// Current segment index (0 = waypoints[0]→waypoints[1]).
    pub segment: usize,
    /// Interpolation parameter [0.0, 1.0] along the current segment.
    pub t: f32,
    /// Whether the platform is currently moving forward through waypoints.
    pub forward: bool,
    /// How long to pause (seconds) at each waypoint before continuing.
    pub pause_time: f32,
    /// Remaining pause time at the current waypoint.
    pub pause_timer: f32,
    /// Whether the platform is currently waiting at a waypoint.
    pub paused: bool,
    /// Accumulated distance moved this tick (used by passenger system).
    pub delta_position: Vec3,
    pub enabled: bool,
}

impl Platform {
    /// Create a platform with the given waypoints, speed, and mode.
    pub fn new(waypoints: Vec<Vec3>, speed: f32, mode: PlatformMode) -> Self {
        Self {
            mode,
            waypoints,
            speed: speed.max(0.0),
            segment: 0,
            t: 0.0,
            forward: true,
            pause_time: 0.0,
            pause_timer: 0.0,
            paused: false,
            delta_position: Vec3::ZERO,
            enabled: true,
        }
    }

    pub fn with_pause(mut self, pause_time: f32) -> Self {
        self.pause_time = pause_time.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance the platform. Returns the world-space displacement this frame.
    pub fn tick(&mut self, dt: f32) -> Vec3 {
        self.delta_position = Vec3::ZERO;

        if !self.enabled || self.waypoints.len() < 2 {
            return Vec3::ZERO;
        }

        if self.paused {
            self.pause_timer -= dt;
            if self.pause_timer <= 0.0 {
                self.paused = false;
            }
            return Vec3::ZERO;
        }

        let (from_idx, to_idx) = self.segment_indices();
        let from = self.waypoints[from_idx];
        let to = self.waypoints[to_idx];
        let segment_len = (to - from).length();

        if segment_len < 1e-6 {
            self.advance_segment();
            return Vec3::ZERO;
        }

        let prev_pos = from.lerp(to, self.t);
        let dt_distance = self.speed * dt;
        self.t += dt_distance / segment_len;

        if self.t >= 1.0 {
            self.t = 1.0;
            let new_pos = to;
            self.delta_position = new_pos - prev_pos;
            self.advance_segment();
        } else {
            let new_pos = from.lerp(to, self.t);
            self.delta_position = new_pos - prev_pos;
        }

        self.delta_position
    }

    /// Current interpolated world-space position of the platform.
    pub fn target_position(&self) -> Vec3 {
        if self.waypoints.len() < 2 {
            return self.waypoints.first().copied().unwrap_or(Vec3::ZERO);
        }
        let (from_idx, to_idx) = self.segment_indices();
        self.waypoints[from_idx].lerp(self.waypoints[to_idx], self.t)
    }

    fn segment_indices(&self) -> (usize, usize) {
        let count = self.waypoints.len();
        if self.forward {
            (self.segment, (self.segment + 1).min(count - 1))
        } else {
            let from = count - 1 - self.segment;
            let to = from.saturating_sub(1);
            (from, to)
        }
    }

    fn advance_segment(&mut self) {
        let last = self.waypoints.len().saturating_sub(2);
        self.t = 0.0;

        match self.mode {
            PlatformMode::OneShot => {
                if self.segment < last {
                    self.segment += 1;
                }
            }
            PlatformMode::Loop => {
                self.segment = (self.segment + 1) % (last + 1);
            }
            PlatformMode::PingPong => {
                if self.forward {
                    if self.segment >= last {
                        self.forward = false;
                        self.segment = 0;
                    } else {
                        self.segment += 1;
                    }
                } else if self.segment >= last {
                    self.forward = true;
                    self.segment = 0;
                } else {
                    self.segment += 1;
                }
            }
        }

        if self.pause_time > 0.0 {
            self.paused = true;
            self.pause_timer = self.pause_time;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_point(mode: PlatformMode) -> Platform {
        Platform::new(vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)], 10.0, mode)
    }

    #[test]
    fn moves_toward_second_waypoint() {
        let mut p = two_point(PlatformMode::Loop);
        let d = p.tick(0.5);
        assert!(d.x > 0.0, "should move in X");
    }

    #[test]
    fn reaches_waypoint_and_loops() {
        let mut p = two_point(PlatformMode::Loop);
        p.tick(1.5); // covers 15 m — past the 10 m segment
        assert_eq!(p.segment, 0); // looped back
    }

    #[test]
    fn ping_pong_reverses_direction() {
        let mut p = two_point(PlatformMode::PingPong);
        p.tick(1.5); // arrives at end
        assert!(!p.forward);
    }

    #[test]
    fn one_shot_stops_at_end() {
        let mut p = two_point(PlatformMode::OneShot);
        p.tick(2.0);
        assert_eq!(p.segment, 0); // no more segments to advance to
    }

    #[test]
    fn pause_at_waypoint() {
        let mut p = two_point(PlatformMode::Loop).with_pause(1.0);
        p.tick(1.5); // reaches waypoint, starts pausing
        let d = p.tick(0.5); // should still be paused
        assert_eq!(d, Vec3::ZERO);
    }

    #[test]
    fn target_position_interpolates() {
        let p = two_point(PlatformMode::Loop);
        // t=0, segment=0 → should be at ZERO
        assert_eq!(p.target_position(), Vec3::ZERO);
    }
}
