use bevy_ecs::prelude::Component;

/// Interruptible periodic emitter. Fires `just_yakked` at a fixed
/// `yak_interval` cadence, but can be silenced mid-cadence via `silence()`.
/// Distinct from `Zulu` (pure metronome with no interrupt): Yak adds a
/// **silence window** — yaking is suppressed for a duration then resumes
/// from where it left off (elapsed is preserved through silence).
///
/// `silence(duration)` sets `silence_remaining` if enabled. Fires
/// `just_silenced` on first entry into silence (no-op if already silenced
/// or if duration <= 0).
///
/// `tick(dt)` clears one-frame flags first. If enabled and silenced: ticks
/// down `silence_remaining`, fires `just_unsilenced` when reaching 0, then
/// returns (no yak this frame). If enabled and not silenced: advances
/// `elapsed`; fires `just_yakked` and increments `yak_count` for each
/// completed interval (handles multiple crossings in one large dt).
///
/// `is_silenced()` returns `silence_remaining > 0.0 && enabled`.
///
/// `yak_fraction()` returns `(elapsed / yak_interval).clamp(0.0, 1.0)`.
///
/// `time_until_yak()` returns `(yak_interval - elapsed).max(0.0)` — time
/// to next yak assuming silence has ended.
///
/// `effective_chatter(base)` returns `base` when enabled and not silenced;
/// `0.0` otherwise.
///
/// Default: `new(2.0)` — yak every 2 seconds.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yak {
    /// Seconds between yaks. Clamped >= 0.01.
    pub yak_interval: f32,
    /// Time elapsed since last yak [0, yak_interval).
    pub elapsed: f32,
    /// Total yak count since creation.
    pub yak_count: u32,
    /// Remaining silence in seconds; 0 when not silenced.
    pub silence_remaining: f32,
    pub just_yakked: bool,
    pub just_silenced: bool,
    pub just_unsilenced: bool,
    pub enabled: bool,
}

impl Yak {
    pub fn new(yak_interval: f32) -> Self {
        Self {
            yak_interval: yak_interval.max(0.01),
            elapsed: 0.0,
            yak_count: 0,
            silence_remaining: 0.0,
            just_yakked: false,
            just_silenced: false,
            just_unsilenced: false,
            enabled: true,
        }
    }

    /// Suppress yakking for `duration` seconds. Fires `just_silenced` on
    /// first entry. No-op if already silenced, disabled, or duration <= 0.
    pub fn silence(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if self.silence_remaining <= 0.0 {
            self.just_silenced = true;
        }
        self.silence_remaining = duration;
    }

    /// Advance one frame. Clears flags, then ticks silence (if active) or
    /// advances toward next yak. Handles multiple interval crossings in one
    /// large dt.
    pub fn tick(&mut self, dt: f32) {
        self.just_yakked = false;
        self.just_silenced = false;
        self.just_unsilenced = false;

        if !self.enabled {
            return;
        }

        if self.silence_remaining > 0.0 {
            self.silence_remaining = (self.silence_remaining - dt).max(0.0);
            if self.silence_remaining == 0.0 {
                self.just_unsilenced = true;
            }
            return;
        }

        self.elapsed += dt;
        if self.elapsed >= self.yak_interval {
            let pulses = (self.elapsed / self.yak_interval) as u32;
            self.yak_count += pulses;
            self.elapsed -= self.yak_interval * pulses as f32;
            self.just_yakked = true;
        }
    }

    /// `true` when actively silenced and enabled.
    pub fn is_silenced(&self) -> bool {
        self.silence_remaining > 0.0 && self.enabled
    }

    /// Elapsed time as a fraction of the yak interval [0.0, 1.0].
    pub fn yak_fraction(&self) -> f32 {
        (self.elapsed / self.yak_interval).clamp(0.0, 1.0)
    }

    /// Time in seconds until the next yak, assuming silence has ended.
    pub fn time_until_yak(&self) -> f32 {
        (self.yak_interval - self.elapsed).max(0.0)
    }

