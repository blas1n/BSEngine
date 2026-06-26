use std::time::Instant;

use bevy_ecs::prelude::Resource;

#[derive(Resource)]
pub struct Time {
    pub delta_seconds: f32,
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
    pub fn tick(&mut self) {
        let now = Instant::now();
        self.delta_seconds = now.duration_since(self.last_tick).as_secs_f32();
        self.elapsed_seconds = now.duration_since(self.startup).as_secs_f32();
        self.last_tick = now;
    }
}
