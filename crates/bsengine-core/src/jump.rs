use bevy_ecs::prelude::Component;

/// Jump state for a character entity.
///
/// The character controller reads `wants_jump`, `impulse`, and `jumps_remaining` each
/// frame to decide whether to apply a vertical impulse. Call `tick(on_ground, dt)` once
/// per frame; call `press()` when the player presses the jump button and `release()` when
/// they release it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Jump {
    /// Initial upward impulse applied per jump (units/sec or force, depending on physics).
    pub impulse: f32,
    /// Maximum number of jumps allowed before landing (1 = single, 2 = double, etc.).
    pub max_jumps: u32,
    /// Jumps remaining before a landing is required.
    pub jumps_remaining: u32,
    /// True when the jump button was pressed this frame — consumed by the controller.
    pub wants_jump: bool,
    /// True while the jump button is held (for variable-height jumps).
    pub held: bool,
    /// Coyote time window in seconds: allows jumping briefly after walking off a ledge.
    pub coyote_time: f32,
    /// Remaining coyote time.
    pub coyote_timer: f32,
    /// Jump buffer window in seconds: a press just before landing still triggers a jump.
    pub jump_buffer: f32,
    /// Remaining buffer time since the last button press.
    pub jump_buffer_timer: f32,
    /// Whether the entity was on the ground last frame (used to detect landing).
    pub was_grounded: bool,
    pub enabled: bool,
}

impl Jump {
    pub fn new(impulse: f32) -> Self {
        Self {
            impulse: impulse.max(0.0),
            max_jumps: 1,
            jumps_remaining: 1,
            wants_jump: false,
            held: false,
            coyote_time: 0.1,
            coyote_timer: 0.0,
            jump_buffer: 0.1,
            jump_buffer_timer: 0.0,
            was_grounded: false,
            enabled: true,
        }
    }

    pub fn with_max_jumps(mut self, n: u32) -> Self {
        self.max_jumps = n.max(1);
        self.jumps_remaining = self.max_jumps;
        self
    }

    pub fn with_coyote_time(mut self, seconds: f32) -> Self {
        self.coyote_time = seconds.max(0.0);
        self
    }

    pub fn with_jump_buffer(mut self, seconds: f32) -> Self {
        self.jump_buffer = seconds.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Call when the player presses the jump button.
    pub fn press(&mut self) {
        if !self.enabled {
            return;
        }
        self.held = true;
        self.jump_buffer_timer = self.jump_buffer;
    }

    /// Call when the player releases the jump button.
    pub fn release(&mut self) {
        self.held = false;
    }

    /// Advance jump timers and set `wants_jump`. Call once per frame before the controller.
    /// `on_ground` — true if the entity is currently touching the ground.
    pub fn tick(&mut self, on_ground: bool, dt: f32) {
        if !self.enabled {
            self.wants_jump = false;
            return;
        }

        // Landing: reset jumps and coyote timer.
        if on_ground && !self.was_grounded {
            self.jumps_remaining = self.max_jumps;
            self.coyote_timer = 0.0;
        }

        // Start coyote window when entity just left the ground without jumping.
        if self.was_grounded && !on_ground && self.jumps_remaining == self.max_jumps {
            self.coyote_timer = self.coyote_time;
        }

        if self.coyote_timer > 0.0 {
            self.coyote_timer -= dt;
        }
        if self.jump_buffer_timer > 0.0 {
            self.jump_buffer_timer -= dt;
        }

        // Determine if a jump should fire.
        let can_jump = self.jumps_remaining > 0
            && (on_ground || self.coyote_timer > 0.0 || self.jumps_remaining < self.max_jumps);
        self.wants_jump = can_jump && self.jump_buffer_timer > 0.0;

        if self.wants_jump {
            self.jumps_remaining -= 1;
            self.jump_buffer_timer = 0.0;
            self.coyote_timer = 0.0;
        }

        self.was_grounded = on_ground;
    }
}

impl Default for Jump {
    fn default() -> Self {
        Self::new(8.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_fires_on_press_while_grounded() {
        let mut j = Jump::new(10.0);
        j.press();
        j.tick(true, 0.016);
        assert!(j.wants_jump);
        assert_eq!(j.jumps_remaining, 0);
    }

    #[test]
    fn double_jump_consumes_two_charges() {
        let mut j = Jump::new(10.0).with_max_jumps(2);
        j.press();
        j.tick(true, 0.016);
        assert!(j.wants_jump);
        j.wants_jump = false;
        j.press();
        j.tick(false, 0.016);
        assert!(j.wants_jump);
        assert_eq!(j.jumps_remaining, 0);
    }

    #[test]
    fn coyote_time_allows_jump_after_ledge() {
        let mut j = Jump::new(10.0).with_coyote_time(0.1);
        j.was_grounded = true;
        j.tick(false, 0.016); // left the ledge
        j.press();
        j.tick(false, 0.016); // still in coyote window
        assert!(j.wants_jump);
    }

    #[test]
    fn jump_buffer_fires_on_landing() {
        let mut j = Jump::new(10.0).with_jump_buffer(0.1);
        // Press while in air.
        j.press();
        j.tick(false, 0.016);
        assert!(!j.wants_jump); // no charge in air (single jump already used conceptually)
                                // Simulate landing with buffer still active.
        j.was_grounded = false;
        j.tick(true, 0.016); // land: restores jumps_remaining, buffer still active
        assert!(j.wants_jump);
    }

    #[test]
    fn disabled_jump_does_nothing() {
        let mut j = Jump::new(10.0).disabled();
        j.press();
        j.tick(true, 0.016);
        assert!(!j.wants_jump);
    }
}
