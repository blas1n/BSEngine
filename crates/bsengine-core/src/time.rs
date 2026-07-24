use std::time::Instant;

use bevy_ecs::prelude::Resource;

/// Frame timing resource updated once per frame by the app's main loop.
#[derive(Resource)]
pub struct Time {
    /// Seconds elapsed since the previous `tick()` call.
    pub delta_seconds: f32,
    /// Total seconds elapsed since the app started.
    pub elapsed_seconds: f32,
    startup: Instant,
    last_tick: Instant,
}

impl Default for Time {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            delta_seconds: 0.0,
            elapsed_seconds: 0.0,
            startup: now,
            last_tick: now,
        }
    }
}

impl Time {
    /// Advances the clock, recomputing `delta_seconds` and `elapsed_seconds` from the current instant.
    pub fn tick(&mut self) {
        let now = Instant::now();
        self.delta_seconds = now.duration_since(self.last_tick).as_secs_f32();
        self.elapsed_seconds = now.duration_since(self.startup).as_secs_f32();
        self.last_tick = now;
    }

    /// Override delta_seconds directly — for use in tests only.
    pub fn set_delta_for_test(&mut self, delta: f32) {
        self.delta_seconds = delta;
    }
}