    /// Returns `base` when enabled and not silenced; `0.0` otherwise.
    pub fn effective_chatter(&self, base: f32) -> f32 {
        if !self.enabled || self.silence_remaining > 0.0 {
            return 0.0;
        }
        base
    }
}

impl Default for Yak {
    fn default() -> Self {
        Self::new(2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yak {
        Yak::new(1.0) // 1-second interval
    }

    // --- construction ---

    #[test]
    fn new_starts_with_empty_state() {
        let y = y();
        assert!((y.yak_interval - 1.0).abs() < 1e-5);
        assert_eq!(y.elapsed, 0.0);
        assert_eq!(y.yak_count, 0);
        assert_eq!(y.silence_remaining, 0.0);
        assert!(!y.just_yakked);
        assert!(!y.just_silenced);
        assert!(!y.just_unsilenced);
    }

    #[test]
    fn interval_clamped_to_min() {
        let y = Yak::new(0.0);
        assert!((y.yak_interval - 0.01).abs() < 1e-6);
    }

    // --- tick: yaking ---

    #[test]
    fn tick_does_not_yak_before_interval() {
        let mut y = y();
        y.tick(0.5);
        assert!(!y.just_yakked);
        assert_eq!(y.yak_count, 0);
    }

    #[test]
    fn tick_yaks_at_interval() {
        let mut y = y(); // 1s interval
        y.tick(1.0);
        assert!(y.just_yakked);
        assert_eq!(y.yak_count, 1);
    }

    #[test]
    fn tick_yaks_crossing_interval() {
        let mut y = y();
        y.tick(0.7);
        y.tick(0.6); // total 1.3s > 1s
        assert!(y.just_yakked);
        assert_eq!(y.yak_count, 1);
    }

    #[test]
    fn tick_elapsed_wraps_correctly() {
        let mut y = y(); // 1s interval
        y.tick(1.3); // 1 yak, 0.3s carry-over
        assert!((y.elapsed - 0.3).abs() < 1e-4);
    }

    #[test]
    fn tick_multiple_yaks_in_large_dt() {
        let mut y = y(); // 1s interval
        y.tick(3.5); // 3 yaks, 0.5s carry-over
        assert!(y.just_yakked);
        assert_eq!(y.yak_count, 3);
        assert!((y.elapsed - 0.5).abs() < 1e-4);
    }

    #[test]
    fn tick_just_yakked_clears_next_frame() {
        let mut y = y();
        y.tick(1.0);
        y.tick(0.016);
        assert!(!y.just_yakked);
    }

    #[test]
    fn tick_no_yak_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.tick(2.0);
        assert!(!y.just_yakked);
        assert_eq!(y.elapsed, 0.0);
    }

    // --- silence ---

    #[test]
    fn silence_sets_remaining() {
        let mut y = y();
        y.silence(3.0);
        assert!((y.silence_remaining - 3.0).abs() < 1e-5);
        assert!(y.just_silenced);
    }

    #[test]
    fn silence_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.silence(3.0);
        assert_eq!(y.silence_remaining, 0.0);
        assert!(!y.just_silenced);
    }

    #[test]
    fn silence_no_op_for_zero_duration() {
        let mut y = y();
        y.silence(0.0);
        assert_eq!(y.silence_remaining, 0.0);
        assert!(!y.just_silenced);
    }

    #[test]
    fn silence_no_op_for_negative_duration() {
        let mut y = y();
        y.silence(-1.0);
        assert_eq!(y.silence_remaining, 0.0);
    }

    #[test]
    fn silence_does_not_refire_just_silenced_when_already_silenced() {
        let mut y = y();
        y.silence(3.0); // first silence
        y.just_silenced = false; // manually clear
        y.silence(5.0); // extend — already silenced, no re-fire
        assert!(!y.just_silenced);
        assert!((y.silence_remaining - 5.0).abs() < 1e-5); // updated
    }

