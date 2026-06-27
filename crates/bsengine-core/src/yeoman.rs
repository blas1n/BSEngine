use bevy_ecs::prelude::Component;

/// Reliability meter for a steadfast servant or companion. Tracks how
/// dependably an entity performs service. `serve(amount)` each frame
/// raises `reliability` and marks the frame as served. A tick with no
/// preceding `serve()` drains `reliability` by `falter_rate * dt`.
///
/// Models companion loyalty ratings, NPC hire-ability scores, service-streak
/// mechanics, or any system where consistent effort builds trust and
/// inaction erodes it.
///
/// `serve(amount)` adds to reliability (clamped to `max_reliability`). Fires
/// `just_trusted` on first reaching max. Marks `served` for this frame.
/// No-op when disabled.
///
/// `tick(dt)` clears `just_trusted` and `just_failed`. When no `serve()` was
/// called this frame and `falter_rate > 0`, drains `reliability` by
/// `falter_rate * dt` (minimum 0). Fires `just_failed` when reliability first
/// reaches 0. Finally resets `served`.
///
/// `is_trusted()` returns `reliability >= max_reliability && enabled`.
///
/// `is_failed()` returns `reliability == 0.0` (not gated by `enabled`).
///
/// `reliability_fraction()` returns `(reliability / max_reliability).clamp(0, 1)`.
///
/// `effective_quality(base)` returns `base * reliability_fraction()` when
/// enabled; `0.0` when disabled. Models service quality scaling with
/// dependability.
///
/// Default: `new(100.0, 5.0)` — starts at 0 reliability, drains 5/sec
/// when not serving.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yeoman {
    pub reliability: f32,
    pub max_reliability: f32,
    pub falter_rate: f32,
    /// Set by `serve()` this frame; cleared by `tick()`.
    pub served: bool,
    pub just_trusted: bool,
    pub just_failed: bool,
    pub enabled: bool,
}

impl Yeoman {
    pub fn new(max_reliability: f32, falter_rate: f32) -> Self {
        Self {
            reliability: 0.0,
            max_reliability: max_reliability.max(0.1),
            falter_rate: falter_rate.max(0.0),
            served: false,
            just_trusted: false,
            just_failed: false,
            enabled: true,
        }
    }

    /// Record service this frame; raise reliability. Fires `just_trusted` on
    /// first reaching max. No-op when disabled.
    pub fn serve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.served = true;
        if self.reliability >= self.max_reliability {
            return;
        }
        self.reliability = (self.reliability + amount).min(self.max_reliability);
        if self.reliability >= self.max_reliability {
            self.just_trusted = true;
        }
    }

    /// Advance one frame. Clears flags, drains reliability when no service
    /// was rendered this frame, then resets `served`.
    pub fn tick(&mut self, dt: f32) {
        self.just_trusted = false;
        self.just_failed = false;
        if !self.served && self.enabled && self.falter_rate > 0.0 && self.reliability > 0.0 {
            self.reliability = (self.reliability - self.falter_rate * dt).max(0.0);
            if self.reliability <= 0.0 {
                self.just_failed = true;
            }
        }
        self.served = false;
    }

    /// `true` when reliability is at maximum and component is enabled.
    pub fn is_trusted(&self) -> bool {
        self.reliability >= self.max_reliability && self.enabled
    }

    /// `true` when reliability is 0 (not gated by `enabled`).
    pub fn is_failed(&self) -> bool {
        self.reliability == 0.0
    }

    /// Fraction of maximum reliability [0.0, 1.0].
    pub fn reliability_fraction(&self) -> f32 {
        (self.reliability / self.max_reliability).clamp(0.0, 1.0)
    }

    /// Returns `base * reliability_fraction()` when enabled; `0.0` when
    /// disabled.
    pub fn effective_quality(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.reliability_fraction()
    }
}

impl Default for Yeoman {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yeoman {
        Yeoman::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_at_zero_reliability() {
        let y = y();
        assert_eq!(y.reliability, 0.0);
        assert!(y.is_failed());
        assert!(!y.is_trusted());
    }

