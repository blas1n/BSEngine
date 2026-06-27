use bevy_ecs::prelude::Component;

/// Idle-triggered fatigue detector. Tracks time since the last `wake()` call
/// and fires `just_yawned` once when `idle_time` crosses `yawn_threshold`.
/// Models player inactivity, NPC idle-state triggers, or stamina drain onset.
///
/// `wake()` resets `idle_time` to 0 and clears `is_drowsy`. No-op when
/// disabled.
///
/// `tick(dt)` clears `just_yawned` first, then if enabled increments
/// `idle_time`. Fires `just_yawned` and sets `is_drowsy` the first time
/// `idle_time` reaches or crosses `yawn_threshold`.
///
/// `is_alert()` returns `idle_time < yawn_threshold && enabled`.
///
/// `idle_fraction()` returns `(idle_time / yawn_threshold).clamp(0.0, 1.0)`.
///
/// `effective_drowsiness(base)` returns `base * idle_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(5.0)` — 5-second idle threshold.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yawn {
    /// Seconds of idle time before a yawn fires. Clamped >= 0.1.
    pub yawn_threshold: f32,
    /// Seconds elapsed since last `wake()`.
    pub idle_time: f32,
    pub just_yawned: bool,
    /// `true` once `idle_time >= yawn_threshold`.
    pub is_drowsy: bool,
    pub enabled: bool,
}

impl Yawn {
    pub fn new(yawn_threshold: f32) -> Self {
        Self {
            yawn_threshold: yawn_threshold.max(0.1),
            idle_time: 0.0,
            just_yawned: false,
            is_drowsy: false,
            enabled: true,
        }
    }

    /// Signal activity: reset idle timer and clear drowsy state. No-op when
    /// disabled.
    pub fn wake(&mut self) {
        if !self.enabled {
            return;
        }
        self.idle_time = 0.0;
        self.is_drowsy = false;
    }

    /// Advance one frame: clear `just_yawned`, then accumulate idle time.
    /// Fires `just_yawned` exactly once when crossing the threshold.
    pub fn tick(&mut self, dt: f32) {
        self.just_yawned = false;
        if !self.enabled {
            return;
        }
        let was_drowsy = self.is_drowsy;
        self.idle_time += dt;
        if !was_drowsy && self.idle_time >= self.yawn_threshold {
            self.just_yawned = true;
            self.is_drowsy = true;
        }
    }

    /// `true` when still alert (idle below threshold) and enabled.
    pub fn is_alert(&self) -> bool {
        self.idle_time < self.yawn_threshold && self.enabled
    }

    /// Idle progress as a fraction of threshold [0.0, 1.0].
    pub fn idle_fraction(&self) -> f32 {
        (self.idle_time / self.yawn_threshold).clamp(0.0, 1.0)
    }

    /// Returns `base * idle_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_drowsiness(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.idle_fraction()
    }
}

