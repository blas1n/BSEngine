use bevy_ecs::prelude::Component;

/// Interval metronome: fires a pulse at a fixed time interval. Models
/// heartbeat timers, synchronized cooldowns, and "every N seconds" mechanics.
///
/// Distinct from `Timer` (one-shot countdown) and `Cooldown` (prevents
/// re-trigger during a window): Zulu fires **repeatedly** at a fixed cadence
/// with no external trigger required and no upper bound on pulses.
///
/// `tick(dt)` clears `just_pulsed`, then if enabled advances `elapsed` by
/// `dt`. Each time `elapsed` crosses `interval`, one pulse fires:
/// `pulse_count` increments, `just_pulsed = true`, and `elapsed` wraps
/// (multiple crossings in one large `dt` fire multiple pulses but set
/// `just_pulsed` only once). No-op (beyond flag clear) when disabled.
///
/// `reset()` sets `elapsed = 0` and `pulse_count = 0`. Does not require
/// `enabled`.
///
/// `time_until_pulse()` returns `(interval - elapsed).max(0.0)` — seconds
/// until the next pulse.
///
/// `pulse_fraction()` returns `(elapsed / interval).clamp(0.0, 1.0)` —
/// progress toward the next pulse.
///
/// `effective_tempo(base)` returns `base / interval` when enabled —
/// scales inversely with interval so a faster metronome yields higher output;
/// returns `0.0` when disabled.
///
/// Default: `new(1.0)` — pulses every 1 second.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zulu {
    /// Seconds between pulses. Clamped >= 0.01.
    pub interval: f32,
    /// Time elapsed since the last pulse (or since creation/reset).
    pub elapsed: f32,
    /// Total pulses fired since creation or last `reset()`.
    pub pulse_count: u32,
    pub just_pulsed: bool,
    pub enabled: bool,
}

impl Zulu {
    pub fn new(interval: f32) -> Self {
        Self {
            interval: interval.max(0.01),
            elapsed: 0.0,
            pulse_count: 0,
            just_pulsed: false,
            enabled: true,
        }
    }

    /// Advance one frame. Clears `just_pulsed`, then accumulates `elapsed`
    /// and fires pulses for each interval crossed. No-op beyond flag clear
    /// when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_pulsed = false;

        if !self.enabled {
            return;
        }

        self.elapsed += dt;
        if self.elapsed >= self.interval {
            let pulses = (self.elapsed / self.interval) as u32;
            self.pulse_count += pulses;
            self.elapsed -= self.interval * pulses as f32;
            self.just_pulsed = true;
        }
    }

    /// Reset elapsed time and pulse count to zero.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.pulse_count = 0;
    }

    /// Seconds until the next pulse.
    pub fn time_until_pulse(&self) -> f32 {
        (self.interval - self.elapsed).max(0.0)
    }

    /// Progress toward the next pulse as a fraction [0.0, 1.0].
    pub fn pulse_fraction(&self) -> f32 {
        (self.elapsed / self.interval).clamp(0.0, 1.0)
    }

    /// Scale `base` inversely by interval. Returns `base / interval` when
    /// enabled — faster cadence gives higher output; `0.0` when disabled.
    pub fn effective_tempo(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base / self.interval
    }
}

impl Default for Zulu {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zulu {
        Zulu::new(1.0) // pulses every 1.0 second
    }

    // --- construction ---

    #[test]
    fn new_starts_clean() {
        let z = z();
        assert_eq!(z.pulse_count, 0);
        assert_eq!(z.elapsed, 0.0);
        assert!(!z.just_pulsed);
        assert!(z.enabled);
    }

    #[test]
    fn interval_clamped_to_minimum() {
        let z = Zulu::new(0.0);
        assert!((z.interval - 0.01).abs() < 1e-6);
    }

    #[test]
    fn negative_interval_clamped() {
        let z = Zulu::new(-5.0);
        assert!((z.interval - 0.01).abs() < 1e-6);
    }

    // --- tick: basic accumulation ---

    #[test]
    fn tick_accumulates_elapsed() {
        let mut z = z();
        z.tick(0.5);
        assert!((z.elapsed - 0.5).abs() < 1e-5);
        assert_eq!(z.pulse_count, 0);
        assert!(!z.just_pulsed);
    }

    #[test]
    fn tick_fires_pulse_at_interval() {
        let mut z = z();
        z.tick(1.0);
        assert_eq!(z.pulse_count, 1);
        assert!(z.just_pulsed);
    }

    #[test]
    fn tick_wraps_elapsed_after_pulse() {
        let mut z = z();
        z.tick(1.3);
        assert!((z.elapsed - 0.3).abs() < 1e-5);
        assert_eq!(z.pulse_count, 1);
    }

    #[test]
    fn tick_fires_multiple_pulses_in_one_large_dt() {
        let mut z = z();
        z.tick(3.5); // 3 full intervals
        assert_eq!(z.pulse_count, 3);
        assert!(z.just_pulsed);
        assert!((z.elapsed - 0.5).abs() < 1e-4);
    }

    #[test]
    fn tick_just_pulsed_true_for_multiple_crossings() {
        let mut z = z();
        z.tick(5.0);
        assert!(z.just_pulsed); // true even for multiple pulses
    }

    // --- tick: flag lifecycle ---

    #[test]
    fn tick_clears_just_pulsed_next_frame() {
        let mut z = z();
        z.tick(1.0); // fires pulse
        z.tick(0.1); // no pulse, should clear
        assert!(!z.just_pulsed);
    }

