use bevy_ecs::prelude::Component;

/// Arousal-alertness accumulation tracker named after wake, the verb
/// and noun meaning to rouse from sleep, to become alert, to emerge
/// from a state of unconsciousness or inattention into full awareness
/// — from the Old English wacan (to awake, to arise) and wacian
/// (to be awake, to watch), both deriving from the Proto-Germanic
/// wakjanan, related to the Proto-Indo-European root weg- (to be
/// strong, to be lively). The word's root connection to watchfulness
/// explains why the same family gives English both wake (emerging
/// from sleep) and wake (a watch kept over a corpse) — both are
/// vigils, one in the morning and one over the dead. The nautical
/// wake — the trail of disturbed water left behind a vessel — derives
/// from a different Old Norse root vök (hole in ice, channel), but
/// converged in usage with the arousal meaning to become the general
/// track or aftermath left by any moving thing. The metaphorical
/// extension gave English "in the wake of" — in the track left by,
/// following as a consequence of — so that waking from sleep and
/// following in someone's track are linguistically adjacent. In
/// physiology, wake is the third state alongside NREM and REM sleep:
/// the state in which the brain is alert, sensory processing is
/// active, and voluntary motion is possible. In game mechanics, a
/// wake mechanic models the slow accumulation of alertness — the
/// build of awareness, vigilance, or readiness that eventually reaches
/// full consciousness and enables perception, reaction, or action
/// that a sleeping or inattentive state would have prevented. `arousal`
/// builds via `rouse(amount)` and accumulates passively at
/// `alert_rate` per second in `tick(dt)` or fades via `lull(amount)`.
///
/// Models arousal-alertness fill levels, vigilance-saturation bars,
/// wakefulness-accumulation trackers, attention-build gauges,
/// consciousness-approach fill levels, readiness-saturation
/// indicators, perception-opening accumulation bars, awareness-
/// threshold meters, detection-window fill levels, or any mechanic
/// where a character, creature, or system slowly accumulates
/// alertness until full wakefulness is reached — the eyes open, the
/// senses sharpen, the reaction time drops to its minimum, and what
/// was hidden from the sleeping state becomes fully visible.
///
/// `rouse(amount)` adds arousal; fires `just_woken` when first
/// reaching `max_arousal`. No-op when disabled.
///
/// `lull(amount)` reduces arousal immediately; fires `just_slept`
/// when reaching 0. No-op when disabled or already dormant.
///
/// `tick(dt)` clears both flags, then increases arousal by
/// `alert_rate * dt` (capped at `max_arousal`). Fires `just_woken`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_woken()` returns `arousal >= max_arousal && enabled`.
///
/// `is_dormant()` returns `arousal == 0.0` (not gated by `enabled`).
///
/// `arousal_fraction()` returns `(arousal / max_arousal).clamp(0, 1)`.
///
/// `effective_alertness(scale)` returns `scale * arousal_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — rouses at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wake {
    pub arousal: f32,
    pub max_arousal: f32,
    pub alert_rate: f32,
    pub just_woken: bool,
    pub just_slept: bool,
    pub enabled: bool,
}

impl Wake {
    pub fn new(max_arousal: f32, alert_rate: f32) -> Self {
        Self {
            arousal: 0.0,
            max_arousal: max_arousal.max(0.1),
            alert_rate: alert_rate.max(0.0),
            just_woken: false,
            just_slept: false,
            enabled: true,
        }
    }

    /// Add arousal; fires `just_woken` when first reaching max.
    /// No-op when disabled.
    pub fn rouse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.arousal < self.max_arousal;
        self.arousal = (self.arousal + amount).min(self.max_arousal);
        if was_below && self.arousal >= self.max_arousal {
            self.just_woken = true;
        }
    }

    /// Reduce arousal; fires `just_slept` when reaching 0.
    /// No-op when disabled or already dormant.
    pub fn lull(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.arousal <= 0.0 {
            return;
        }
        self.arousal = (self.arousal - amount).max(0.0);
        if self.arousal <= 0.0 {
            self.just_slept = true;
        }
    }

    /// Clear flags, then increase arousal by `alert_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_woken = false;
        self.just_slept = false;
        if self.enabled && self.alert_rate > 0.0 && self.arousal < self.max_arousal {
            let was_below = self.arousal < self.max_arousal;
            self.arousal = (self.arousal + self.alert_rate * dt).min(self.max_arousal);
            if was_below && self.arousal >= self.max_arousal {
                self.just_woken = true;
            }
        }
    }

    /// `true` when arousal is at maximum and component is enabled.
    pub fn is_woken(&self) -> bool {
        self.arousal >= self.max_arousal && self.enabled
    }

    /// `true` when arousal is 0 (not gated by `enabled`).
    pub fn is_dormant(&self) -> bool {
        self.arousal == 0.0
    }

    /// Fraction of maximum arousal [0.0, 1.0].
    pub fn arousal_fraction(&self) -> f32 {
        (self.arousal / self.max_arousal).clamp(0.0, 1.0)
    }

    /// Returns `scale * arousal_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_alertness(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.arousal_fraction()
    }
}