    #[test]
    fn new_clamps_max_reliability() {
        let y = Yeoman::new(-5.0, 1.0);
        assert!((y.max_reliability - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_falter_rate() {
        let y = Yeoman::new(100.0, -3.0);
        assert_eq!(y.falter_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yeoman::default();
        assert!((y.max_reliability - 100.0).abs() < 1e-5);
        assert!((y.falter_rate - 5.0).abs() < 1e-5);
    }

    // --- serve ---

    #[test]
    fn serve_increases_reliability() {
        let mut y = y();
        y.serve(30.0);
        assert!((y.reliability - 30.0).abs() < 1e-4);
    }

    #[test]
    fn serve_sets_served_flag() {
        let mut y = y();
        y.serve(10.0);
        assert!(y.served);
    }

    #[test]
    fn serve_clamps_at_max() {
        let mut y = y();
        y.serve(200.0);
        assert!((y.reliability - 100.0).abs() < 1e-5);
    }

    #[test]
    fn serve_fires_just_trusted_at_max() {
        let mut y = y();
        y.serve(100.0);
        assert!(y.just_trusted);
        assert!(y.is_trusted());
    }

    #[test]
    fn serve_no_refire_when_already_trusted() {
        let mut y = y();
        y.serve(100.0);
        y.serve(100.0); // already at max
        y.tick(0.016);
        y.serve(100.0); // still at max
        assert!(!y.just_trusted);
    }

    #[test]
    fn serve_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.serve(50.0);
        assert_eq!(y.reliability, 0.0);
        assert!(!y.served);
    }

    #[test]
    fn serve_no_op_for_zero_amount() {
        let mut y = y();
        y.serve(0.0);
        assert_eq!(y.reliability, 0.0);
        assert!(!y.served);
    }

    // --- tick with no serve (falter) ---

    #[test]
    fn tick_drains_when_no_serve() {
        let mut y = y(); // falter_rate = 10
        y.serve(50.0);
        y.tick(0.016); // served=true this frame → no drain, just reset
        y.tick(1.0); // no serve → drain 10*1 = 10 → 40
        assert!((y.reliability - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_drain_when_served() {
        let mut y = y();
        y.serve(50.0);
        y.tick(1.0); // served this frame → no drain
        assert!((y.reliability - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_failed_on_empty() {
        let mut y = y();
        y.serve(5.0);
        y.tick(0.016); // served → no drain
        y.tick(1.0); // drain 10*1 → 0
        assert!(y.just_failed);
        assert!(y.is_failed());
    }

    #[test]
    fn tick_no_fail_when_already_zero() {
        let mut y = y();
        y.tick(1.0); // already 0
        assert!(!y.just_failed);
    }

    #[test]
    fn tick_clears_just_trusted() {
        let mut y = y();
        y.serve(100.0);
        y.serve(10.0); // keep served=true
        y.tick(0.016);
        assert!(!y.just_trusted);
    }

    #[test]
    fn tick_clears_just_failed() {
        let mut y = y();
        y.serve(5.0);
        y.tick(0.016);
        y.tick(1.0); // just_failed fires
        assert!(y.just_failed);
        y.tick(0.016); // cleared
        assert!(!y.just_failed);
    }

    #[test]
    fn tick_resets_served() {
        let mut y = y();
        y.serve(10.0);
        y.tick(0.016);
        assert!(!y.served);
    }

    #[test]
    fn tick_no_drain_when_disabled() {
        let mut y = y();
        y.serve(50.0);
        y.enabled = false;
        y.tick(1.0); // not enabled → no drain
        assert!((y.reliability - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_drain_when_falter_rate_zero() {
        let mut y = Yeoman::new(100.0, 0.0);
        y.serve(40.0);
        y.tick(0.016);
        y.tick(100.0); // no drain
        assert!((y.reliability - 40.0).abs() < 1e-3);
    }

    // --- sustained service ---

    #[test]
    fn sustained_service_builds_to_max() {
        let mut y = y();
        for _ in 0..10 {
            y.serve(15.0); // 10 × 15 = 150, clamps to 100
            y.tick(0.016);
        }
        assert!((y.reliability - 100.0).abs() < 1e-3);
    }

    #[test]
    fn missed_frames_erode_reliability() {
        let mut y = y();
        y.serve(80.0);
        y.tick(0.016); // served → no drain
                       // 3 frames of no service, 10/s drain, dt=0.1 → 3 drain
        for _ in 0..3 {
            y.tick(0.1);
        }
        assert!((y.reliability - 77.0).abs() < 0.1);
    }

    // --- is_trusted / is_failed ---

    #[test]
    fn is_trusted_false_when_below_max() {
        let mut y = y();
        y.serve(50.0);
        assert!(!y.is_trusted());
    }

    #[test]
    fn is_trusted_false_when_disabled() {
        let mut y = y();
        y.serve(100.0);
        y.enabled = false;
        assert!(!y.is_trusted());
    }

    #[test]
    fn is_failed_true_at_zero() {
        assert!(y().is_failed());
    }

    #[test]
    fn is_failed_true_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_failed()); // not gated
    }

    // --- fractions / effective ---

    #[test]
    fn reliability_fraction_zero_at_start() {
        assert_eq!(y().reliability_fraction(), 0.0);
    }

    #[test]
    fn reliability_fraction_half_at_midpoint() {
        let mut y = y();
        y.serve(50.0);
        assert!((y.reliability_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn reliability_fraction_one_at_max() {
        let mut y = y();
        y.serve(100.0);
        assert!((y.reliability_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_quality_zero_when_empty() {
        assert_eq!(y().effective_quality(100.0), 0.0);
    }

    #[test]
    fn effective_quality_scales_with_fraction() {
        let mut y = y();
        y.serve(75.0);
        assert!((y.effective_quality(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_quality_zero_when_disabled() {
        let mut y = y();
        y.serve(100.0);
        y.enabled = false;
        assert_eq!(y.effective_quality(100.0), 0.0);
    }
}
