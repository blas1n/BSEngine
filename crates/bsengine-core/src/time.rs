use crate::clock::Instant;

use bevy_ecs::prelude::{Resource, SystemSet};

/// The set the app's clock ticks in, once per frame in `PreUpdate`.
///
/// A `PreUpdate` system that reads `delta_seconds` must run `.after` this.
/// Unordered, the scheduler is free to run it before the tick: it then reads
/// the previous frame's step -- and on a fixed clock's first frame, 0 -- so an
/// input hold measured in frames comes out one frame long or short depending
/// on which plugin was added first.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimeSystems;

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

    /// Leaves out the time since the last tick, for an app back from the
    /// background: the next tick measures from now, and `elapsed_seconds`
    /// does not count the time away. Without it the first frame back is one
    /// frame as long as the whole absence -- a timer due in a second fires
    /// at once, a tween jumps to its end. A fixed clock is untouched: it
    /// never read the time.
    pub fn resume_after_pause(&mut self) {
        let now = Instant::now();
        let away = now.duration_since(self.last_tick);
        self.startup += away;
        self.last_tick = now;
    }

    /// Override delta_seconds directly — for use in tests only.
    pub fn set_delta_for_test(&mut self, delta: f32) {
        self.delta_seconds = delta;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// A clock whose last tick was `away` ago and which started `ran` before
    /// that -- an app that ran for `ran` and then sat in the background for
    /// `away`, with no sleep in the test.
    fn clock_back_from(ran: Duration, away: Duration) -> Time {
        let now = Instant::now();
        Time {
            delta_seconds: 0.016,
            elapsed_seconds: ran.as_secs_f32(),
            startup: now - away - ran,
            last_tick: now - away,
            fixed_step: None,
        }
    }

    /// The hard side: a minute away, and the first frame back is a frame,
    /// not a minute -- with the time before the pause still counted.
    #[test]
    fn the_time_away_is_neither_a_frame_nor_elapsed_time() {
        let (ran, away) = (Duration::from_secs(10), Duration::from_secs(60));

        // The premise, on the same clock without the call: the first tick
        // back is the whole absence.
        let mut control = clock_back_from(ran, away);
        control.tick();
        assert!(
            control.delta_seconds >= 59.0,
            "the fixture's first frame back is {}s, not the minute away -- it measures nothing",
            control.delta_seconds
        );

        let mut time = clock_back_from(ran, away);
        time.resume_after_pause();
        time.tick();
        assert!(
            time.delta_seconds < 1.0,
            "the first frame back was {}s long: the time away was counted as a frame",
            time.delta_seconds
        );
        assert!(
            (9.0..11.0).contains(&time.elapsed_seconds),
            "elapsed is {}s after 10s of running and 60s away: the pause was {}",
            time.elapsed_seconds,
            if time.elapsed_seconds > 11.0 {
                "counted"
            } else {
                "taken out of the time before it too"
            }
        );
    }
}
