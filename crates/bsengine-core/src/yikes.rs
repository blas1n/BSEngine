use bevy_ecs::prelude::Component;

/// Startle and fright accumulator. `fright` builds via `startle(amount)` and
/// fades passively at `calm_rate` per second via `tick(dt)`. Active calming is
/// available via `settle(amount)`.
///
/// Unlike threshold accumulators, `just_startled` fires on EVERY successful
/// `startle()` call — not only on reaching max. Each individual shock event is
/// thus trackable. This makes the component suitable for triggering audio,
/// animation reactions, and one-shot responses to each startle even when the
/// entity is already frightened.
///
/// `just_calmed` fires once when fright first reaches 0 (via `settle()` or
/// passive fading).
///
/// Models NPC startle reactions, jump-scare awareness levels, sudden-alarm
/// accumulators, or any stat that tracks individual shock events AND accumulated
/// overall terror.
///
/// `startle(amount)` adds to fright (capped at `max_fright`). Sets
/// `just_startled` on every successful call. No-op when disabled.
///
/// `settle(amount)` reduces fright. Fires `just_calmed` when reaching 0.
/// No-op when disabled.
///
/// `tick(dt)` clears `just_startled` and `just_calmed`. Then (when enabled
/// and `calm_rate > 0`) reduces fright by `calm_rate * dt`, floored at 0.
/// Fires `just_calmed` if fright reaches 0.
///
/// `is_terrified()` returns `fright >= max_fright && enabled`.
///
/// `is_calm()` returns `fright == 0.0` (not gated by `enabled`).
///
/// `fright_fraction()` returns `(fright / max_fright).clamp(0, 1)`.
///
/// `effective_panic(base)` returns `base * fright_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(100.0, 15.0)` — calms at 15/sec, starts at ease.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yikes {
    pub fright: f32,
    pub max_fright: f32,
    pub calm_rate: f32,
    /// Set on every successful `startle()` call (not only at max). Cleared by `tick()`.
    pub just_startled: bool,
    pub just_calmed: bool,
    pub enabled: bool,
}

impl Yikes {
    pub fn new(max_fright: f32, calm_rate: f32) -> Self {
        Self {
            fright: 0.0,
            max_fright: max_fright.max(0.1),
            calm_rate: calm_rate.max(0.0),
            just_startled: false,
            just_calmed: false,
            enabled: true,
        }
    }

    /// Add fright; sets `just_startled` on every successful call.
    /// No-op when disabled or already at max.
    pub fn startle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fright >= self.max_fright {
            return;
        }
        self.fright = (self.fright + amount).min(self.max_fright);
        self.just_startled = true;
    }

    /// Reduce fright; fires `just_calmed` when reaching 0.
    /// No-op when disabled or already calm.
    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fright <= 0.0 {
            return;
        }
        self.fright = (self.fright - amount).max(0.0);
        if self.fright <= 0.0 {
            self.just_calmed = true;
        }
    }

    /// Advance one frame: clear flags, then fade fright passively when
    /// enabled and `calm_rate > 0`. Fires `just_calmed` if fright hits 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_startled = false;
        self.just_calmed = false;
        if self.enabled && self.calm_rate > 0.0 && self.fright > 0.0 {
            self.fright = (self.fright - self.calm_rate * dt).max(0.0);
            if self.fright <= 0.0 {
                self.just_calmed = true;
            }
        }
    }

    /// `true` when fright is at maximum and component is enabled.
    pub fn is_terrified(&self) -> bool {
        self.fright >= self.max_fright && self.enabled
    }

    /// `true` when fright is 0 (not gated by `enabled`).
    pub fn is_calm(&self) -> bool {
        self.fright == 0.0
    }

    /// Fraction of maximum fright [0.0, 1.0].
    pub fn fright_fraction(&self) -> f32 {
        (self.fright / self.max_fright).clamp(0.0, 1.0)
    }

    /// Returns `base * fright_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_panic(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.fright_fraction()
    }
}

