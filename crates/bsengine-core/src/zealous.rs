use bevy_ecs::prelude::Component;

/// Conviction-zeal tracker. `conviction` builds via `commit(amount)` and
/// deepens passively at `devote_rate` per second in `tick(dt)` or is
/// weakened immediately via `waver(amount)`.
///
/// Models religious-fervor meters, ideology-commitment bars,
/// crusader-zeal accumulators, loyalty-conviction fill levels,
/// cult-devotion trackers, mission-focus gauges, unwavering-belief
/// indicators, cause-dedication progress bars, or any mechanic where
/// passionate commitment to a goal escalates into unstoppable fervor.
///
/// `commit(amount)` adds conviction; fires `just_zealous` when first
/// reaching `max_conviction`. No-op when disabled.
///
/// `waver(amount)` reduces conviction immediately; fires `just_wavered`
/// when reaching 0. No-op when disabled or already wavered.
///
/// `tick(dt)` clears both flags, then increases conviction by
/// `devote_rate * dt` (capped at `max_conviction`). Fires `just_zealous`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_zealous()` returns `conviction >= max_conviction && enabled`.
///
/// `is_wavered()` returns `conviction == 0.0` (not gated by `enabled`).
///
/// `conviction_fraction()` returns `(conviction / max_conviction).clamp(0, 1)`.
///
/// `effective_fervor(scale)` returns `scale * conviction_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 3.0)` — deepens devotion at 3 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zealous {
    pub conviction: f32,
    pub max_conviction: f32,
    pub devote_rate: f32,
    pub just_zealous: bool,
    pub just_wavered: bool,
    pub enabled: bool,
}

impl Zealous {
    pub fn new(max_conviction: f32, devote_rate: f32) -> Self {
        Self {
            conviction: 0.0,
            max_conviction: max_conviction.max(0.1),
            devote_rate: devote_rate.max(0.0),
            just_zealous: false,
            just_wavered: false,
            enabled: true,
        }
    }

    /// Add conviction; fires `just_zealous` when first reaching max.
    /// No-op when disabled.
    pub fn commit(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.conviction < self.max_conviction;
        self.conviction = (self.conviction + amount).min(self.max_conviction);
        if was_below && self.conviction >= self.max_conviction {
            self.just_zealous = true;
        }
    }

    /// Reduce conviction; fires `just_wavered` when reaching 0.
    /// No-op when disabled or already wavered.
    pub fn waver(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.conviction <= 0.0 {
            return;
        }
        self.conviction = (self.conviction - amount).max(0.0);
        if self.conviction <= 0.0 {
            self.just_wavered = true;
        }
    }

    /// Clear flags, then increase conviction by `devote_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_zealous = false;
        self.just_wavered = false;
        if self.enabled && self.devote_rate > 0.0 && self.conviction < self.max_conviction {
            let was_below = self.conviction < self.max_conviction;
            self.conviction = (self.conviction + self.devote_rate * dt).min(self.max_conviction);
            if was_below && self.conviction >= self.max_conviction {
                self.just_zealous = true;
            }
        }
    }

    /// `true` when conviction is at maximum and component is enabled.
    pub fn is_zealous(&self) -> bool {
        self.conviction >= self.max_conviction && self.enabled
    }

    /// `true` when conviction is 0 (not gated by `enabled`).
    pub fn is_wavered(&self) -> bool {
        self.conviction == 0.0
    }

    /// Fraction of maximum conviction [0.0, 1.0].
    pub fn conviction_fraction(&self) -> f32 {
        (self.conviction / self.max_conviction).clamp(0.0, 1.0)
    }

    /// Returns `scale * conviction_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_fervor(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.conviction_fraction()
    }
}

