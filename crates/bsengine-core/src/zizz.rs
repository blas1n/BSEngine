use bevy_ecs::prelude::Component;

/// Drowsiness-sleep tracker. `drowse` builds via `doze(amount)` and
/// deepens passively at `drift_rate` per second in `tick(dt)` or is
/// cleared immediately via `rouse(amount)`.
///
/// Models sleep-deprivation gauges, creature-hibernation meters,
/// narcolepsy-trigger bars, sedation-stack trackers, drowsy-driving
/// fatigue indicators, insomnia-relief fill levels, nap-readiness
/// accumulators, or any mechanic where accumulated drowsiness
/// eventually forces a sleep state that can be broken by stimulus.
///
/// `doze(amount)` adds drowse; fires `just_asleep` when first
/// reaching `max_drowse`. No-op when disabled.
///
/// `rouse(amount)` reduces drowse immediately; fires `just_roused`
/// when reaching 0. No-op when disabled or already roused.
///
/// `tick(dt)` clears both flags, then increases drowse by
/// `drift_rate * dt` (capped at `max_drowse`). Fires `just_asleep`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_asleep()` returns `drowse >= max_drowse && enabled`.
///
/// `is_roused()` returns `drowse == 0.0` (not gated by `enabled`).
///
/// `drowse_fraction()` returns `(drowse / max_drowse).clamp(0, 1)`.
///
/// `effective_torpor(scale)` returns `scale * drowse_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 3.0)` — drifts to sleep at 3 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zizz {
    pub drowse: f32,
    pub max_drowse: f32,
    pub drift_rate: f32,
    pub just_asleep: bool,
    pub just_roused: bool,
    pub enabled: bool,
}

impl Zizz {
    pub fn new(max_drowse: f32, drift_rate: f32) -> Self {
        Self {
            drowse: 0.0,
            max_drowse: max_drowse.max(0.1),
            drift_rate: drift_rate.max(0.0),
            just_asleep: false,
            just_roused: false,
            enabled: true,
        }
    }

    /// Add drowse; fires `just_asleep` when first reaching max.
    /// No-op when disabled.
    pub fn doze(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.drowse < self.max_drowse;
        self.drowse = (self.drowse + amount).min(self.max_drowse);
        if was_below && self.drowse >= self.max_drowse {
            self.just_asleep = true;
        }
    }

    /// Reduce drowse; fires `just_roused` when reaching 0.
    /// No-op when disabled or already roused.
    pub fn rouse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.drowse <= 0.0 {
            return;
        }
        self.drowse = (self.drowse - amount).max(0.0);
        if self.drowse <= 0.0 {
            self.just_roused = true;
        }
    }

    /// Clear flags, then increase drowse by `drift_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_asleep = false;
        self.just_roused = false;
        if self.enabled && self.drift_rate > 0.0 && self.drowse < self.max_drowse {
            let was_below = self.drowse < self.max_drowse;
            self.drowse = (self.drowse + self.drift_rate * dt).min(self.max_drowse);
            if was_below && self.drowse >= self.max_drowse {
                self.just_asleep = true;
            }
        }
    }

    /// `true` when drowse is at maximum and component is enabled.
    pub fn is_asleep(&self) -> bool {
        self.drowse >= self.max_drowse && self.enabled
    }

    /// `true` when drowse is 0 (not gated by `enabled`).
    pub fn is_roused(&self) -> bool {
        self.drowse == 0.0
    }

    /// Fraction of maximum drowse [0.0, 1.0].
    pub fn drowse_fraction(&self) -> f32 {
        (self.drowse / self.max_drowse).clamp(0.0, 1.0)
    }

    /// Returns `scale * drowse_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_torpor(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.drowse_fraction()
    }
}

