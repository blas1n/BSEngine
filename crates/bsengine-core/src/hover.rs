use bevy_ecs::prelude::Component;

/// Mode that controls how the hover height is maintained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverMode {
    /// Apply a spring force proportional to height error (vehicles, drones).
    Spring,
    /// Teleport/clamp position directly to target height (platforms, UI objects).
    Snap,
}

/// Hover / levitation component — keeps an entity at a target height above a surface.
///
/// The movement system reads `lift_force` each frame and adds it to the rigidbody,
/// or in Snap mode writes the clamped position directly.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Hover {
    pub mode: HoverMode,
    /// Desired hover height above the ground surface (metres).
    pub target_height: f32,
    /// Measured height above ground this frame (written by the physics query).
    pub current_height: f32,
    /// Spring stiffness — how strongly height error is corrected (N/m).
    pub stiffness: f32,
    /// Damping coefficient — reduces oscillation (N·s/m).
    pub damping: f32,
    /// Vertical velocity of the entity (used for damping term, written by movement).
    pub vertical_velocity: f32,
    /// Upward force to apply this frame (computed by tick, read by movement system).
    pub lift_force: f32,
    /// Maximum upward force the hover can apply (N).
    pub max_force: f32,
    /// True when the hover system is within `snap_tolerance` of target height.
    pub at_target: bool,
    /// Distance threshold within which the entity is considered "at target".
    pub snap_tolerance: f32,
    pub enabled: bool,
}

impl Hover {
    pub fn spring(target_height: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            mode: HoverMode::Spring,
            target_height: target_height.max(0.0),
            current_height: 0.0,
            stiffness: stiffness.max(0.0),
            damping: damping.max(0.0),
            vertical_velocity: 0.0,
            lift_force: 0.0,
            max_force: f32::INFINITY,
            at_target: false,
            snap_tolerance: 0.05,
            enabled: true,
        }
    }

    pub fn snap(target_height: f32) -> Self {
        Self {
            mode: HoverMode::Snap,
            target_height: target_height.max(0.0),
            current_height: 0.0,
            stiffness: 0.0,
            damping: 0.0,
            vertical_velocity: 0.0,
            lift_force: 0.0,
            max_force: f32::INFINITY,
            at_target: false,
            snap_tolerance: 0.01,
            enabled: true,
        }
    }

    pub fn with_max_force(mut self, n: f32) -> Self {
        self.max_force = n.max(0.0);
        self
    }

    pub fn with_snap_tolerance(mut self, t: f32) -> Self {
        self.snap_tolerance = t.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Compute `lift_force` (Spring) or height correction (Snap) for this frame.
    ///
    /// Call after writing `current_height` and `vertical_velocity` from the physics query.
    pub fn tick(&mut self) {
        if !self.enabled {
            self.lift_force = 0.0;
            self.at_target = false;
            return;
        }

        let error = self.target_height - self.current_height;
        self.at_target = error.abs() <= self.snap_tolerance;

        match self.mode {
            HoverMode::Spring => {
                let force = self.stiffness * error - self.damping * self.vertical_velocity;
                self.lift_force = force.max(0.0).min(self.max_force);
            }
            HoverMode::Snap => {
                self.lift_force = error; // treated as a position delta by the movement system
            }
        }
    }

    pub fn height_error(&self) -> f32 {
        self.target_height - self.current_height
    }

    pub fn height_fraction(&self) -> f32 {
        if self.target_height <= 0.0 {
            1.0
        } else {
            (self.current_height / self.target_height).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_produces_upward_force_when_below_target() {
        let mut h = Hover::spring(2.0, 100.0, 10.0);
        h.current_height = 1.0; // 1 m below target
        h.vertical_velocity = 0.0;
        h.tick();
        assert!(h.lift_force > 0.0);
    }

    #[test]
    fn spring_force_zero_when_above_target() {
        let mut h = Hover::spring(2.0, 100.0, 10.0);
        h.current_height = 3.0; // above target
        h.vertical_velocity = 0.0;
        h.tick();
        // spring would produce negative (downward) force — clamped to 0
        assert_eq!(h.lift_force, 0.0);
    }

    #[test]
    fn spring_clamped_by_max_force() {
        let mut h = Hover::spring(5.0, 1000.0, 0.0).with_max_force(50.0);
        h.current_height = 0.0;
        h.tick();
        assert!(h.lift_force <= 50.0);
    }

    #[test]
    fn at_target_set_when_within_tolerance() {
        let mut h = Hover::spring(2.0, 100.0, 10.0).with_snap_tolerance(0.1);
        h.current_height = 2.05;
        h.tick();
        assert!(h.at_target);
    }

    #[test]
    fn snap_returns_height_delta() {
        let mut h = Hover::snap(3.0);
        h.current_height = 1.0;
        h.tick();
        assert!((h.lift_force - 2.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_clears_lift_force() {
        let mut h = Hover::spring(2.0, 100.0, 0.0).disabled();
        h.current_height = 0.0;
        h.tick();
        assert_eq!(h.lift_force, 0.0);
        assert!(!h.at_target);
    }
}
