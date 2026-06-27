use bevy_ecs::prelude::Component;

/// Disarm CC — prevents weapon attacks while allowing movement and ability use.
///
/// Unlike `Silence` (blocks abilities) or `Root` (blocks movement), a disarmed
/// entity is locked out only from basic weapon attacks (melee swings, ranged
/// shots). The combat system checks `is_active()` before processing an attack.
///
/// `apply(duration)` extends the disarm if the new duration is longer. `tick(dt)`
/// advances the timer. `clear()` removes the disarm instantly (cleanse effect).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Disarm {
    pub duration: f32,
    pub timer: f32,
    /// True on the first frame a disarm is applied or refreshed.
    pub just_disarmed: bool,
    /// True on the first frame the disarm expires.
    pub just_rearmed: bool,
    pub enabled: bool,
}

impl Disarm {
    pub fn new() -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            just_disarmed: false,
            just_rearmed: false,
            enabled: true,
        }
    }

    /// Apply a disarm for `duration` seconds.
    ///
    /// Resets the timer and uses the new duration only if it exceeds the
    /// remaining time. Either way, `just_disarmed` is set for this frame.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        let remaining = self.duration - self.timer;
        if duration > remaining {
            self.duration = duration.max(0.0);
            self.timer = 0.0;
        }
        self.just_disarmed = true;
    }

    /// Lift the disarm immediately (cleanse effect).
    pub fn clear(&mut self) {
        self.duration = 0.0;
        self.timer = 0.0;
    }

    /// Advance the disarm timer by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.just_disarmed = false;
        self.just_rearmed = false;

        if !self.is_active() {
            return;
        }

        self.timer += dt;
        if self.timer >= self.duration {
            self.timer = self.duration;
            self.just_rearmed = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer < self.duration
    }

    /// Remaining disarm duration in seconds.
    pub fn remaining(&self) -> f32 {
        (self.duration - self.timer).max(0.0)
    }

    /// Fraction of the disarm duration elapsed [0.0, 1.0].
    pub fn elapsed_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Disarm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_starts_disarm() {
        let mut d = Disarm::new();
        d.apply(3.0);
        assert!(d.is_active());
        assert!(d.just_disarmed);
    }

    #[test]
    fn longer_apply_resets_timer() {
        let mut d = Disarm::new();
        d.apply(2.0);
        d.tick(1.0);
        d.apply(3.0); // remaining 1.0 < 3.0 → reset
        assert_eq!(d.timer, 0.0);
        assert_eq!(d.duration, 3.0);
    }

    #[test]
    fn shorter_apply_keeps_existing() {
        let mut d = Disarm::new();
        d.apply(5.0);
        d.apply(1.0); // shorter → keep, but flag
        assert_eq!(d.duration, 5.0);
        assert!(d.just_disarmed);
    }

    #[test]
    fn tick_expires_disarm() {
        let mut d = Disarm::new();
        d.apply(2.0);
        d.tick(2.1);
        assert!(!d.is_active());
        assert!(d.just_rearmed);
    }

    #[test]
    fn clear_lifts_disarm() {
        let mut d = Disarm::new();
        d.apply(5.0);
        d.clear();
        assert!(!d.is_active());
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut d = Disarm::new();
        d.enabled = false;
        d.apply(3.0);
        assert!(!d.is_active());
        assert!(!d.just_disarmed);
    }

    #[test]
    fn remaining_decreases() {
        let mut d = Disarm::new();
        d.apply(4.0);
        d.tick(1.0);
        assert!((d.remaining() - 3.0).abs() < 1e-5);
    }

    #[test]
    fn elapsed_fraction_correct() {
        let mut d = Disarm::new();
        d.apply(4.0);
        d.tick(1.0);
        assert!((d.elapsed_fraction() - 0.25).abs() < 1e-5);
    }
}
