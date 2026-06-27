use bevy_ecs::prelude::Component;

/// Timed rest state: while `is_resting()`, the entity is inactive (cannot
/// move or attack) and all regeneration rates are scaled by `regen_multiplier`.
/// Callers multiply base regen values by `effective_regen(base)` each tick.
///
/// `rest(duration)` starts or extends the rest period (high-watermark: only
/// replaces the timer when `duration > timer`). Sets `just_began` on the
/// inactive → active transition. No-op when disabled or `duration ≤ 0`.
///
/// `rouse()` ends the rest early. Sets `just_ended`. No-op when not resting.
///
/// `tick(dt)` clears flags at the start, then counts down the timer. Sets
/// `just_ended` when the timer expires naturally.
///
/// `effective_regen(base)` returns `base * regen_multiplier` while resting
/// and enabled; returns `base` otherwise.
///
/// Distinct from `Regen` (unconditional passive HP recovery), `Heal` (instant
/// or pulsed HP restoration), and `Survive` (death-prevention mechanic):
/// Repose is a **voluntary rest trade-off** — the entity sacrifices combat
/// availability in exchange for amplified regeneration, making rest timing a
/// strategic choice rather than a passive bonus.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Repose {
    pub active: bool,
    pub timer: f32,
    /// Multiplier applied to all regen rates while resting. Clamped ≥ 1.0.
    pub regen_multiplier: f32,
    pub just_began: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Repose {
    pub fn new(regen_multiplier: f32) -> Self {
        Self {
            active: false,
            timer: 0.0,
            regen_multiplier: regen_multiplier.max(1.0),
            just_began: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Enter or extend the rest period for `duration` seconds. High-watermark:
    /// only replaces the current timer when `duration > timer`. Sets `just_began`
    /// on the inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn rest(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_resting = self.is_resting();
            self.timer = duration;
            self.active = true;
            if !was_resting {
                self.just_began = true;
            }
        }
    }

    /// End the rest period early. Sets `just_ended`. No-op when not resting.
    pub fn rouse(&mut self) {
        if !self.is_resting() {
            return;
        }
        self.active = false;
        self.timer = 0.0;
        self.just_ended = true;
    }

    /// Advance the rest timer. Clears one-frame flags at start. Sets `just_ended`
    /// when the timer expires naturally.
    pub fn tick(&mut self, dt: f32) {
        self.just_began = false;
        self.just_ended = false;

        if self.active {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.active = false;
                self.just_ended = true;
            }
        }
    }

    /// `true` when resting and the component is enabled.
    pub fn is_resting(&self) -> bool {
        self.active && self.enabled
    }

    /// Base regen scaled by `regen_multiplier` while resting and enabled.
    /// Returns `base` unchanged otherwise.
    pub fn effective_regen(&self, base: f32) -> f32 {
        if self.is_resting() {
            base * self.regen_multiplier
        } else {
            base
        }
    }

    /// Fraction of the current rest timer relative to a known original duration.
    /// Returns `(timer / original_duration).clamp(0, 1)`; 0.0 when not resting.
    pub fn remaining_fraction(&self, original_duration: f32) -> f32 {
        if !self.active || original_duration <= 0.0 {
            return 0.0;
        }
        (self.timer / original_duration).clamp(0.0, 1.0)
    }
}

impl Default for Repose {
    fn default() -> Self {
        Self::new(2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let r = Repose::new(2.0);
        assert!(!r.active);
        assert!(!r.is_resting());
    }

    #[test]
    fn rest_starts_rest_period() {
        let mut r = Repose::new(2.0);
        r.rest(5.0);
        assert!(r.active);
        assert!(r.just_began);
        assert!(r.is_resting());
    }

    #[test]
    fn rest_extends_on_longer_duration() {
        let mut r = Repose::new(2.0);
        r.rest(3.0);
        r.tick(0.016);
        r.rest(10.0);
        assert!((r.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn rest_no_extend_on_shorter_duration() {
        let mut r = Repose::new(2.0);
        r.rest(10.0);
        r.rest(3.0);
        assert!((r.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn just_began_not_set_on_extend() {
        let mut r = Repose::new(2.0);
        r.rest(3.0);
        r.tick(0.016);
        r.rest(10.0);
        assert!(!r.just_began);
    }

    #[test]
    fn rest_no_op_when_disabled() {
        let mut r = Repose::new(2.0);
        r.enabled = false;
        r.rest(5.0);
        assert!(!r.active);
    }

    #[test]
    fn rest_no_op_at_zero_duration() {
        let mut r = Repose::new(2.0);
        r.rest(0.0);
        assert!(!r.active);
    }

    #[test]
    fn rouse_ends_rest() {
        let mut r = Repose::new(2.0);
        r.rest(5.0);
        r.rouse();
        assert!(!r.active);
        assert!(!r.is_resting());
        assert!(r.just_ended);
    }

    #[test]
    fn rouse_no_op_when_not_resting() {
        let mut r = Repose::new(2.0);
        r.rouse();
        assert!(!r.just_ended);
    }

    #[test]
    fn tick_expires_naturally() {
        let mut r = Repose::new(2.0);
        r.rest(1.0);
        r.tick(1.1);
        assert!(!r.active);
        assert!(r.just_ended);
    }

    #[test]
    fn tick_clears_just_began() {
        let mut r = Repose::new(2.0);
        r.rest(5.0);
        r.tick(0.016);
        assert!(!r.just_began);
    }

    #[test]
    fn tick_clears_just_ended() {
        let mut r = Repose::new(2.0);
        r.rest(0.5);
        r.tick(1.0); // expires
        r.tick(0.016);
        assert!(!r.just_ended);
    }

    #[test]
    fn is_resting_false_when_disabled() {
        let mut r = Repose::new(2.0);
        r.rest(5.0);
        r.enabled = false;
        assert!(!r.is_resting());
    }

    #[test]
    fn effective_regen_multiplied_while_resting() {
        let mut r = Repose::new(3.0);
        r.rest(5.0);
        // 10 * 3 = 30
        assert!((r.effective_regen(10.0) - 30.0).abs() < 1e-3);
    }

    #[test]
    fn effective_regen_base_when_not_resting() {
        let r = Repose::new(3.0);
        assert!((r.effective_regen(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn effective_regen_base_when_disabled() {
        let mut r = Repose::new(3.0);
        r.rest(5.0);
        r.enabled = false;
        assert!((r.effective_regen(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut r = Repose::new(2.0);
        r.rest(4.0);
        r.tick(2.0);
        assert!((r.remaining_fraction(4.0) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_not_resting() {
        let r = Repose::new(2.0);
        assert_eq!(r.remaining_fraction(5.0), 0.0);
    }

    #[test]
    fn remaining_fraction_zero_at_zero_duration() {
        let mut r = Repose::new(2.0);
        r.rest(5.0);
        assert_eq!(r.remaining_fraction(0.0), 0.0);
    }

    #[test]
    fn regen_multiplier_clamped_to_one() {
        let r = Repose::new(0.5);
        assert!((r.regen_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn re_enters_rest_after_expiry() {
        let mut r = Repose::new(2.0);
        r.rest(1.0);
        r.tick(2.0); // expires
        r.tick(0.016);
        r.rest(5.0); // re-enter
        assert!(r.just_began);
    }
}
