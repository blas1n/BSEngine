use bevy_ecs::prelude::Component;

/// Root (immobilization) CC — entity cannot move but retains other actions.
///
/// Unlike `Stun` (full CC) or `Slow` (partial speed reduction), Root removes
/// movement entirely while leaving attacking, casting, and turning intact.
/// Common in action RPGs, MOBAs, and tower-defence designs.
///
/// The locomotion system suppresses movement while `is_active()` is true.
/// `apply(duration)` refreshes the root if the new duration exceeds the
/// remaining time. `tick(dt)` expires the root. `clear()` removes it
/// immediately (e.g. cleanse, break-free ability).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Root {
    pub duration: f32,
    pub timer: f32,
    /// When true, the entity can still rotate/turn while rooted.
    pub allows_rotation: bool,
    /// When true, the entity can still attack while rooted.
    pub allows_attack: bool,
    /// True on the first frame the root is applied or refreshed.
    pub just_rooted: bool,
    /// True on the first frame the root expires.
    pub just_freed: bool,
    pub enabled: bool,
}

impl Root {
    pub fn new(duration: f32) -> Self {
        Self {
            duration: duration.max(0.0),
            timer: 0.0,
            allows_rotation: true,
            allows_attack: true,
            just_rooted: false,
            just_freed: false,
            enabled: true,
        }
    }

    pub fn with_allows_rotation(mut self, allow: bool) -> Self {
        self.allows_rotation = allow;
        self
    }

    pub fn with_allows_attack(mut self, allow: bool) -> Self {
        self.allows_attack = allow;
        self
    }

    /// Apply a root. Does nothing if disabled.
    ///
    /// If the new duration is longer than the remaining root time, the timer
    /// is reset and the new duration is used; otherwise, the existing root
    /// is kept but `just_rooted` is still set.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        let remaining = self.duration - self.timer;
        if duration > remaining {
            self.duration = duration.max(0.0);
            self.timer = 0.0;
        }
        self.just_rooted = true;
    }

    /// Remove the root immediately.
    pub fn clear(&mut self) {
        self.duration = 0.0;
        self.timer = 0.0;
    }

    /// Advance the timer by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.just_rooted = false;
        self.just_freed = false;

        if !self.is_active() {
            return;
        }

        self.timer += dt;
        if self.timer >= self.duration {
            self.timer = self.duration;
            self.just_freed = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer < self.duration
    }

    /// Remaining root duration in seconds.
    pub fn remaining(&self) -> f32 {
        (self.duration - self.timer).max(0.0)
    }

    /// Fraction of the root duration elapsed [0.0, 1.0].
    pub fn elapsed_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_starts_root() {
        let mut r = Root::new(0.0);
        r.apply(3.0);
        assert!(r.is_active());
        assert!(r.just_rooted);
    }

    #[test]
    fn longer_apply_resets_timer() {
        let mut r = Root::new(2.0);
        r.tick(1.0);
        r.apply(3.0); // 2.0 remaining < 3.0 → reset
        assert_eq!(r.timer, 0.0);
        assert_eq!(r.duration, 3.0);
    }

    #[test]
    fn shorter_apply_keeps_existing() {
        let mut r = Root::new(5.0);
        r.tick(0.0); // activate
        r.apply(1.0); // 5.0 remaining > 1.0 → keep
        assert_eq!(r.duration, 5.0);
        assert!(r.just_rooted); // still flags just_rooted
    }

    #[test]
    fn tick_expires_root() {
        let mut r = Root::new(2.0);
        r.tick(2.1);
        assert!(!r.is_active());
        assert!(r.just_freed);
    }

    #[test]
    fn clear_deactivates() {
        let mut r = Root::new(5.0);
        r.clear();
        assert!(!r.is_active());
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut r = Root::new(0.0);
        r.enabled = false;
        r.apply(3.0);
        assert!(!r.is_active());
        assert!(!r.just_rooted);
    }

    #[test]
    fn remaining_decreases() {
        let mut r = Root::new(4.0);
        r.tick(1.0);
        assert!((r.remaining() - 3.0).abs() < 1e-5);
    }

    #[test]
    fn elapsed_fraction_correct() {
        let mut r = Root::new(4.0);
        r.tick(1.0);
        assert!((r.elapsed_fraction() - 0.25).abs() < 1e-5);
    }

    #[test]
    fn allows_rotation_default_true() {
        let r = Root::new(3.0);
        assert!(r.allows_rotation);
        assert!(r.allows_attack);
    }
}
