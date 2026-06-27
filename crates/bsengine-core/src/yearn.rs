use bevy_ecs::prelude::Component;

/// Longing accumulator. Tracks desire intensity in [0, `max_longing`]. Builds
/// passively each tick via `accumulation_rate` and can be boosted manually
/// with `ache()` or suppressed with `suppress()`. Fires `just_fulfilled` on
/// reaching max and `just_suppressed` on returning to 0.
///
/// Models craving, desire, wish mechanics, AI goal urgency, thirst for a
/// specific objective, or any state that intensifies with time and resets on
/// satisfaction.
///
/// `ache(amount)` increases longing. Fires `just_fulfilled` on first reaching
/// `max_longing`. No-op when disabled or already fulfilled.
///
/// `suppress(amount)` decreases longing. Fires `just_suppressed` on first
/// reaching 0. No-op when disabled or already suppressed.
///
/// `tick(dt)` clears `just_fulfilled` and `just_suppressed`, then (when
/// enabled and `accumulation_rate > 0`) calls `ache(accumulation_rate * dt)`
/// to grow longing passively.
///
/// `is_fulfilled()` returns `longing >= max_longing && enabled`.
///
/// `is_suppressed()` returns `longing == 0.0` (not gated by `enabled`).
///
/// `longing_fraction()` returns `(longing / max_longing).clamp(0, 1)`.
///
/// `effective_pull(base)` returns `base * longing_fraction()` when enabled;
/// `0.0` when disabled. Scales with how strongly the entity yearns.
///
/// Default: `new(100.0, 0.0)` — no passive accumulation, starts empty.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yearn {
    pub longing: f32,
    pub max_longing: f32,
    pub accumulation_rate: f32,
    pub just_fulfilled: bool,
    pub just_suppressed: bool,
    pub enabled: bool,
}

impl Yearn {
    pub fn new(max_longing: f32, accumulation_rate: f32) -> Self {
        Self {
            longing: 0.0,
            max_longing: max_longing.max(0.1),
            accumulation_rate: accumulation_rate.max(0.0),
            just_fulfilled: false,
            just_suppressed: false,
            enabled: true,
        }
    }

    /// Increase longing. Fires `just_fulfilled` on reaching `max_longing`.
    /// No-op when disabled or already fulfilled.
    pub fn ache(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.longing >= self.max_longing {
            return;
        }
        self.longing = (self.longing + amount).min(self.max_longing);
        if self.longing >= self.max_longing {
            self.just_fulfilled = true;
        }
    }

    /// Decrease longing. Fires `just_suppressed` on reaching 0. No-op when
    /// disabled or already suppressed.
    pub fn suppress(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.longing <= 0.0 {
            return;
        }
        self.longing = (self.longing - amount).max(0.0);
        if self.longing <= 0.0 {
            self.just_suppressed = true;
        }
    }

    /// Advance one frame: clear flags, then grow longing passively when
    /// enabled and `accumulation_rate > 0`.
    pub fn tick(&mut self, dt: f32) {
        self.just_fulfilled = false;
        self.just_suppressed = false;
        if self.enabled && self.accumulation_rate > 0.0 {
            self.ache(self.accumulation_rate * dt);
        }
    }

    /// `true` when longing is at maximum and component is enabled.
    pub fn is_fulfilled(&self) -> bool {
        self.longing >= self.max_longing && self.enabled
    }

    /// `true` when longing is 0 (not gated by `enabled`).
    pub fn is_suppressed(&self) -> bool {
        self.longing == 0.0
    }

    /// Fraction of maximum longing [0.0, 1.0].
    pub fn longing_fraction(&self) -> f32 {
        (self.longing / self.max_longing).clamp(0.0, 1.0)
    }

    /// Returns `base * longing_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_pull(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.longing_fraction()
    }
}