impl Default for Yikes {
    fn default() -> Self {
        Self::new(100.0, 15.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yikes {
        Yikes::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_calm() {
        let y = y();
        assert_eq!(y.fright, 0.0);
        assert!(y.is_calm());
        assert!(!y.is_terrified());
    }

    #[test]
    fn new_clamps_max_fright() {
        let y = Yikes::new(-5.0, 1.0);
        assert!((y.max_fright - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_calm_rate() {
        let y = Yikes::new(100.0, -3.0);
        assert_eq!(y.calm_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yikes::default();
        assert!((y.max_fright - 100.0).abs() < 1e-5);
        assert!((y.calm_rate - 15.0).abs() < 1e-5);
        assert_eq!(y.fright, 0.0);
    }

    // --- startle ---

    #[test]
    fn startle_increases_fright() {
        let mut y = y();
        y.startle(40.0);
        assert!((y.fright - 40.0).abs() < 1e-4);
    }

    #[test]
    fn startle_clamps_at_max() {
        let mut y = y();
        y.startle(200.0);
        assert!((y.fright - 100.0).abs() < 1e-5);
    }

    #[test]
    fn startle_fires_just_startled_on_every_call() {
        let mut y = y();
        y.startle(10.0);
        assert!(y.just_startled);
        y.tick(0.016);
        y.startle(10.0); // second startle
        assert!(y.just_startled);
    }

    #[test]
    fn startle_fires_just_startled_not_just_at_max() {
        let mut y = y();
        y.startle(30.0); // not at max, but still fires
        assert!(y.just_startled);
        assert!(!y.is_terrified());
    }

    #[test]
    fn startle_no_op_when_at_max() {
        let mut y = y();
        y.startle(100.0);
        y.tick(0.0); // clears just_startled
        y.startle(10.0); // at max → no-op
        assert!(!y.just_startled);
    }

    #[test]
    fn startle_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.startle(50.0);
        assert_eq!(y.fright, 0.0);
        assert!(!y.just_startled);
    }

    #[test]
    fn startle_no_op_for_zero_amount() {
        let mut y = y();
        y.startle(0.0);
        assert_eq!(y.fright, 0.0);
        assert!(!y.just_startled);
    }

    #[test]
    fn startle_accumulates() {
        let mut y = y();
        y.startle(30.0);
        y.startle(25.0);
        assert!((y.fright - 55.0).abs() < 1e-3);
    }

    // --- settle ---

    #[test]
    fn settle_reduces_fright() {
        let mut y = y();
        y.startle(70.0);
        y.settle(20.0);
        assert!((y.fright - 50.0).abs() < 1e-3);
    }

    #[test]
    fn settle_clamps_at_zero() {
        let mut y = y();
        y.startle(30.0);
        y.settle(200.0);
        assert_eq!(y.fright, 0.0);
    }

    #[test]
    fn settle_fires_just_calmed_at_zero() {
        let mut y = y();
        y.startle(30.0);
        y.settle(30.0);
        assert!(y.just_calmed);
        assert!(y.is_calm());
    }

    #[test]
    fn settle_no_op_when_already_calm() {
        let mut y = y();
        y.settle(10.0); // already 0
        assert!(!y.just_calmed);
    }

    #[test]
    fn settle_no_op_when_disabled() {
        let mut y = y();
        y.startle(50.0);
        y.enabled = false;
        y.settle(50.0);
        assert!((y.fright - 50.0).abs() < 1e-3);
    }

    // --- tick (passive calming) ---

    #[test]
    fn tick_fades_fright() {
        let mut y = y(); // calm_rate = 10
        y.startle(60.0);
        y.tick(1.0); // 60 - 10 = 50
        assert!((y.fright - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_at_zero() {
        let mut y = y();
        y.startle(5.0);
        y.tick(100.0);
        assert_eq!(y.fright, 0.0);
    }

    #[test]
    fn tick_fires_just_calmed_on_fade_to_zero() {
        let mut y = y();
        y.startle(5.0);
        y.tick(1.0); // fades 10 → 0
        assert!(y.just_calmed);
    }

    #[test]
    fn tick_no_fade_when_calm() {
        let mut y = y();
        y.tick(100.0); // fright=0
        assert!(!y.just_calmed);
    }

    #[test]
    fn tick_no_fade_when_rate_zero() {
        let mut y = Yikes::new(100.0, 0.0);
        y.startle(50.0);
        y.tick(100.0);
        assert!((y.fright - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_fade_when_disabled() {
        let mut y = y();
        y.startle(50.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.fright - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_startled() {
        let mut y = y();
        y.startle(30.0);
        y.tick(0.016);
        assert!(!y.just_startled);
    }

    #[test]
    fn tick_clears_just_calmed() {
        let mut y = y();
        y.startle(5.0);
        y.tick(1.0); // just_calmed fires
        y.tick(0.016); // cleared
        assert!(!y.just_calmed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = y();
        y.startle(80.0);
        y.tick(2.0); // 80 - 10*2 = 60
        assert!((y.fright - 60.0).abs() < 1e-2);
    }

    // --- is_terrified / is_calm ---

    #[test]
    fn is_terrified_at_max() {
        let mut y = y();
        y.startle(100.0);
        assert!(y.is_terrified());
    }

    #[test]
    fn is_terrified_false_below_max() {
        let mut y = y();
        y.startle(50.0);
        assert!(!y.is_terrified());
    }

    #[test]
    fn is_terrified_false_when_disabled() {
        let mut y = y();
        y.startle(100.0);
        y.enabled = false;
        assert!(!y.is_terrified());
    }

    #[test]
    fn is_calm_true_at_start() {
        assert!(y().is_calm());
    }

    #[test]
    fn is_calm_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_calm());
    }

    // --- fractions / effective ---

    #[test]
    fn fright_fraction_zero_when_calm() {
        assert_eq!(y().fright_fraction(), 0.0);
    }

    #[test]
    fn fright_fraction_half_at_midpoint() {
        let mut y = y();
        y.startle(50.0);
        assert!((y.fright_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_panic_zero_when_calm() {
        assert_eq!(y().effective_panic(100.0), 0.0);
    }

    #[test]
    fn effective_panic_scales_with_fraction() {
        let mut y = y();
        y.startle(75.0);
        assert!((y.effective_panic(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_panic_zero_when_disabled() {
        let mut y = y();
        y.startle(50.0);
        y.enabled = false;
        assert_eq!(y.effective_panic(100.0), 0.0);
    }
}
