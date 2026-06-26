use bevy_ecs::prelude::Component;

/// Per-entity cooldown timer for abilities, attacks, or any time-gated action.
/// A system must call `tick(dt)` each frame. `start()` begins a new cooldown.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Cooldown {
    /// Total cooldown duration in seconds.
    pub duration: f32,
    /// Remaining time in seconds. 0 means the cooldown is ready.
    pub remaining: f32,
}

impl Cooldown {
    pub fn new(duration: f32) -> Self {
        Self {
            duration: duration.max(0.0),
            remaining: 0.0,
        }
    }

    /// Returns `true` when no cooldown is active.
    pub fn is_ready(&self) -> bool {
        self.remaining <= 0.0
    }

    /// Starts (or restarts) the cooldown.
    pub fn start(&mut self) {
        self.remaining = self.duration;
    }

    /// Advances the cooldown by `dt` seconds. Returns `true` if it just became ready.
    pub fn tick(&mut self, dt: f32) -> bool {
        let was_active = self.remaining > 0.0;
        self.remaining = (self.remaining - dt).max(0.0);
        was_active && self.remaining <= 0.0
    }

    /// Fraction of the cooldown that has elapsed [0.0, 1.0].
    /// Returns 1.0 when ready (no cooldown active).
    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        1.0 - (self.remaining / self.duration).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_starts_ready() {
        let c = Cooldown::new(1.0);
        assert!(c.is_ready());
        assert!((c.progress() - 1.0).abs() < 0.001);
    }

    #[test]
    fn start_sets_remaining() {
        let mut c = Cooldown::new(2.0);
        c.start();
        assert!(!c.is_ready());
        assert_eq!(c.remaining, 2.0);
    }

    #[test]
    fn tick_reduces_remaining() {
        let mut c = Cooldown::new(1.0);
        c.start();
        let expired = c.tick(0.5);
        assert!(!expired);
        assert!(!c.is_ready());
        assert!((c.remaining - 0.5).abs() < 0.001);
    }

    #[test]
    fn tick_returns_true_when_expires() {
        let mut c = Cooldown::new(1.0);
        c.start();
        let expired = c.tick(2.0);
        assert!(expired);
        assert!(c.is_ready());
    }

    #[test]
    fn progress_increases_as_cooldown_drains() {
        let mut c = Cooldown::new(2.0);
        c.start();
        c.tick(1.0);
        assert!((c.progress() - 0.5).abs() < 0.001);
    }
}
