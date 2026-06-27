use bevy_ecs::prelude::Component;

/// Silence CC — prevents ability and spell use without affecting movement.
///
/// Unlike `Stun` (full CC), a silenced entity can still move, attack with
/// basic attacks, and use non-ability actions. The ability system checks
/// `is_active()` before executing any ability slot.
///
/// `apply(duration)` refreshes the silence if the new duration extends beyond
/// the remaining time. `tick(dt)` advances the timer. `clear()` lifts the
/// silence immediately (cleanse, anti-silence immunity).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Silence {
    pub duration: f32,
    pub timer: f32,
    /// True on the first frame silence is applied or refreshed.
    pub just_silenced: bool,
    /// True on the first frame silence expires.
    pub just_unsilenced: bool,
    pub enabled: bool,
}

impl Silence {
    pub fn new() -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            just_silenced: false,
            just_unsilenced: false,
            enabled: true,
        }
    }

    /// Apply a silence for `duration` seconds.
    ///
    /// If the new duration exceeds the remaining time, the timer is reset and
    /// the full new duration is used. If shorter, the existing silence is kept
    /// but `just_silenced` is still set (indicating a reapply attempt).
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        let remaining = self.duration - self.timer;
        if duration > remaining {
            self.duration = duration.max(0.0);
            self.timer = 0.0;
        }
        self.just_silenced = true;
    }

    /// Lift the silence immediately (cleanse effect).
    pub fn clear(&mut self) {
        self.duration = 0.0;
        self.timer = 0.0;
    }

    /// Advance the silence timer by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.just_silenced = false;
        self.just_unsilenced = false;

        if !self.is_active() {
            return;
        }

        self.timer += dt;
        if self.timer >= self.duration {
            self.timer = self.duration;
            self.just_unsilenced = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer < self.duration
    }

    /// Remaining silence duration in seconds.
    pub fn remaining(&self) -> f32 {
        (self.duration - self.timer).max(0.0)
    }

    /// Fraction of the silence duration elapsed [0.0, 1.0].
    pub fn elapsed_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Silence {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_starts_silence() {
        let mut s = Silence::new();
        s.apply(3.0);
        assert!(s.is_active());
        assert!(s.just_silenced);
    }

    #[test]
    fn longer_apply_resets_timer() {
        let mut s = Silence::new();
        s.apply(2.0);
        s.tick(1.0); // 1 sec in
        s.apply(3.0); // 2.0 remaining < 3.0 → reset
        assert_eq!(s.timer, 0.0);
        assert_eq!(s.duration, 3.0);
    }

    #[test]
    fn shorter_apply_keeps_existing() {
        let mut s = Silence::new();
        s.apply(5.0);
        s.apply(1.0); // shorter than 5.0 remaining → keep, but still flags
        assert_eq!(s.duration, 5.0);
        assert!(s.just_silenced);
    }

    #[test]
    fn tick_expires_silence() {
        let mut s = Silence::new();
        s.apply(2.0);
        s.tick(2.1);
        assert!(!s.is_active());
        assert!(s.just_unsilenced);
    }

    #[test]
    fn clear_lifts_silence() {
        let mut s = Silence::new();
        s.apply(5.0);
        s.clear();
        assert!(!s.is_active());
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut s = Silence::new();
        s.enabled = false;
        s.apply(3.0);
        assert!(!s.is_active());
        assert!(!s.just_silenced);
    }

    #[test]
    fn remaining_decreases() {
        let mut s = Silence::new();
        s.apply(4.0);
        s.tick(1.0);
        assert!((s.remaining() - 3.0).abs() < 1e-5);
    }

    #[test]
    fn elapsed_fraction_correct() {
        let mut s = Silence::new();
        s.apply(4.0);
        s.tick(1.0);
        assert!((s.elapsed_fraction() - 0.25).abs() < 1e-5);
    }
}
