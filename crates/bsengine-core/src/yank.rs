use bevy_ecs::prelude::Component;

/// Per-frame impulse accumulator. Collects sudden-pull forces during a frame
/// and exposes them to systems (e.g. physics) before clearing on `tick()`.
/// Tracks the highest single-frame total ever recorded as `peak`.
///
/// Models grapple pulls, rope jerks, magnetic attraction spikes, wind gusts,
/// or any brief force event that multiple systems may contribute to and
/// physics must read once per frame.
///
/// `yank(force)` adds to `impulse` for this frame. Fires `just_yanked` on
/// the first call per frame. Updates `peak` when the new total exceeds it.
/// No-op when disabled or `force <= 0`.
///
/// `tick(_dt)` clears `impulse` and `just_yanked`. `peak` persists.
///
/// `is_yanking()` returns `impulse > 0.0 && enabled`.
///
/// `impulse_fraction()` returns `(impulse / peak).clamp(0, 1)` when
/// `peak > 0`; `0.0` otherwise. Measures this frame's pull relative to the
/// historical maximum.
///
/// `effective_pull(base)` returns `base * impulse_fraction()` when enabled
/// and yanking; `0.0` otherwise.
///
/// Default: `new()` — zero impulse, zero peak.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yank {
    /// Accumulated pull this frame. Cleared by `tick()`.
    pub impulse: f32,
    /// Highest single-frame accumulated impulse ever recorded.
    pub peak: f32,
    pub just_yanked: bool,
    pub enabled: bool,
}

impl Yank {
    pub fn new() -> Self {
        Self {
            impulse: 0.0,
            peak: 0.0,
            just_yanked: false,
            enabled: true,
        }
    }

    /// Add `force` to this frame's impulse. Updates `peak` if the new total
    /// exceeds it. Fires `just_yanked` on first call this frame. No-op when
    /// disabled or `force <= 0`.
    pub fn yank(&mut self, force: f32) {
        if !self.enabled || force <= 0.0 {
            return;
        }
        self.impulse += force;
        self.just_yanked = true;
        if self.impulse > self.peak {
            self.peak = self.impulse;
        }
    }

    /// Advance one frame: clear `impulse` and `just_yanked`. `peak` is kept.
    pub fn tick(&mut self, _dt: f32) {
        self.impulse = 0.0;
        self.just_yanked = false;
    }

    /// `true` when a pull is active this frame and component is enabled.
    pub fn is_yanking(&self) -> bool {
        self.impulse > 0.0 && self.enabled
    }

    /// Ratio of current impulse to historical peak [0.0, 1.0]. `0.0` when
    /// no peak has been recorded.
    pub fn impulse_fraction(&self) -> f32 {
        if self.peak <= 0.0 {
            return 0.0;
        }
        (self.impulse / self.peak).clamp(0.0, 1.0)
    }

    /// Returns `base * impulse_fraction()` when enabled and yanking; `0.0`
    /// otherwise.
    pub fn effective_pull(&self, base: f32) -> f32 {
        if !self.is_yanking() {
            return 0.0;
        }
        base * self.impulse_fraction()
    }
}

impl Default for Yank {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yank {
        Yank::new()
    }

    // --- construction ---

    #[test]
    fn new_starts_idle() {
        let y = y();
        assert_eq!(y.impulse, 0.0);
        assert_eq!(y.peak, 0.0);
        assert!(!y.just_yanked);
        assert!(!y.is_yanking());
    }

    #[test]
    fn default_same_as_new() {
        let y = Yank::default();
        assert_eq!(y.impulse, 0.0);
        assert_eq!(y.peak, 0.0);
    }

    // --- yank ---

    #[test]
    fn yank_accumulates_impulse() {
        let mut y = y();
        y.yank(30.0);
        assert!((y.impulse - 30.0).abs() < 1e-4);
    }

    #[test]
    fn yank_multiple_calls_accumulate() {
        let mut y = y();
        y.yank(20.0);
        y.yank(15.0);
        assert!((y.impulse - 35.0).abs() < 1e-4);
    }

    #[test]
    fn yank_fires_just_yanked() {
        let mut y = y();
        y.yank(10.0);
        assert!(y.just_yanked);
    }

