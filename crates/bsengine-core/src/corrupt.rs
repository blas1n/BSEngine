use bevy_ecs::prelude::Component;

/// Corruption debuff that causes the entity's own effects to misfire or invert.
///
/// While corrupted, the ability system calls `check_corrupted(rng_value)` before
/// applying any effect originating from this entity. When it returns `true` the
/// caller should invert or negate the effect (e.g. heal → damage, buff → debuff,
/// boost → penalty). `chance` [0.0, 1.0] controls how often misfires occur.
///
/// Examples: chaos-magic debuffs, corrupted runes, cursed items.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_uncorrupted` when the corruption lifts.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Corrupt {
    pub duration: f32,
    pub timer: f32,
    /// Probability [0.0, 1.0] that any single effect misfires while corrupted.
    pub chance: f32,
    pub just_corrupted: bool,
    pub just_uncorrupted: bool,
    pub enabled: bool,
}

impl Corrupt {
    pub fn new(chance: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            chance: chance.clamp(0.0, 1.0),
            just_corrupted: false,
            just_uncorrupted: false,
            enabled: true,
        }
    }

    /// Apply or extend corruption for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_corrupted = true;
            }
        }
    }

    /// Remove the corruption immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_uncorrupted = true;
        }
    }

    /// Advance the timer; sets `just_uncorrupted` when the effect ends.
    pub fn tick(&mut self, dt: f32) {
        self.just_corrupted = false;
        self.just_uncorrupted = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_uncorrupted = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `true` when an outgoing effect should be inverted/negated.
    /// `rng_value` must be a pre-rolled float in [0.0, 1.0).
    pub fn check_corrupted(&self, rng_value: f32) -> bool {
        self.enabled && self.is_active() && rng_value < self.chance
    }

    /// Fraction of the corruption duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Corrupt {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_corruption() {
        let mut c = Corrupt::new(0.5);
        c.apply(3.0);
        assert!(c.is_active());
        assert!(c.just_corrupted);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut c = Corrupt::new(0.5);
        c.apply(2.0);
        c.tick(0.016);
        c.apply(5.0);
        assert!((c.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut c = Corrupt::new(0.5);
        c.apply(5.0);
        c.apply(2.0);
        assert!((c.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_corruption() {
        let mut c = Corrupt::new(0.5);
        c.apply(1.0);
        c.tick(1.1);
        assert!(!c.is_active());
        assert!(c.just_uncorrupted);
    }

    #[test]
    fn clear_ends_early() {
        let mut c = Corrupt::new(0.5);
        c.apply(5.0);
        c.clear();
        assert!(!c.is_active());
        assert!(c.just_uncorrupted);
    }

    #[test]
    fn check_corrupted_below_chance() {
        let mut c = Corrupt::new(0.5);
        c.apply(3.0);
        assert!(c.check_corrupted(0.3)); // 0.3 < 0.5 → misfire
    }

    #[test]
    fn check_corrupted_above_chance() {
        let mut c = Corrupt::new(0.5);
        c.apply(3.0);
        assert!(!c.check_corrupted(0.7)); // 0.7 >= 0.5 → normal
    }

    #[test]
    fn check_corrupted_when_inactive() {
        let c = Corrupt::new(1.0); // timer = 0
        assert!(!c.check_corrupted(0.0));
    }

    #[test]
    fn check_corrupted_zero_chance() {
        let mut c = Corrupt::new(0.0);
        c.apply(5.0);
        assert!(!c.check_corrupted(0.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut c = Corrupt::new(0.5);
        c.apply(2.0);
        c.tick(1.0);
        assert!((c.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut c = Corrupt::new(0.5);
        c.enabled = false;
        c.apply(5.0);
        assert!(!c.is_active());
    }

    #[test]
    fn disabled_check_corrupted_false() {
        let mut c = Corrupt::new(1.0);
        c.apply(5.0);
        c.enabled = false;
        assert!(!c.check_corrupted(0.0));
    }
}
