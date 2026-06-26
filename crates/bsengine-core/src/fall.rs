use bevy_ecs::prelude::Component;

/// Tracks the character's vertical air phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallState {
    /// On the ground (or a stable surface).
    Grounded,
    /// Ascending (jumped or launched upward) — not yet falling.
    Rising,
    /// Descending under gravity.
    Falling,
    /// First frame of landing — impact speed recorded.
    Landing,
}

/// Free-fall tracking component — detects hard landings, fall damage, and coyote time.
///
/// The movement system updates `fall_speed` and `is_grounded` each frame, then calls
/// `tick(dt, is_grounded, vertical_speed)`. Check `state` and `fall_distance` to
/// trigger fall-damage, landing animation, or sound effects.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Fall {
    pub state: FallState,
    /// Downward speed this frame (positive = falling, written by movement system).
    pub fall_speed: f32,
    /// Metres fallen since leaving the ground.
    pub fall_distance: f32,
    /// Speed at the moment of landing (used to calculate fall damage).
    pub landing_speed: f32,
    /// Y-position when the character last left the ground.
    pub launch_height: f32,
    /// Fall distance beyond which fall damage is applied.
    pub lethal_distance: f32,
    /// Fall distance at which soft landing (stumble) triggers.
    pub soft_landing_distance: f32,
    /// Seconds the character can still jump after walking off a ledge.
    pub coyote_time: f32,
    /// Remaining coyote-time window (seconds).
    pub coyote_timer: f32,
    pub enabled: bool,
}

impl Fall {
    pub fn new(soft_landing_distance: f32, lethal_distance: f32) -> Self {
        Self {
            state: FallState::Grounded,
            fall_speed: 0.0,
            fall_distance: 0.0,
            landing_speed: 0.0,
            launch_height: 0.0,
            lethal_distance: lethal_distance.max(0.0),
            soft_landing_distance: soft_landing_distance.max(0.0),
            coyote_time: 0.12,
            coyote_timer: 0.0,
            enabled: true,
        }
    }

    pub fn with_coyote_time(mut self, secs: f32) -> Self {
        self.coyote_time = secs.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Update state each frame.
    ///
    /// * `is_grounded` — true when the physics system reports ground contact.
    /// * `vertical_speed` — positive = moving up, negative = falling.
    /// * `current_y` — world-space Y for fall-distance tracking.
    pub fn tick(&mut self, dt: f32, is_grounded: bool, vertical_speed: f32, current_y: f32) {
        if !self.enabled {
            return;
        }

        // Clear one-frame Landing state.
        if self.state == FallState::Landing {
            self.state = FallState::Grounded;
        }

        if is_grounded {
            if matches!(self.state, FallState::Falling | FallState::Rising) {
                // Just landed.
                self.landing_speed = self.fall_speed;
                self.fall_distance = (self.launch_height - current_y).max(0.0);
                self.state = FallState::Landing;
            }
            self.fall_speed = 0.0;
            self.coyote_timer = self.coyote_time;
            self.launch_height = current_y;
        } else {
            if self.state == FallState::Grounded || self.state == FallState::Landing {
                // Just left the ground.
                self.state = if vertical_speed >= 0.0 {
                    FallState::Rising
                } else {
                    FallState::Falling
                };
                self.launch_height = current_y;
            }

            if self.coyote_timer > 0.0 {
                self.coyote_timer = (self.coyote_timer - dt).max(0.0);
            }

            self.fall_speed = (-vertical_speed).max(0.0);

            if vertical_speed < 0.0 {
                self.state = FallState::Falling;
            }
        }
    }

    /// True if the character is currently falling (not just left the ground).
    pub fn is_falling(&self) -> bool {
        self.state == FallState::Falling
    }

    /// True within the coyote-time window (character can still jump).
    pub fn can_coyote_jump(&self) -> bool {
        self.coyote_timer > 0.0
    }

    /// True on the first frame of landing.
    pub fn just_landed(&self) -> bool {
        self.state == FallState::Landing
    }

    /// True if the landing speed exceeds the soft-landing threshold.
    pub fn is_hard_landing(&self) -> bool {
        self.just_landed() && self.landing_speed >= self.soft_landing_distance
    }

    /// True if the fall distance exceeds the lethal threshold.
    pub fn is_lethal_fall(&self) -> bool {
        self.just_landed() && self.fall_distance >= self.lethal_distance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounded_tick_maintains_grounded_state() {
        let mut f = Fall::new(5.0, 15.0);
        f.tick(0.016, true, 0.0, 0.0);
        assert_eq!(f.state, FallState::Grounded);
    }

    #[test]
    fn leaving_ground_transitions_to_falling() {
        let mut f = Fall::new(5.0, 15.0);
        f.tick(0.016, false, -5.0, 0.0); // vertical_speed < 0 → falling
        assert!(f.is_falling());
    }

    #[test]
    fn landing_detected_on_first_grounded_frame() {
        let mut f = Fall::new(5.0, 15.0);
        f.tick(0.016, false, -10.0, 0.0);
        f.tick(0.016, true, 0.0, -0.5); // landed
        assert!(f.just_landed());
        assert!(f.landing_speed > 0.0);
    }

    #[test]
    fn coyote_timer_counts_down_after_leaving_ground() {
        let mut f = Fall::new(5.0, 15.0).with_coyote_time(0.12);
        f.coyote_timer = f.coyote_time; // simulate was grounded
        f.tick(0.016, false, -1.0, 0.0);
        assert!(f.can_coyote_jump());
        // Drain completely.
        f.tick(0.5, false, -1.0, 0.0);
        assert!(!f.can_coyote_jump());
    }

    #[test]
    fn lethal_fall_flagged_when_distance_exceeds_threshold() {
        let mut f = Fall::new(5.0, 10.0);
        // Simulate falling from height=20 → landing at y=0.
        f.tick(0.016, false, -10.0, 20.0);
        f.tick(0.016, true, 0.0, 0.0);
        assert!(f.is_lethal_fall());
    }

    #[test]
    fn short_fall_not_lethal() {
        let mut f = Fall::new(5.0, 10.0);
        f.tick(0.016, false, -5.0, 3.0);
        f.tick(0.016, true, 0.0, 0.0);
        assert!(!f.is_lethal_fall());
    }
}