impl Default for Zealous {
    fn default() -> Self {
        Self::new(100.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zealous {
        Zealous::new(100.0, 3.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_wavered() {
        let z = z();
        assert_eq!(z.conviction, 0.0);
        assert!(z.is_wavered());
        assert!(!z.is_zealous());
    }

    #[test]
    fn new_clamps_max_conviction() {
        let z = Zealous::new(-5.0, 3.0);
        assert!((z.max_conviction - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_devote_rate() {
        let z = Zealous::new(100.0, -3.0);
        assert_eq!(z.devote_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zealous::default();
        assert!((z.max_conviction - 100.0).abs() < 1e-5);
        assert!((z.devote_rate - 3.0).abs() < 1e-5);
    }

    // --- commit ---

    #[test]
    fn commit_adds_conviction() {
        let mut z = z();
        z.commit(40.0);
        assert!((z.conviction - 40.0).abs() < 1e-3);
    }

    #[test]
    fn commit_clamps_at_max() {
        let mut z = z();
        z.commit(200.0);
        assert!((z.conviction - 100.0).abs() < 1e-3);
    }

    #[test]
    fn commit_fires_just_zealous_at_max() {
        let mut z = z();
        z.commit(100.0);
        assert!(z.just_zealous);
        assert!(z.is_zealous());
    }

    #[test]
    fn commit_no_just_zealous_when_already_at_max() {
        let mut z = z();
        z.conviction = 100.0;
        z.commit(10.0);
        assert!(!z.just_zealous);
    }

    #[test]
    fn commit_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.commit(50.0);
        assert_eq!(z.conviction, 0.0);
    }

    #[test]
    fn commit_no_op_when_amount_zero() {
        let mut z = z();
        z.commit(0.0);
        assert_eq!(z.conviction, 0.0);
    }

    // --- waver ---

    #[test]
    fn waver_reduces_conviction() {
        let mut z = z();
        z.conviction = 60.0;
        z.waver(20.0);
        assert!((z.conviction - 40.0).abs() < 1e-3);
    }

    #[test]
    fn waver_clamps_at_zero() {
        let mut z = z();
        z.conviction = 30.0;
        z.waver(200.0);
        assert_eq!(z.conviction, 0.0);
    }

    #[test]
    fn waver_fires_just_wavered_at_zero() {
        let mut z = z();
        z.conviction = 30.0;
        z.waver(30.0);
        assert!(z.just_wavered);
    }

    #[test]
    fn waver_no_op_when_already_wavered() {
        let mut z = z();
        z.waver(10.0);
        assert!(!z.just_wavered);
    }

    #[test]
    fn waver_no_op_when_disabled() {
        let mut z = z();
        z.conviction = 50.0;
        z.enabled = false;
        z.waver(50.0);
        assert!((z.conviction - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_deepens_conviction() {
        let mut z = z(); // rate=3
        z.tick(1.0); // 0 + 3 = 3
        assert!((z.conviction - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_zealous_on_devote_to_max() {
        let mut z = Zealous::new(100.0, 200.0);
        z.conviction = 95.0;
        z.tick(1.0);
        assert!(z.just_zealous);
        assert!(z.is_zealous());
    }

    #[test]
    fn tick_no_devote_when_already_zealous() {
        let mut z = z();
        z.conviction = 100.0;
        z.tick(1.0);
        assert!(!z.just_zealous);
    }

    #[test]
    fn tick_no_devote_when_rate_zero() {
        let mut z = Zealous::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.conviction, 0.0);
    }

    #[test]
    fn tick_no_devote_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.conviction, 0.0);
    }

    #[test]
    fn tick_clears_just_zealous() {
        let mut z = Zealous::new(100.0, 200.0);
        z.conviction = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_zealous);
    }

    #[test]
    fn tick_clears_just_wavered() {
        let mut z = z();
        z.conviction = 10.0;
        z.waver(10.0);
        z.tick(0.016);
        assert!(!z.just_wavered);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=3
        z.tick(3.0); // 3*3 = 9
        assert!((z.conviction - 9.0).abs() < 1e-3);
    }

    // --- is_zealous / is_wavered ---

    #[test]
    fn is_zealous_false_when_disabled() {
        let mut z = z();
        z.conviction = 100.0;
        z.enabled = false;
        assert!(!z.is_zealous());
    }

    #[test]
    fn is_wavered_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_wavered());
    }

    // --- conviction_fraction / effective_fervor ---

    #[test]
    fn conviction_fraction_zero_when_wavered() {
        assert_eq!(z().conviction_fraction(), 0.0);
    }

    #[test]
    fn conviction_fraction_half_at_midpoint() {
        let mut z = z();
        z.conviction = 50.0;
        assert!((z.conviction_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_fervor_zero_when_wavered() {
        assert_eq!(z().effective_fervor(100.0), 0.0);
    }

    #[test]
    fn effective_fervor_scales_with_conviction() {
        let mut z = z();
        z.conviction = 70.0;
        assert!((z.effective_fervor(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_fervor_zero_when_disabled() {
        let mut z = z();
        z.conviction = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_fervor(100.0), 0.0);
    }
}
