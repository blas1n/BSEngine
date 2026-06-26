use bevy_ecs::prelude::Component;

/// A countdown timer that can fire once or repeat.
/// `TimerPlugin` ticks all active timers each frame.
/// Check `just_finished()` in your systems to react when a timer fires.
#[derive(Component, Debug, Clone)]
pub struct Timer {
    duration: f32,
    elapsed: f32,
    repeating: bool,
    just_finished: bool,
    finished: bool,
}

impl Timer {
    /// One-shot timer that fires once after `duration` seconds.
    pub fn new(duration: f32) -> Self {
        Self {
            duration: duration.max(0.0),
            elapsed: 0.0,
            repeating: false,
            just_finished: false,
            finished: false,
        }
    }

    /// Repeating timer that fires every `duration` seconds.
    pub fn repeating(duration: f32) -> Self {
        Self {
            duration: duration.max(0.0),
            elapsed: 0.0,
            repeating: true,
            just_finished: false,
            finished: false,
        }
    }

    /// Advance the timer by `delta` seconds. Returns `&Self` for chaining.
    pub fn tick(&mut self, delta: f32) -> &Self {
        self.just_finished = false;

        if self.finished && !self.repeating {
            return self;
        }

        self.elapsed += delta;

        if self.elapsed >= self.duration {
            self.just_finished = true;
            if self.repeating {
                self.elapsed -= self.duration;
            } else {
                self.elapsed = self.duration;
                self.finished = true;
            }
        }

        self
    }

    /// True only during the frame the timer crossed its duration.
    pub fn just_finished(&self) -> bool {
        self.just_finished
    }

    /// True once a one-shot timer has completed (stays true after).
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Progress in [0, 1]. Reaches 1.0 exactly when duration is hit.
    pub fn fraction(&self) -> f32 {
        if self.duration == 0.0 {
            return 1.0;
        }
        (self.elapsed / self.duration).clamp(0.0, 1.0)
    }

    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    pub fn duration(&self) -> f32 {
        self.duration
    }

    pub fn is_repeating(&self) -> bool {
        self.repeating
    }

    /// Reset elapsed back to 0 and clear finished flags.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.just_finished = false;
        self.finished = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_finished_before_duration() {
        let mut t = Timer::new(1.0);
        t.tick(0.5);
        assert!(!t.just_finished());
        assert!(!t.is_finished());
    }

    #[test]
    fn just_finished_when_duration_reached() {
        let mut t = Timer::new(1.0);
        t.tick(1.0);
        assert!(t.just_finished());
        assert!(t.is_finished());
    }

    #[test]
    fn just_finished_clears_next_tick() {
        let mut t = Timer::new(1.0);
        t.tick(1.0);
        t.tick(0.1);
        assert!(!t.just_finished());
    }

    #[test]
    fn one_shot_stops_after_finish() {
        let mut t = Timer::new(1.0);
        t.tick(2.0);
        assert!(t.is_finished());
        t.tick(1.0);
        assert!(
            !t.just_finished(),
            "should not re-fire after one-shot completes"
        );
    }

    #[test]
    fn repeating_fires_again() {
        let mut t = Timer::repeating(1.0);
        t.tick(1.0);
        assert!(t.just_finished());
        t.tick(1.0);
        assert!(t.just_finished(), "repeating should fire again");
    }

    #[test]
    fn fraction_zero_at_start() {
        assert_eq!(Timer::new(1.0).fraction(), 0.0);
    }

    #[test]
    fn fraction_one_at_end() {
        let mut t = Timer::new(1.0);
        t.tick(1.0);
        assert_eq!(t.fraction(), 1.0);
    }

    #[test]
    fn fraction_midpoint() {
        let mut t = Timer::new(1.0);
        t.tick(0.5);
        assert!((t.fraction() - 0.5).abs() < 0.001);
    }

    #[test]
    fn reset_clears_state() {
        let mut t = Timer::new(1.0);
        t.tick(1.0);
        t.reset();
        assert!(!t.is_finished());
        assert!(!t.just_finished());
        assert_eq!(t.fraction(), 0.0);
    }

    #[test]
    fn zero_duration_fires_immediately() {
        let mut t = Timer::new(0.0);
        t.tick(0.0);
        assert!(t.just_finished());
    }
}