impl Default for Zizz {
    fn default() -> Self {
        Self::new(100.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zizz {
        Zizz::new(100.0, 3.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_roused() {
        let z = z();
        assert_eq!(z.drowse, 0.0);
        assert!(z.is_roused());
        assert!(!z.is_asleep());
    }

    #[test]
    fn new_clamps_max_drowse() {
        let z = Zizz::new(-5.0, 3.0);
        assert!((z.max_drowse - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_drift_rate() {
        let z = Zizz::new(100.0, -3.0);
        assert_eq!(z.drift_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zizz::default();
        assert!((z.max_drowse - 100.0).abs() < 1e-5);
        assert!((z.drift_rate - 3.0).abs() < 1e-5);
    }

    // --- doze ---

    #[test]
    fn doze_adds_drowse() {
        let mut z = z();
        z.doze(40.0);
        assert!((z.drowse - 40.0).abs() < 1e-3);
    }

    #[test]
    fn doze_clamps_at_max() {
        let mut z = z();
        z.doze(200.0);
        assert!((z.drowse - 100.0).abs() < 1e-3);
    }

    #[test]
    fn doze_fires_just_asleep_at_max() {
        let mut z = z();
        z.doze(100.0);
        assert!(z.just_asleep);
        assert!(z.is_asleep());
    }

    #[test]
    fn doze_no_just_asleep_when_already_at_max() {
        let mut z = z();
        z.drowse = 100.0;
        z.doze(10.0);
        assert!(!z.just_asleep);
    }

    #[test]
    fn doze_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.doze(50.0);
        assert_eq!(z.drowse, 0.0);
    }

    #[test]
    fn doze_no_op_when_amount_zero() {
        let mut z = z();
        z.doze(0.0);
        assert_eq!(z.drowse, 0.0);
    }

    // --- rouse ---

    #[test]
    fn rouse_reduces_drowse() {
        let mut z = z();
        z.drowse = 60.0;
        z.rouse(20.0);
        assert!((z.drowse - 40.0).abs() < 1e-3);
    }

    #[test]
    fn rouse_clamps_at_zero() {
        let mut z = z();
        z.drowse = 30.0;
        z.rouse(200.0);
        assert_eq!(z.drowse, 0.0);
    }

    #[test]
    fn rouse_fires_just_roused_at_zero() {
        let mut z = z();
        z.drowse = 30.0;
        z.rouse(30.0);
        assert!(z.just_roused);
    }

    #[test]
    fn rouse_no_op_when_already_roused() {
        let mut z = z();
        z.rouse(10.0);
        assert!(!z.just_roused);
    }

    #[test]
    fn rouse_no_op_when_disabled() {
        let mut z = z();
        z.drowse = 50.0;
        z.enabled = false;
        z.rouse(50.0);
        assert!((z.drowse - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_drifts_drowse() {
        let mut z = z(); // rate=3
        z.tick(1.0); // 0 + 3 = 3
        assert!((z.drowse - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_asleep_on_drift_to_max() {
        let mut z = Zizz::new(100.0, 200.0);
        z.drowse = 95.0;
        z.tick(1.0);
        assert!(z.just_asleep);
        assert!(z.is_asleep());
    }

    #[test]
    fn tick_no_drift_when_already_asleep() {
        let mut z = z();
        z.drowse = 100.0;
        z.tick(1.0);
        assert!(!z.just_asleep);
    }

    #[test]
    fn tick_no_drift_when_rate_zero() {
        let mut z = Zizz::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.drowse, 0.0);
    }

    #[test]
    fn tick_no_drift_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.drowse, 0.0);
    }

    #[test]
    fn tick_clears_just_asleep() {
        let mut z = Zizz::new(100.0, 200.0);
        z.drowse = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_asleep);
    }

    #[test]
    fn tick_clears_just_roused() {
        let mut z = z();
        z.drowse = 10.0;
        z.rouse(10.0);
        z.tick(0.016);
        assert!(!z.just_roused);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=3
        z.tick(3.0); // 3*3 = 9
        assert!((z.drowse - 9.0).abs() < 1e-3);
    }

    // --- is_asleep / is_roused ---

    #[test]
    fn is_asleep_false_when_disabled() {
        let mut z = z();
        z.drowse = 100.0;
        z.enabled = false;
        assert!(!z.is_asleep());
    }

    #[test]
    fn is_roused_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_roused());
    }

    // --- drowse_fraction / effective_torpor ---

    #[test]
    fn drowse_fraction_zero_when_roused() {
        assert_eq!(z().drowse_fraction(), 0.0);
    }

    #[test]
    fn drowse_fraction_half_at_midpoint() {
        let mut z = z();
        z.drowse = 50.0;
        assert!((z.drowse_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_torpor_zero_when_roused() {
        assert_eq!(z().effective_torpor(100.0), 0.0);
    }

    #[test]
    fn effective_torpor_scales_with_drowse() {
        let mut z = z();
        z.drowse = 60.0;
        assert!((z.effective_torpor(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_torpor_zero_when_disabled() {
        let mut z = z();
        z.drowse = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_torpor(100.0), 0.0);
    }
}