    #[test]
    fn tick_clears_just_pulsed_even_without_pulse() {
        let mut z = z();
        z.just_pulsed = true;
        z.tick(0.1);
        assert!(!z.just_pulsed);
    }

    // --- tick: disabled ---

    #[test]
    fn tick_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(10.0);
        assert_eq!(z.elapsed, 0.0);
        assert_eq!(z.pulse_count, 0);
        assert!(!z.just_pulsed);
    }

    #[test]
    fn tick_clears_just_pulsed_when_disabled() {
        let mut z = z();
        z.just_pulsed = true;
        z.enabled = false;
        z.tick(0.016);
        assert!(!z.just_pulsed);
    }

    // --- tick: accumulated across frames ---

    #[test]
    fn pulse_fires_across_multiple_small_ticks() {
        let mut z = z();
        z.tick(0.4);
        z.tick(0.4);
        z.tick(0.4); // total = 1.2, crosses interval
        assert_eq!(z.pulse_count, 1);
        assert!(z.just_pulsed);
    }

    #[test]
    fn elapsed_accumulates_correctly() {
        let mut z = z(); // interval=1.0
        z.tick(0.3);
        z.tick(0.3);
        z.tick(0.3); // total=0.9
        assert!((z.elapsed - 0.9).abs() < 1e-5);
        assert_eq!(z.pulse_count, 0);
    }

    // --- reset ---

    #[test]
    fn reset_clears_elapsed_and_count() {
        let mut z = z();
        z.tick(2.5);
        z.reset();
        assert_eq!(z.elapsed, 0.0);
        assert_eq!(z.pulse_count, 0);
    }

    #[test]
    fn reset_does_not_clear_just_pulsed() {
        let mut z = z();
        z.tick(1.0); // fires just_pulsed
        z.reset();
        assert!(z.just_pulsed); // reset doesn't clear flags
    }

    #[test]
    fn reset_works_when_disabled() {
        let mut z = z();
        z.tick(1.5);
        z.enabled = false;
        z.reset();
        assert_eq!(z.elapsed, 0.0);
        assert_eq!(z.pulse_count, 0);
    }

    #[test]
    fn reset_allows_normal_ticking_after() {
        let mut z = z();
        z.tick(3.0); // 3 pulses
        z.reset();
        z.tick(1.0); // fresh pulse after reset
        assert_eq!(z.pulse_count, 1);
        assert!(z.just_pulsed);
    }

    // --- time_until_pulse ---

    #[test]
    fn time_until_pulse_full_at_start() {
        let z = z(); // interval=1.0, elapsed=0
        assert!((z.time_until_pulse() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn time_until_pulse_decreases_with_elapsed() {
        let mut z = z();
        z.tick(0.6);
        assert!((z.time_until_pulse() - 0.4).abs() < 1e-5);
    }

    #[test]
    fn time_until_pulse_floors_at_zero() {
        let mut z = z();
        z.elapsed = 0.0; // just after pulse fired and wrapped
        assert!(z.time_until_pulse() >= 0.0);
    }

    // --- pulse_fraction ---

    #[test]
    fn pulse_fraction_zero_at_start() {
        let z = z();
        assert_eq!(z.pulse_fraction(), 0.0);
    }

    #[test]
    fn pulse_fraction_at_half_interval() {
        let mut z = z();
        z.tick(0.5);
        assert!((z.pulse_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn pulse_fraction_approaches_one_before_pulse() {
        let mut z = z();
        z.tick(0.99);
        assert!((z.pulse_fraction() - 0.99).abs() < 1e-4);
    }

    #[test]
    fn pulse_fraction_resets_after_pulse() {
        let mut z = z();
        z.tick(1.5); // pulse fired, 0.5 remaining
        assert!((z.pulse_fraction() - 0.5).abs() < 1e-4);
    }

    // --- effective_tempo ---

    #[test]
    fn effective_tempo_at_default_interval() {
        let z = z(); // interval=1.0 → base/1.0=base
        assert!((z.effective_tempo(60.0) - 60.0).abs() < 1e-4);
    }

    #[test]
    fn effective_tempo_faster_interval_higher_output() {
        let z = Zulu::new(0.5); // interval=0.5 → base/0.5=2*base
        assert!((z.effective_tempo(60.0) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn effective_tempo_slower_interval_lower_output() {
        let z = Zulu::new(2.0); // interval=2.0 → base/2.0=0.5*base
        assert!((z.effective_tempo(60.0) - 30.0).abs() < 1e-3);
    }

    #[test]
    fn effective_tempo_zero_when_disabled() {
        let mut z = z();
        z.enabled = false;
        assert_eq!(z.effective_tempo(60.0), 0.0);
    }

    // --- multi-interval cycle ---

    #[test]
    fn counts_pulses_over_many_ticks() {
        let mut z = Zulu::new(0.5); // pulses every 0.5s
        for _ in 0..10 {
            z.tick(0.5); // 10 ticks × 0.5s = 5 seconds → 10 pulses
        }
        assert_eq!(z.pulse_count, 10);
    }

    #[test]
    fn re_enable_after_disable_resumes_from_same_elapsed() {
        let mut z = z();
        z.tick(0.4); // 0.4 elapsed
        z.enabled = false;
        z.tick(10.0); // disabled — elapsed stays at 0.4
        z.enabled = true;
        z.tick(0.7); // 0.4 + 0.7 = 1.1 → 1 pulse
        assert_eq!(z.pulse_count, 1);
    }
}
