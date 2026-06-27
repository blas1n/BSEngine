use bevy_ecs::prelude::Component;

/// Rate-based resource drain applied to an entity over time.
///
/// Each frame the system calls `tick(dt)`, which returns the amount of resource
/// to subtract this frame (`rate * dt`). The caller decides which resource to
/// deplete (mana, stamina, health, etc.). When the drain is timed, `tick`
/// also counts down the timer and sets `just_expired` on the final frame.
///
/// `apply(rate, duration)` uses high-watermark on duration. To model a
/// permanent drain, set a very long duration (e.g. `f32::MAX`).
///
/// Examples: mana siphon debuffs, stamina bleed, heat drain.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Drain {
    /// Resource units drained per second.
    pub rate: f32,
    pub duration: f32,
    pub timer: f32,
    pub just_drained: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Drain {
    pub fn new(rate: f32, duration: f32) -> Self {
        Self {
            rate: rate.max(0.0),
            duration,
            timer: duration,
            just_drained: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Apply or extend the drain. High-watermark: only replaces the current
    /// timer if the new duration is longer. Updates `rate` to the new value
    /// when the duration is extended.
    pub fn apply(&mut self, rate: f32, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.rate = rate.max(0.0);
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_drained = true;
            }
        }
    }

    /// Remove the drain immediately.
    pub fn clear(&mut self) {
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_expired = false;
    }

    /// Advance the timer and return the amount of resource to drain this frame.
    /// Returns `0.0` when the drain is inactive or disabled.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_drained = false;
        self.just_expired = false;

        if !self.enabled || !self.is_active() {
            return 0.0;
        }

        let amount = self.rate * dt;

        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_expired = true;
        }

        amount
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the drain duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Drain {
    fn default() -> Self {
        Self::new(5.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_active() {
        let d = Drain::new(10.0, 5.0);
        assert!(d.is_active());
    }

    #[test]
    fn tick_returns_rate_times_dt() {
        let mut d = Drain::new(10.0, 5.0);
        let amount = d.tick(0.1);
        assert!((amount - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_expires_drain() {
        let mut d = Drain::new(10.0, 1.0);
        d.tick(1.1);
        assert!(!d.is_active());
        assert!(d.just_expired);
    }

    #[test]
    fn tick_zero_when_inactive() {
        let mut d = Drain::new(10.0, 1.0);
        d.tick(2.0); // expire
        let amount = d.tick(0.1);
        assert!((amount - 0.0).abs() < 1e-5);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut d = Drain::new(5.0, 3.0);
        d.tick(0.1);
        d.apply(10.0, 8.0);
        assert!((d.timer - 8.0).abs() < 1e-4);
        assert!((d.rate - 10.0).abs() < 1e-5);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut d = Drain::new(5.0, 5.0);
        d.apply(20.0, 2.0);
        assert!((d.timer - 5.0).abs() < 1e-4);
        assert!((d.rate - 5.0).abs() < 1e-5);
    }

    #[test]
    fn apply_on_inactive_activates() {
        let mut d = Drain::new(5.0, 1.0);
        d.tick(2.0); // expire
        d.apply(10.0, 3.0);
        assert!(d.is_active());
        assert!(d.just_drained);
    }

    #[test]
    fn clear_deactivates() {
        let mut d = Drain::new(5.0, 5.0);
        d.clear();
        assert!(!d.is_active());
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut d = Drain::new(5.0, 2.0);
        d.tick(1.0);
        assert!((d.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_tick_returns_zero() {
        let mut d = Drain::new(10.0, 5.0);
        d.enabled = false;
        let amount = d.tick(1.0);
        assert!((amount - 0.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut d = Drain::new(0.0, 0.0);
        d.enabled = false;
        d.apply(10.0, 5.0);
        assert!(!d.is_active());
    }
}
