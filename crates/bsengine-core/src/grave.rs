use bevy_ecs::prelude::Component;

/// Death-countdown mark: entity is doomed — when the timer expires, the death
/// system should kill it regardless of remaining HP. `tick(dt)` returns `true`
/// and sets `just_expired` at that moment.
///
/// `mark(duration)` starts or extends the countdown (high-watermark); sets
/// `just_marked` on the inactive → active transition. `reprieve(additional)`
/// adds time without the high-watermark restriction — stacking extensions delay
/// death further. The mark cannot be cancelled once placed; only `reprieve`
/// postpones it.
///
/// Distinct from `Curse` (generic negative status), `Doom` (stat-reducing
/// debuff), and `Revive` (post-death resurrection): Grave is a **hard death
/// timer** — when it fires, nothing can stop the entity from dying short of
/// having the Grave component disabled before expiry.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Grave {
    pub duration: f32,
    pub timer: f32,
    pub just_marked: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Grave {
    pub fn new() -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            just_marked: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Place the death mark for `duration` seconds. High-watermark: only
    /// replaces the timer when `duration > timer`. Sets `just_marked` on the
    /// inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn mark(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_marked = self.is_marked();
            self.duration = duration;
            self.timer = duration;
            if !was_marked {
                self.just_marked = true;
            }
        }
    }

    /// Add `additional` seconds to the remaining timer (additive extension).
    /// Unlike `mark`, this always adds the time rather than replacing the timer.
    /// No-op when the entity is not currently marked or `additional ≤ 0`.
    pub fn reprieve(&mut self, additional: f32) {
        if !self.is_marked() || additional <= 0.0 {
            return;
        }
        self.timer += additional;
        if self.timer > self.duration {
            self.duration = self.timer;
        }
    }

    /// Advance the death timer. Returns `true` and sets `just_expired` when
    /// the countdown reaches zero. Clears one-frame flags at the start of
    /// each tick.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.just_marked = false;
        self.just_expired = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_expired = true;
                return true;
            }
        }
        false
    }

    pub fn is_marked(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the death timer remaining [1.0 = just marked, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Grave {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_starts_countdown() {
        let mut g = Grave::new();
        g.mark(10.0);
        assert!(g.is_marked());
        assert!(g.just_marked);
    }

    #[test]
    fn mark_extends_on_longer_duration() {
        let mut g = Grave::new();
        g.mark(5.0);
        g.tick(0.016);
        g.mark(15.0);
        assert!((g.timer - 15.0).abs() < 1e-4);
    }

    #[test]
    fn mark_no_extend_on_shorter_duration() {
        let mut g = Grave::new();
        g.mark(15.0);
        g.mark(5.0);
        assert!((g.timer - 15.0).abs() < 1e-4);
    }

    #[test]
    fn just_marked_not_set_on_extend() {
        let mut g = Grave::new();
        g.mark(5.0);
        g.tick(0.016);
        g.mark(15.0);
        assert!(!g.just_marked);
    }

    #[test]
    fn reprieve_adds_time() {
        let mut g = Grave::new();
        g.mark(5.0);
        g.reprieve(3.0);
        assert!((g.timer - 8.0).abs() < 1e-5);
    }

    #[test]
    fn reprieve_no_op_when_not_marked() {
        let mut g = Grave::new();
        g.reprieve(5.0);
        assert!(!g.is_marked());
    }

    #[test]
    fn reprieve_stacks_additively() {
        let mut g = Grave::new();
        g.mark(2.0);
        g.reprieve(3.0);
        g.reprieve(4.0);
        assert!((g.timer - 9.0).abs() < 1e-5);
    }

    #[test]
    fn reprieve_no_op_for_zero_or_negative() {
        let mut g = Grave::new();
        g.mark(5.0);
        let before = g.timer;
        g.reprieve(0.0);
        g.reprieve(-1.0);
        assert!((g.timer - before).abs() < 1e-5);
    }

    #[test]
    fn tick_returns_true_and_expires_on_countdown() {
        let mut g = Grave::new();
        g.mark(1.0);
        let expired = g.tick(1.1);
        assert!(expired);
        assert!(!g.is_marked());
        assert!(g.just_expired);
    }

    #[test]
    fn tick_returns_false_before_expiry() {
        let mut g = Grave::new();
        g.mark(5.0);
        assert!(!g.tick(1.0));
    }

    #[test]
    fn tick_clears_just_marked() {
        let mut g = Grave::new();
        g.mark(5.0);
        g.tick(0.016);
        assert!(!g.just_marked);
    }

    #[test]
    fn tick_clears_just_expired() {
        let mut g = Grave::new();
        g.mark(0.5);
        g.tick(1.0);
        g.tick(0.016);
        assert!(!g.just_expired);
    }

    #[test]
    fn tick_no_op_when_not_marked() {
        let mut g = Grave::new();
        assert!(!g.tick(1.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut g = Grave::new();
        g.mark(4.0);
        g.tick(2.0);
        assert!((g.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_not_marked() {
        let g = Grave::new();
        assert!((g.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_mark_no_op() {
        let mut g = Grave::new();
        g.enabled = false;
        g.mark(10.0);
        assert!(!g.is_marked());
    }

    #[test]
    fn reprieve_after_extension_updates_duration() {
        let mut g = Grave::new();
        g.mark(5.0);
        g.reprieve(10.0);
        assert!((g.duration - 15.0).abs() < 1e-5);
    }
}
