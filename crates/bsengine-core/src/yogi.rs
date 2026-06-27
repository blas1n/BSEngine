use bevy_ecs::prelude::Component;

/// Meditation-focus tracker. `depth` builds each tick only when `meditate()`
/// was called that same frame (marking active practice). Without a `meditate()`
/// call, depth passively drifts back down at `drift_rate` per second.
/// Explicit distraction via `distract(amount)` also reduces depth immediately.
///
/// The per-frame opt-in via `meditate()` is mechanically distinct from
/// passive accumulators: the caller must actively signal sustained practice
/// each frame or depth erodes, mirroring how real focus works.
///
/// Models meditation depth, concentration meters, flow-state gauges, NPC
/// contemplation bars, or any system where focus requires active maintenance
/// and lapses cause gradual regression.
///
/// `meditate()` marks this frame as a practice frame. No-op when disabled.
/// Does NOT directly change `depth` — that happens in `tick()`.
///
/// `distract(amount)` immediately reduces `depth` when above 0. Fires
/// `just_scattered` when reaching 0. No-op when disabled.
///
/// `tick(dt)` clears `just_transcended` and `just_scattered`. Then:
///  - If `meditate()` was called and `focus_rate > 0`: grow depth by
///    `focus_rate * dt` (capped at max). Fires `just_transcended` at max.
///  - Otherwise, if `drift_rate > 0`: reduce depth by `drift_rate * dt`
///    (floored at 0). Fires `just_scattered` at 0.
///  Finally resets `meditating`.
///
/// `is_transcended()` returns `depth >= max_depth && enabled`.
///
/// `is_scattered()` returns `depth == 0.0` (not gated by `enabled`).
///
/// `depth_fraction()` returns `(depth / max_depth).clamp(0, 1)`.
///
/// `effective_serenity(base)` returns `base * depth_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 20.0, 5.0)` — builds at 20/sec while meditating,
/// drifts at 5/sec otherwise.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yogi {
    pub depth: f32,
    pub max_depth: f32,
    pub focus_rate: f32,
    pub drift_rate: f32,
    /// Set by `meditate()` this frame; cleared by `tick()`.
    pub meditating: bool,
    pub just_transcended: bool,
    pub just_scattered: bool,
    pub enabled: bool,
}

impl Yogi {
    pub fn new(max_depth: f32, focus_rate: f32, drift_rate: f32) -> Self {
        Self {
            depth: 0.0,
            max_depth: max_depth.max(0.1),
            focus_rate: focus_rate.max(0.0),
            drift_rate: drift_rate.max(0.0),
            meditating: false,
            just_transcended: false,
            just_scattered: false,
            enabled: true,
        }
    }

    /// Mark this frame as active practice. No-op when disabled.
    pub fn meditate(&mut self) {
        if !self.enabled {
            return;
        }
        self.meditating = true;
    }

    /// Scatter focus immediately; fires `just_scattered` when reaching 0.
    /// No-op when disabled or depth already 0.
    pub fn distract(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.depth <= 0.0 {
            return;
        }
        self.depth = (self.depth - amount).max(0.0);
        if self.depth <= 0.0 {
            self.just_scattered = true;
        }
    }

    /// Advance one frame: clear flags, then grow depth when meditating or
    /// drift it when not. Resets `meditating` at end of frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_transcended = false;
        self.just_scattered = false;
        if self.enabled {
            if self.meditating {
                if self.focus_rate > 0.0 && self.depth < self.max_depth {
                    let prev_below_max = self.depth < self.max_depth;
                    self.depth = (self.depth + self.focus_rate * dt).min(self.max_depth);
                    if prev_below_max && self.depth >= self.max_depth {
                        self.just_transcended = true;
                    }
                }
            } else if self.drift_rate > 0.0 && self.depth > 0.0 {
                self.depth = (self.depth - self.drift_rate * dt).max(0.0);
                if self.depth <= 0.0 {
                    self.just_scattered = true;
                }
            }
        }
        self.meditating = false;
    }

    /// `true` when depth is at maximum and component is enabled.
    pub fn is_transcended(&self) -> bool {
        self.depth >= self.max_depth && self.enabled
    }

    /// `true` when depth is 0 (not gated by `enabled`).
    pub fn is_scattered(&self) -> bool {
        self.depth == 0.0
    }

    /// Fraction of maximum depth [0.0, 1.0].
    pub fn depth_fraction(&self) -> f32 {
        (self.depth / self.max_depth).clamp(0.0, 1.0)
    }

    /// Returns `base * depth_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_serenity(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.depth_fraction()
    }
}

