use bevy_ecs::prelude::Component;

/// Corruption debuff that reduces the amount of healing the entity receives.
///
/// `effective_healing(raw)` returns `raw * (1.0 - healing_reduction)` while
/// tainted. A `healing_reduction` of 1.0 completely blocks all incoming heals
/// (full grievous-wounds effect); 0.5 halves healing.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_cleansed` on expiry. `clear()` removes the debuff early (cleanse).
///
/// Distinct from `Curse` (generic negative status with many subtypes) and
/// `Poison` (damage over time): Taint specifically targets and corrupts the
/// healing pipeline without dealing damage itself.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Taint {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] of incoming healing that is suppressed.
    /// 0.0 = no effect; 1.0 = all healing blocked.
    pub healing_reduction: f32,
    pub just_tainted: bool,
    pub just_cleansed: bool,
    pub enabled: bool,
}

impl Taint {
    pub fn new(healing_reduction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            healing_reduction: healing_reduction.clamp(0.0, 1.0),
            just_tainted: false,
            just_cleansed: false,
            enabled: true,
        }
    }

    /// Apply or extend the taint for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_tainted = true;
            }
        }
    }

    /// Cleanse the taint immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_cleansed = true;
        }
    }

    /// Advance the timer; sets `just_cleansed` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_tainted = false;
        self.just_cleansed = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_cleansed = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective healing received after applying the taint.
    /// Returns `raw * (1.0 - healing_reduction)` while active, `raw` otherwise.
    pub fn effective_healing(&self, raw: f32) -> f32 {
        if self.is_active() {
            raw * (1.0 - self.healing_reduction)
        } else {
            raw
        }
    }

    /// Fraction of the taint duration remaining [1.0 = just applied, 0.0 = cleansed].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Taint {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_taint() {
        let mut t = Taint::new(0.5);
        t.apply(3.0);
        assert!(t.is_active());
        assert!(t.just_tainted);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut t = Taint::new(0.5);
        t.apply(2.0);
        t.tick(0.016);
        t.apply(5.0);
        assert!((t.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut t = Taint::new(0.5);
        t.apply(5.0);
        t.apply(2.0);
        assert!((t.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_taint() {
        let mut t = Taint::new(0.5);
        t.apply(1.0);
        t.tick(1.1);
        assert!(!t.is_active());
        assert!(t.just_cleansed);
    }

    #[test]
    fn clear_ends_early() {
        let mut t = Taint::new(0.5);
        t.apply(5.0);
        t.clear();
        assert!(!t.is_active());
        assert!(t.just_cleansed);
    }

    #[test]
    fn effective_healing_while_active() {
        let mut t = Taint::new(0.4);
        t.apply(3.0);
        let healed = t.effective_healing(100.0);
        assert!((healed - 60.0).abs() < 1e-3); // 100 * (1 - 0.4)
    }

    #[test]
    fn effective_healing_when_inactive() {
        let t = Taint::new(0.5);
        assert!((t.effective_healing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn full_healing_block() {
        let mut t = Taint::new(1.0);
        t.apply(3.0);
        assert!((t.effective_healing(100.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut t = Taint::new(0.5);
        t.apply(2.0);
        t.tick(1.0);
        assert!((t.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut t = Taint::new(0.5);
        t.enabled = false;
        t.apply(5.0);
        assert!(!t.is_active());
    }

    #[test]
    fn tick_clears_just_tainted() {
        let mut t = Taint::new(0.5);
        t.apply(3.0);
        t.tick(0.016);
        assert!(!t.just_tainted);
    }
}