impl Default for Yawn {
    fn default() -> Self {
        Self::new(5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yawn {
        Yawn::new(1.0) // 1-second threshold
    }

    // --- construction ---

    #[test]
    fn new_starts_alert_and_idle() {
        let y = y();
        assert_eq!(y.idle_time, 0.0);
        assert!(!y.just_yawned);
        assert!(!y.is_drowsy);
        assert!(y.is_alert());
    }

    #[test]
    fn threshold_clamped_to_tenth() {
        let y = Yawn::new(0.0);
        assert!((y.yawn_threshold - 0.1).abs() < 1e-5);
    }

    // --- wake ---

    #[test]
    fn wake_resets_idle_time() {
        let mut y = y();
        y.tick(0.5);
        y.wake();
        assert_eq!(y.idle_time, 0.0);
    }

    #[test]
    fn wake_clears_drowsy() {
        let mut y = y();
        y.tick(1.0); // becomes drowsy
        y.wake();
        assert!(!y.is_drowsy);
    }

    #[test]
    fn wake_no_op_when_disabled() {
        let mut y = y();
        y.tick(0.5);
        y.enabled = false;
        y.wake();
        assert!((y.idle_time - 0.5).abs() < 1e-4);
    }

    // --- tick ---

    #[test]
    fn tick_increments_idle_time() {
        let mut y = y();
        y.tick(0.3);
        assert!((y.idle_time - 0.3).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_yawned_on_crossing_threshold() {
        let mut y = y();
        y.tick(1.0);
        assert!(y.just_yawned);
        assert!(y.is_drowsy);
    }

    #[test]
    fn tick_fires_just_yawned_crossing_from_below() {
        let mut y = y();
        y.tick(0.5);
        y.tick(0.6); // crosses 1.0
        assert!(y.just_yawned);
    }

    #[test]
    fn tick_does_not_refire_just_yawned() {
        let mut y = y();
        y.tick(1.0); // fires
        y.tick(0.016); // clears
        assert!(!y.just_yawned);
    }

    #[test]
    fn tick_does_not_fire_again_after_drowsy() {
        let mut y = y();
        y.tick(1.0); // fires, is_drowsy=true
        y.tick(1.0); // just_yawned cleared, already drowsy
        assert!(!y.just_yawned);
    }

    #[test]
    fn tick_clears_just_yawned_even_when_disabled() {
        let mut y = y();
        y.just_yawned = true;
        y.enabled = false;
        y.tick(0.016);
        assert!(!y.just_yawned);
    }

    #[test]
    fn tick_no_op_on_idle_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.tick(2.0);
        assert_eq!(y.idle_time, 0.0);
        assert!(!y.is_drowsy);
    }

    // --- is_alert ---

    #[test]
    fn is_alert_true_when_fresh() {
        assert!(y().is_alert());
    }

    #[test]
    fn is_alert_false_after_yawn() {
        let mut y = y();
        y.tick(1.0);
        assert!(!y.is_alert());
    }

    #[test]
    fn is_alert_true_after_wake() {
        let mut y = y();
        y.tick(1.0);
        y.wake();
        assert!(y.is_alert());
    }

    #[test]
    fn is_alert_false_when_disabled() {
        let y_disabled = {
            let mut y = y();
            y.enabled = false;
            y
        };
        assert!(!y_disabled.is_alert());
    }

    // --- idle_fraction ---

    #[test]
    fn idle_fraction_zero_when_fresh() {
        assert_eq!(y().idle_fraction(), 0.0);
    }

    #[test]
    fn idle_fraction_half_at_midpoint() {
        let mut y = y();
        y.tick(0.5);
        assert!((y.idle_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn idle_fraction_one_at_threshold() {
        let mut y = y();
        y.tick(1.0);
        assert!((y.idle_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn idle_fraction_clamped_above_threshold() {
        let mut y = y();
        y.tick(5.0);
        assert!((y.idle_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_drowsiness ---

    #[test]
    fn effective_drowsiness_zero_when_fresh() {
        let y = y();
        assert_eq!(y.effective_drowsiness(100.0), 0.0);
    }

    #[test]
    fn effective_drowsiness_scales_with_idle() {
        let mut y = y();
        y.tick(0.5); // fraction=0.5
        assert!((y.effective_drowsiness(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_drowsiness_max_at_threshold() {
        let mut y = y();
        y.tick(1.0);
        assert!((y.effective_drowsiness(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_drowsiness_zero_when_disabled() {
        let mut y = y();
        y.tick(1.0);
        y.enabled = false;
        assert_eq!(y.effective_drowsiness(100.0), 0.0);
    }

    // --- wake-yawn cycle ---

    #[test]
    fn wake_resets_and_allows_new_yawn() {
        let mut y = y();
        y.tick(1.0); // first yawn
        y.wake();
        y.tick(1.0); // second yawn
        assert!(y.just_yawned);
    }
}