impl Default for Yearn {
    fn default() -> Self {
        Self::new(100.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yearn {
        Yearn::new(100.0, 0.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_suppressed() {
        let y = y();
        assert_eq!(y.longing, 0.0);
        assert!(y.is_suppressed());
        assert!(!y.is_fulfilled());
    }

    #[test]
    fn new_clamps_max_longing() {
        let y = Yearn::new(-5.0, 0.0);
        assert!((y.max_longing - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_accumulation_rate() {
        let y = Yearn::new(100.0, -10.0);
        assert_eq!(y.accumulation_rate, 0.0);
    }

    #[test]
    fn default_max_longing_is_hundred() {
        assert!((Yearn::default().max_longing - 100.0).abs() < 1e-5);
    }

    // --- ache ---

    #[test]
    fn ache_increases_longing() {
        let mut y = y();
        y.ache(30.0);
        assert!((y.longing - 30.0).abs() < 1e-4);
    }

    #[test]
    fn ache_clamps_at_max() {
        let mut y = y();
        y.ache(200.0);
        assert!((y.longing - 100.0).abs() < 1e-5);
    }

    #[test]
    fn ache_fires_just_fulfilled_at_max() {
        let mut y = y();
        y.ache(100.0);
        assert!(y.just_fulfilled);
        assert!(y.is_fulfilled());
    }

    #[test]
    fn ache_no_refire_when_already_fulfilled() {
        let mut y = y();
        y.ache(100.0);
        y.tick(0.016);
        y.ache(1.0); // already at max
        assert!(!y.just_fulfilled);
    }

    #[test]
    fn ache_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.ache(50.0);
        assert_eq!(y.longing, 0.0);
    }

    #[test]
    fn ache_no_op_for_zero_amount() {
        let mut y = y();
        y.ache(0.0);
        assert_eq!(y.longing, 0.0);
    }

    // --- suppress ---

    #[test]
    fn suppress_decreases_longing() {
        let mut y = y();
        y.ache(80.0);
        y.tick(0.016);
        y.suppress(30.0);
        assert!((y.longing - 50.0).abs() < 1e-4);
    }

    #[test]
    fn suppress_clamps_at_zero() {
        let mut y = y();
        y.ache(40.0);
        y.tick(0.016);
        y.suppress(100.0);
        assert_eq!(y.longing, 0.0);
    }

    #[test]
    fn suppress_fires_just_suppressed_at_zero() {
        let mut y = y();
        y.ache(40.0);
        y.tick(0.016);
        y.suppress(40.0);
        assert!(y.just_suppressed);
        assert!(y.is_suppressed());
    }

    #[test]
    fn suppress_no_refire_when_already_zero() {
        let mut y = y();
        y.suppress(10.0); // already 0
        assert!(!y.just_suppressed);
    }

    #[test]
    fn suppress_no_op_when_disabled() {
        let mut y = y();
        y.ache(50.0);
        y.enabled = false;
        y.suppress(20.0);
        assert!((y.longing - 50.0).abs() < 1e-4);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_fulfilled() {
        let mut y = y();
        y.ache(100.0);
        y.tick(0.016);
        assert!(!y.just_fulfilled);
    }

    #[test]
    fn tick_clears_just_suppressed() {
        let mut y = y();
        y.ache(30.0);
        y.suppress(30.0);
        y.tick(0.016);
        assert!(!y.just_suppressed);
    }

    #[test]
    fn tick_accumulates_passively() {
        let mut y = Yearn::new(100.0, 10.0);
        y.tick(1.0); // 10 per second * 1s = 10
        assert!((y.longing - 10.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_accumulation_when_rate_zero() {
        let mut y = y(); // accumulation_rate = 0
        y.tick(10.0);
        assert_eq!(y.longing, 0.0);
    }

    #[test]
    fn tick_fires_just_fulfilled_via_passive_accumulation() {
        let mut y = Yearn::new(10.0, 100.0);
        y.tick(1.0); // 100 * 1.0 >> 10
        assert!(y.just_fulfilled);
    }

    #[test]
    fn tick_no_accumulation_when_disabled() {
        let mut y = Yearn::new(100.0, 10.0);
        y.enabled = false;
        y.tick(1.0);
        assert_eq!(y.longing, 0.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = Yearn::new(100.0, 20.0);
        y.tick(0.5); // 20 * 0.5 = 10
        assert!((y.longing - 10.0).abs() < 1e-3);
    }

    // --- is_fulfilled / is_suppressed ---

    #[test]
    fn is_fulfilled_false_below_max() {
        let mut y = y();
        y.ache(50.0);
        assert!(!y.is_fulfilled());
    }

    #[test]
    fn is_fulfilled_false_when_disabled() {
        let mut y = y();
        y.ache(100.0);
        y.enabled = false;
        assert!(!y.is_fulfilled());
    }

    #[test]
    fn is_suppressed_true_at_zero() {
        assert!(y().is_suppressed());
    }

    #[test]
    fn is_suppressed_true_even_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_suppressed());
    }

    #[test]
    fn is_suppressed_false_with_longing() {
        let mut y = y();
        y.ache(10.0);
        assert!(!y.is_suppressed());
    }

    // --- fractions / effective ---

    #[test]
    fn longing_fraction_zero_when_empty() {
        assert_eq!(y().longing_fraction(), 0.0);
    }

    #[test]
    fn longing_fraction_half_at_midpoint() {
        let mut y = y();
        y.ache(50.0);
        assert!((y.longing_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn longing_fraction_one_at_max() {
        let mut y = y();
        y.ache(100.0);
        assert!((y.longing_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_pull_zero_when_empty() {
        assert_eq!(y().effective_pull(100.0), 0.0);
    }

    #[test]
    fn effective_pull_scales_with_fraction() {
        let mut y = y();
        y.ache(75.0);
        assert!((y.effective_pull(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_pull_zero_when_disabled() {
        let mut y = y();
        y.ache(50.0);
        y.enabled = false;
        assert_eq!(y.effective_pull(100.0), 0.0);
    }
}