    #[test]
    fn yank_updates_peak() {
        let mut y = y();
        y.yank(40.0);
        assert!((y.peak - 40.0).abs() < 1e-4);
    }

    #[test]
    fn yank_peak_records_highest_frame_total() {
        let mut y = y();
        y.yank(30.0);
        y.yank(20.0); // frame total = 50
        y.tick(0.016);
        y.yank(10.0); // frame total = 10 < 50
        assert!((y.peak - 50.0).abs() < 1e-4);
    }

    #[test]
    fn yank_new_peak_when_higher() {
        let mut y = y();
        y.yank(30.0);
        y.tick(0.016);
        y.yank(50.0); // new peak
        assert!((y.peak - 50.0).abs() < 1e-4);
    }

    #[test]
    fn yank_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.yank(50.0);
        assert_eq!(y.impulse, 0.0);
        assert!(!y.just_yanked);
    }

    #[test]
    fn yank_no_op_for_zero_force() {
        let mut y = y();
        y.yank(0.0);
        assert_eq!(y.impulse, 0.0);
        assert!(!y.just_yanked);
    }

    #[test]
    fn yank_no_op_for_negative_force() {
        let mut y = y();
        y.yank(-10.0);
        assert_eq!(y.impulse, 0.0);
    }

    // --- tick ---

    #[test]
    fn tick_clears_impulse() {
        let mut y = y();
        y.yank(40.0);
        y.tick(0.016);
        assert_eq!(y.impulse, 0.0);
    }

    #[test]
    fn tick_clears_just_yanked() {
        let mut y = y();
        y.yank(10.0);
        y.tick(0.016);
        assert!(!y.just_yanked);
    }

    #[test]
    fn tick_preserves_peak() {
        let mut y = y();
        y.yank(60.0);
        y.tick(0.016);
        assert!((y.peak - 60.0).abs() < 1e-4);
    }

    #[test]
    fn tick_without_yank_stays_idle() {
        let mut y = y();
        y.tick(0.016);
        assert_eq!(y.impulse, 0.0);
        assert!(!y.just_yanked);
    }

    // --- is_yanking ---

    #[test]
    fn is_yanking_true_with_impulse() {
        let mut y = y();
        y.yank(10.0);
        assert!(y.is_yanking());
    }

    #[test]
    fn is_yanking_false_when_cleared() {
        let mut y = y();
        y.yank(10.0);
        y.tick(0.016);
        assert!(!y.is_yanking());
    }

    #[test]
    fn is_yanking_false_when_disabled() {
        let mut y = y();
        y.impulse = 50.0; // set directly to bypass guard
        y.enabled = false;
        assert!(!y.is_yanking());
    }

    // --- impulse_fraction ---

    #[test]
    fn impulse_fraction_zero_with_no_peak() {
        assert_eq!(y().impulse_fraction(), 0.0);
    }

    #[test]
    fn impulse_fraction_one_at_peak() {
        let mut y = y();
        y.yank(80.0); // sets peak = 80, impulse = 80
        assert!((y.impulse_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn impulse_fraction_half_when_below_peak() {
        let mut y = y();
        y.yank(100.0); // peak = 100
        y.tick(0.016);
        y.yank(50.0); // impulse = 50, peak = 100
        assert!((y.impulse_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn impulse_fraction_zero_when_idle() {
        let mut y = y();
        y.yank(50.0);
        y.tick(0.016); // cleared
        assert_eq!(y.impulse_fraction(), 0.0);
    }

    // --- effective_pull ---

    #[test]
    fn effective_pull_zero_when_idle() {
        assert_eq!(y().effective_pull(100.0), 0.0);
    }

    #[test]
    fn effective_pull_full_when_at_peak() {
        let mut y = y();
        y.yank(50.0);
        assert!((y.effective_pull(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_pull_half_when_below_peak() {
        let mut y = y();
        y.yank(100.0); // peak = 100
        y.tick(0.016);
        y.yank(50.0);
        assert!((y.effective_pull(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_pull_zero_when_disabled() {
        let mut y = y();
        y.yank(50.0);
        y.enabled = false;
        assert_eq!(y.effective_pull(100.0), 0.0);
    }
}