impl Default for Wake {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wake {
        Wake::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dormant() {
        let w = w();
        assert_eq!(w.arousal, 0.0);
        assert!(w.is_dormant());
        assert!(!w.is_woken());
    }

    #[test]
    fn new_clamps_max_arousal() {
        let w = Wake::new(-5.0, 1.5);
        assert!((w.max_arousal - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_alert_rate() {
        let w = Wake::new(100.0, -1.5);
        assert_eq!(w.alert_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wake::default();
        assert!((w.max_arousal - 100.0).abs() < 1e-5);
        assert!((w.alert_rate - 1.5).abs() < 1e-5);
    }

    // --- rouse ---

    #[test]
    fn rouse_adds_arousal() {
        let mut w = w();
        w.rouse(40.0);
        assert!((w.arousal - 40.0).abs() < 1e-3);
    }

    #[test]
    fn rouse_clamps_at_max() {
        let mut w = w();
        w.rouse(200.0);
        assert!((w.arousal - 100.0).abs() < 1e-3);
    }

    #[test]
    fn rouse_fires_just_woken_at_max() {
        let mut w = w();
        w.rouse(100.0);
        assert!(w.just_woken);
        assert!(w.is_woken());
    }

    #[test]
    fn rouse_no_just_woken_when_already_at_max() {
        let mut w = w();
        w.arousal = 100.0;
        w.rouse(10.0);
        assert!(!w.just_woken);
    }

    #[test]
    fn rouse_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.rouse(50.0);
        assert_eq!(w.arousal, 0.0);
    }

    #[test]
    fn rouse_no_op_when_amount_zero() {
        let mut w = w();
        w.rouse(0.0);
        assert_eq!(w.arousal, 0.0);
    }

    // --- lull ---

    #[test]
    fn lull_reduces_arousal() {
        let mut w = w();
        w.arousal = 60.0;
        w.lull(20.0);
        assert!((w.arousal - 40.0).abs() < 1e-3);
    }

    #[test]
    fn lull_clamps_at_zero() {
        let mut w = w();
        w.arousal = 30.0;
        w.lull(200.0);
        assert_eq!(w.arousal, 0.0);
    }

    #[test]
    fn lull_fires_just_slept_at_zero() {
        let mut w = w();
        w.arousal = 30.0;
        w.lull(30.0);
        assert!(w.just_slept);
    }

    #[test]
    fn lull_no_op_when_already_dormant() {
        let mut w = w();
        w.lull(10.0);
        assert!(!w.just_slept);
    }

    #[test]
    fn lull_no_op_when_disabled() {
        let mut w = w();
        w.arousal = 50.0;
        w.enabled = false;
        w.lull(50.0);
        assert!((w.arousal - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_arousal() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.arousal - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_woken_on_arousal_to_max() {
        let mut w = Wake::new(100.0, 200.0);
        w.arousal = 95.0;
        w.tick(1.0);
        assert!(w.just_woken);
        assert!(w.is_woken());
    }

    #[test]
    fn tick_no_build_when_already_woken() {
        let mut w = w();
        w.arousal = 100.0;
        w.tick(1.0);
        assert!(!w.just_woken);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wake::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.arousal, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.arousal, 0.0);
    }

    #[test]
    fn tick_clears_just_woken() {
        let mut w = Wake::new(100.0, 200.0);
        w.arousal = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_woken);
    }

    #[test]
    fn tick_clears_just_slept() {
        let mut w = w();
        w.arousal = 10.0;
        w.lull(10.0);
        w.tick(0.016);
        assert!(!w.just_slept);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.arousal - 9.0).abs() < 1e-3);
    }

    // --- is_woken / is_dormant ---

    #[test]
    fn is_woken_false_when_disabled() {
        let mut w = w();
        w.arousal = 100.0;
        w.enabled = false;
        assert!(!w.is_woken());
    }

    #[test]
    fn is_dormant_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_dormant());
    }

    // --- arousal_fraction / effective_alertness ---

    #[test]
    fn arousal_fraction_zero_when_dormant() {
        assert_eq!(w().arousal_fraction(), 0.0);
    }

    #[test]
    fn arousal_fraction_half_at_midpoint() {
        let mut w = w();
        w.arousal = 50.0;
        assert!((w.arousal_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_alertness_zero_when_dormant() {
        assert_eq!(w().effective_alertness(100.0), 0.0);
    }

    #[test]
    fn effective_alertness_scales_with_arousal() {
        let mut w = w();
        w.arousal = 75.0;
        assert!((w.effective_alertness(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_alertness_zero_when_disabled() {
        let mut w = w();
        w.arousal = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_alertness(100.0), 0.0);
    }
}