    // --- tick: silenced ---

    #[test]
    fn tick_suppresses_yak_while_silenced() {
        let mut y = y();
        y.silence(2.0);
        y.tick(0.016); // clear just_silenced
        y.tick(1.5); // would yak at 1s, but silenced
        assert!(!y.just_yakked);
        assert_eq!(y.yak_count, 0);
    }

    #[test]
    fn tick_elapsed_not_advanced_while_silenced() {
        let mut y = y();
        y.elapsed = 0.3;
        y.silence(2.0);
        y.tick(1.0);
        assert!((y.elapsed - 0.3).abs() < 1e-5); // unchanged
    }

    #[test]
    fn tick_decrements_silence_remaining() {
        let mut y = y();
        y.silence(2.0);
        y.tick(0.5);
        assert!((y.silence_remaining - 1.5).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_unsilenced_when_silence_expires() {
        let mut y = y();
        y.silence(1.0);
        y.tick(0.016); // clear just_silenced
        y.tick(1.0); // silence expires
        assert!(y.just_unsilenced);
        assert_eq!(y.silence_remaining, 0.0);
    }

    #[test]
    fn tick_just_unsilenced_clears_next_frame() {
        let mut y = y();
        y.silence(1.0);
        y.tick(1.5); // silence expires
        y.tick(0.016);
        assert!(!y.just_unsilenced);
    }

    #[test]
    fn tick_resumes_yaking_after_silence() {
        let mut y = y();
        y.silence(1.0);
        y.tick(1.1); // silence expires, elapsed stays 0 (early return)
        y.tick(1.1); // elapsed 1.1s → crosses 1s interval → yak
        assert!(y.just_yakked);
    }

    // --- is_silenced ---

    #[test]
    fn is_silenced_false_initially() {
        let y = y();
        assert!(!y.is_silenced());
    }

    #[test]
    fn is_silenced_true_after_silence() {
        let mut y = y();
        y.silence(2.0);
        assert!(y.is_silenced());
    }

    #[test]
    fn is_silenced_false_when_disabled() {
        let mut y = y();
        y.silence_remaining = 1.0;
        y.enabled = false;
        assert!(!y.is_silenced());
    }

    // --- yak_fraction ---

    #[test]
    fn yak_fraction_zero_initially() {
        let y = y();
        assert_eq!(y.yak_fraction(), 0.0);
    }

    #[test]
    fn yak_fraction_at_half() {
        let mut y = y(); // 1s interval
        y.tick(0.5);
        assert!((y.yak_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn yak_fraction_wraps_after_yak() {
        let mut y = y();
        y.tick(1.3); // elapsed = 0.3 → fraction = 0.3
        assert!((y.yak_fraction() - 0.3).abs() < 1e-4);
    }

    // --- time_until_yak ---

    #[test]
    fn time_until_yak_one_interval_initially() {
        let y = y();
        assert!((y.time_until_yak() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn time_until_yak_decreases_with_elapsed() {
        let mut y = y();
        y.tick(0.7);
        assert!((y.time_until_yak() - 0.3).abs() < 1e-4);
    }

    // --- effective_chatter ---

    #[test]
    fn effective_chatter_returns_base_when_active() {
        let y = y();
        assert!((y.effective_chatter(42.0) - 42.0).abs() < 1e-5);
    }

    #[test]
    fn effective_chatter_zero_when_silenced() {
        let mut y = y();
        y.silence(1.0);
        assert_eq!(y.effective_chatter(42.0), 0.0);
    }

    #[test]
    fn effective_chatter_zero_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert_eq!(y.effective_chatter(42.0), 0.0);
    }

    // --- flag clearing ---

    #[test]
    fn tick_clears_just_silenced() {
        let mut y = y();
        y.silence(2.0); // sets just_silenced
        y.tick(0.016);
        assert!(!y.just_silenced);
    }
}
