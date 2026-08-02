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
    /// When set, `tick()` advances by this fixed step instead of reading the
    /// clock. See [`Time::fixed`].
    fixed_step: Option<f32>,
}

impl Default for Time {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            delta_seconds: 0.0,
            elapsed_seconds: 0.0,
            startup: now,
            last_tick: now,
            fixed_step: None,
        }
    }
}

impl Time {
    /// A clock that advances exactly `dt` seconds per `tick()`, ignoring how
    /// long the frame really took.
    ///
    /// For headless replay. Rapier already steps a fixed 1/60s per frame no
    /// matter the frame's real duration, so a wall-clock `Time` puts anything
    /// driven by it (nav-mesh agents, animation, tweens, timers) on a
    /// different clock from physics — and headless frames run in well under a
    /// millisecond, so the two diverge by more than an order of magnitude, by
    /// a ratio that changes with machine speed. Pass Rapier's timestep here so
    /// every system advances together and a replay is reproducible.
    pub fn fixed(dt: f32) -> Self {
        Self {
            fixed_step: Some(dt),
            ..Self::default()
        }
    }

    /// Advances the clock, recomputing `delta_seconds` and `elapsed_seconds` from the current instant.
    pub fn tick(&mut self) {
        if let Some(dt) = self.fixed_step {
            self.delta_seconds = dt;
            self.elapsed_seconds += dt;
            return;
        }
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