impl Default for Yogi {
    fn default() -> Self {
        Self::new(100.0, 20.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yogi {
        Yogi::new(100.0, 20.0, 5.0) // focus 20/sec, drift 5/sec
    }

    // --- construction ---

    #[test]
    fn new_starts_scattered() {
        let y = y();
        assert_eq!(y.depth, 0.0);
        assert!(y.is_scattered());
        assert!(!y.is_transcended());
    }

    #[test]
    fn new_clamps_max_depth() {
        let y = Yogi::new(-5.0, 10.0, 1.0);
        assert!((y.max_depth - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_focus_rate() {
        let y = Yogi::new(100.0, -5.0, 1.0);
        assert_eq!(y.focus_rate, 0.0);
    }

    #[test]
    fn new_clamps_drift_rate() {
        let y = Yogi::new(100.0, 10.0, -3.0);
        assert_eq!(y.drift_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yogi::default();
        assert!((y.max_depth - 100.0).abs() < 1e-5);
        assert!((y.focus_rate - 20.0).abs() < 1e-5);
        assert!((y.drift_rate - 5.0).abs() < 1e-5);
    }

    // --- meditate ---

    #[test]
    fn meditate_sets_meditating_flag() {
        let mut y = y();
        y.meditate();
        assert!(y.meditating);
    }

    #[test]
    fn meditate_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.meditate();
        assert!(!y.meditating);
    }

    // --- tick with meditate ---

    #[test]
    fn tick_grows_depth_when_meditating() {
        let mut y = y(); // focus_rate=20
        y.meditate();
        y.tick(1.0); // 0 + 20*1 = 20
        assert!((y.depth - 20.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_depth_at_max() {
        let mut y = y();
        y.meditate();
        y.tick(100.0); // overshoots max
        assert!((y.depth - 100.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_transcended_on_reaching_max() {
        let mut y = Yogi::new(100.0, 50.0, 0.0);
        y.distract(0.0); // keep at 0
        y.depth = 90.0;
        y.meditate();
        y.tick(1.0); // 90 + 50 > 100
        assert!(y.just_transcended);
        assert!(y.is_transcended());
    }

    #[test]
    fn tick_no_growth_when_already_transcended() {
        let mut y = Yogi::new(100.0, 20.0, 0.0);
        y.depth = 100.0;
        y.meditate();
        y.tick(1.0); // already at max
        assert!(!y.just_transcended);
    }

    #[test]
    fn tick_scales_focus_with_dt() {
        let mut y = y();
        y.meditate();
        y.tick(2.0); // 0 + 20*2 = 40
        assert!((y.depth - 40.0).abs() < 1e-2);
    }

    #[test]
    fn tick_resets_meditating_flag() {
        let mut y = y();
        y.meditate();
        y.tick(0.016);
        assert!(!y.meditating);
    }

    // --- tick without meditate (drift) ---

    #[test]
    fn tick_drifts_when_not_meditating() {
        let mut y = y(); // drift_rate=5
        y.depth = 50.0;
        y.tick(1.0); // 50 - 5*1 = 45
        assert!((y.depth - 45.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_scattered_on_drift_to_zero() {
        let mut y = Yogi::new(100.0, 20.0, 100.0);
        y.depth = 5.0;
        y.tick(1.0); // drifts past 0
        assert!(y.just_scattered);
        assert!(y.is_scattered());
    }

    #[test]
    fn tick_no_drift_when_already_scattered() {
        let mut y = y();
        y.tick(100.0); // depth=0
        assert!(!y.just_scattered);
    }

    #[test]
    fn tick_no_drift_when_rate_zero() {
        let mut y = Yogi::new(100.0, 20.0, 0.0);
        y.depth = 50.0;
        y.tick(100.0); // no drift
        assert!((y.depth - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_drift_when_disabled() {
        let mut y = y();
        y.depth = 50.0;
        y.enabled = false;
        y.tick(1.0); // disabled, no drift
        assert!((y.depth - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_transcended() {
        let mut y = Yogi::new(100.0, 50.0, 0.0);
        y.depth = 90.0;
        y.meditate();
        y.tick(1.0); // just_transcended fires
        y.tick(0.016); // cleared
        assert!(!y.just_transcended);
    }

    #[test]
    fn tick_clears_just_scattered() {
        let mut y = Yogi::new(100.0, 20.0, 100.0);
        y.depth = 5.0;
        y.tick(1.0); // just_scattered fires
        y.tick(0.016); // cleared
        assert!(!y.just_scattered);
    }

    // --- distract ---

    #[test]
    fn distract_reduces_depth() {
        let mut y = y();
        y.depth = 70.0;
        y.distract(20.0);
        assert!((y.depth - 50.0).abs() < 1e-3);
    }

    #[test]
    fn distract_clamps_at_zero() {
        let mut y = y();
        y.depth = 30.0;
        y.distract(200.0);
        assert_eq!(y.depth, 0.0);
    }

    #[test]
    fn distract_fires_just_scattered() {
        let mut y = y();
        y.depth = 30.0;
        y.distract(30.0);
        assert!(y.just_scattered);
    }

    #[test]
    fn distract_no_op_when_scattered() {
        let mut y = y();
        y.distract(10.0); // already 0
        assert!(!y.just_scattered);
    }

    #[test]
    fn distract_no_op_when_disabled() {
        let mut y = y();
        y.depth = 50.0;
        y.enabled = false;
        y.distract(50.0);
        assert!((y.depth - 50.0).abs() < 1e-3);
    }

    // --- sustained practice ---

    #[test]
    fn sustained_practice_builds_depth() {
        let mut y = Yogi::new(100.0, 20.0, 0.0);
        for _ in 0..5 {
            y.meditate();
            y.tick(1.0); // 5 * 20 = 100
        }
        assert!((y.depth - 100.0).abs() < 1e-3);
    }

    #[test]
    fn missed_frame_causes_drift() {
        let mut y = y();
        y.depth = 50.0;
        y.meditate();
        y.tick(0.016); // served → grow
        y.tick(0.016); // missed → drift
        assert!(y.depth < 50.0 + 20.0 * 0.016); // drifted back slightly
    }

    // --- is_transcended / is_scattered ---

    #[test]
    fn is_transcended_false_when_disabled() {
        let mut y = y();
        y.depth = 100.0;
        y.enabled = false;
        assert!(!y.is_transcended());
    }

    #[test]
    fn is_scattered_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_scattered());
    }

    // --- fractions / effective ---

    #[test]
    fn depth_fraction_zero_when_scattered() {
        assert_eq!(y().depth_fraction(), 0.0);
    }

    #[test]
    fn depth_fraction_half_at_midpoint() {
        let mut y = y();
        y.depth = 50.0;
        assert!((y.depth_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_serenity_zero_when_scattered() {
        assert_eq!(y().effective_serenity(100.0), 0.0);
    }

    #[test]
    fn effective_serenity_scales_with_depth() {
        let mut y = y();
        y.depth = 75.0;
        assert!((y.effective_serenity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_serenity_zero_when_disabled() {
        let mut y = y();
        y.depth = 50.0;
        y.enabled = false;
        assert_eq!(y.effective_serenity(100.0), 0.0);
    }
}
