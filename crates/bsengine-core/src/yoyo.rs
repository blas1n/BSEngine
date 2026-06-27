use bevy_ecs::prelude::Component;

/// Self-reversing triangular-wave oscillator. Level ramps linearly from 0 to
/// `max_yoyo` then immediately back to 0, repeating indefinitely — a
/// continuous triangular wave driven purely by `tick()`.
///
/// Unlike `Oscillate` (sinusoidal, position-based) and `Wine`
/// (tent-shaped, one-shot until `uncork()`), Yoyo is **continuous and
/// automatic** — it never stops reversing unless disabled, and its output
/// curve is triangular (linear ramp in both directions), not sinusoidal.
///
/// `tick(dt)` clears one-frame flags first, then if enabled: moves
/// `yoyo_level` by `travel_rate * dt` in `going_up` direction. Clamps and
/// reverses direction at both boundaries:
/// - At `max_yoyo`: clamps to max, sets `going_up = false`, fires
///   `just_peaked`.
/// - At 0.0: clamps to 0, sets `going_up = true`, fires `just_bottomed`.
///
/// `reset()` resets `yoyo_level` to 0.0 and `going_up` to true. No-op when
/// already at 0 going up.
///
/// `yoyo_fraction()` returns `(yoyo_level / max_yoyo).clamp(0.0, 1.0)`.
///
/// `effective_swing(base)` returns `base * yoyo_fraction()` when enabled —
/// oscillates between 0 and `base` in sync with the wave. Returns `base`
/// unchanged when disabled.
///
/// Default: `new(1.0, 1.0)` — ramps 0→1 in 1 second, then 1→0 in 1 second
/// (full period = 2 seconds).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yoyo {
    /// Current wave level [0, max_yoyo].
    pub yoyo_level: f32,
    /// Maximum wave amplitude. Clamped >= 1.0.
    pub max_yoyo: f32,
    /// Travel rate in both directions (units/second). Clamped >= 0.0.
    pub travel_rate: f32,
    /// Current direction: `true` = rising toward max, `false` = falling to 0.
    pub going_up: bool,
    pub just_peaked: bool,
    pub just_bottomed: bool,
    pub enabled: bool,
}

impl Yoyo {
    pub fn new(max_yoyo: f32, travel_rate: f32) -> Self {
        Self {
            yoyo_level: 0.0,
            max_yoyo: max_yoyo.max(1.0),
            travel_rate: travel_rate.max(0.0),
            going_up: true,
            just_peaked: false,
            just_bottomed: false,
            enabled: true,
        }
    }

    /// Reset to origin: `yoyo_level = 0.0`, `going_up = true`. No-op when
    /// already at 0 going up.
    pub fn reset(&mut self) {
        if self.yoyo_level == 0.0 && self.going_up {
            return;
        }
        self.yoyo_level = 0.0;
        self.going_up = true;
    }

    /// Advance one frame: clear flags, then step the wave. Reverses at
    /// boundaries. No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_bottomed = false;

        if !self.enabled {
            return;
        }

        if self.going_up {
            self.yoyo_level += self.travel_rate * dt;
            if self.yoyo_level >= self.max_yoyo {
                self.yoyo_level = self.max_yoyo;
                self.going_up = false;
                self.just_peaked = true;
            }
        } else {
            self.yoyo_level -= self.travel_rate * dt;
            if self.yoyo_level <= 0.0 {
                self.yoyo_level = 0.0;
                self.going_up = true;
                self.just_bottomed = true;
            }
        }
    }

    /// Wave level as a fraction of maximum [0.0, 1.0].
    pub fn yoyo_fraction(&self) -> f32 {
        (self.yoyo_level / self.max_yoyo).clamp(0.0, 1.0)
    }

    /// Scale `base` by current wave fraction. Returns `base * yoyo_fraction()`
    /// when enabled — oscillates between 0 and `base`; returns `base` when
    /// disabled.
    pub fn effective_swing(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * self.yoyo_fraction()
    }
}

impl Default for Yoyo {
    fn default() -> Self {
        Self::new(1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Yoyo {
        Yoyo::new(10.0, 1.0) // max=10, 1 unit/s
    }

    #[test]
    fn new_starts_at_zero_going_up() {
        let w = w();
        assert_eq!(w.yoyo_level, 0.0);
        assert!(w.going_up);
        assert!(!w.just_peaked);
        assert!(!w.just_bottomed);
    }

    // --- tick (rising) ---

    #[test]
    fn tick_rises_when_going_up() {
        let mut w = w();
        w.tick(3.0); // 3.0
        assert!((w.yoyo_level - 3.0).abs() < 1e-4);
        assert!(w.going_up);
    }

    #[test]
    fn tick_fires_just_peaked_at_max() {
        let mut w = w();
        w.tick(10.0); // exactly at max
        assert!(w.just_peaked);
        assert!(!w.going_up);
        assert!((w.yoyo_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_peaked_crossing_max() {
        let mut w = w();
        w.tick(7.0); // 7.0
        w.tick(5.0); // crosses 10.0
        assert!(w.just_peaked);
        assert!(!w.going_up);
    }

    #[test]
    fn tick_just_peaked_clears_next_frame() {
        let mut w = w();
        w.tick(10.0);
        w.tick(0.016);
        assert!(!w.just_peaked);
    }

    // --- tick (falling) ---

    #[test]
    fn tick_falls_after_peak() {
        let mut w = w();
        w.tick(10.0); // at max, going_up=false
        w.tick(3.0); // 10 - 3 = 7
        assert!((w.yoyo_level - 7.0).abs() < 1e-4);
        assert!(!w.going_up);
    }

    #[test]
    fn tick_fires_just_bottomed_at_zero() {
        let mut w = w();
        w.tick(10.0); // peak
        w.tick(10.0); // back to zero
        assert!(w.just_bottomed);
        assert!(w.going_up);
        assert_eq!(w.yoyo_level, 0.0);
    }

    #[test]
    fn tick_fires_just_bottomed_crossing_zero() {
        let mut w = w();
        w.tick(10.0); // peak
        w.tick(7.0); // 3.0
        w.tick(5.0); // crosses zero
        assert!(w.just_bottomed);
        assert!(w.going_up);
    }

    #[test]
    fn tick_just_bottomed_clears_next_frame() {
        let mut w = w();
        w.tick(10.0); // peak
        w.tick(10.0); // bottom
        w.tick(0.016);
        assert!(!w.just_bottomed);
    }

    // --- full cycle ---

    #[test]
    fn tick_completes_full_cycle() {
        let mut w = w(); // rate=1/s, max=10
        w.tick(10.0); // peak
        w.tick(10.0); // bottom (full cycle)
        assert_eq!(w.yoyo_level, 0.0);
        assert!(w.going_up); // starts rising again
    }

    #[test]
    fn tick_rises_again_after_bottom() {
        let mut w = w();
        w.tick(10.0); // peak
        w.tick(10.0); // bottom
        w.tick(4.0); // rising: 4.0
        assert!((w.yoyo_level - 4.0).abs() < 1e-4);
        assert!(w.going_up);
    }

    // --- disabled ---

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(10.0);
        assert_eq!(w.yoyo_level, 0.0);
        assert!(w.going_up);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_peaked = true;
        w.just_bottomed = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_peaked);
        assert!(!w.just_bottomed);
    }

    // --- reset ---

    #[test]
    fn reset_returns_to_origin() {
        let mut w = w();
        w.tick(5.0); // 5.0, going_up
        w.reset();
        assert_eq!(w.yoyo_level, 0.0);
        assert!(w.going_up);
    }

    #[test]
    fn reset_when_falling() {
        let mut w = w();
        w.tick(10.0); // peak, going_up=false
        w.reset();
        assert_eq!(w.yoyo_level, 0.0);
        assert!(w.going_up);
    }

    #[test]
    fn reset_no_op_when_already_at_origin() {
        let mut w = w();
        w.reset(); // already at 0, going_up=true
        assert_eq!(w.yoyo_level, 0.0);
        assert!(w.going_up);
    }

    // --- yoyo_fraction ---

    #[test]
    fn yoyo_fraction_zero_at_start() {
        let w = w();
        assert_eq!(w.yoyo_fraction(), 0.0);
    }

    #[test]
    fn yoyo_fraction_half_at_midpoint() {
        let mut w = w();
        w.tick(5.0); // 5/10=0.5
        assert!((w.yoyo_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn yoyo_fraction_one_at_peak() {
        let mut w = w();
        w.tick(10.0);
        assert!((w.yoyo_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn yoyo_fraction_half_on_descent() {
        let mut w = w();
        w.tick(10.0); // peak
        w.tick(5.0); // 5.0 — halfway down
        assert!((w.yoyo_fraction() - 0.5).abs() < 1e-4);
    }

    // --- effective_swing ---

    #[test]
    fn effective_swing_zero_at_start() {
        let w = w();
        assert!((w.effective_swing(100.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn effective_swing_half_at_midpoint() {
        let mut w = w();
        w.tick(5.0); // fraction=0.5 → 100*0.5=50
        assert!((w.effective_swing(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_swing_full_at_peak() {
        let mut w = w();
        w.tick(10.0); // fraction=1.0 → 100
        assert!((w.effective_swing(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_swing_passthrough_when_disabled() {
        let mut w = w();
        w.tick(5.0);
        w.enabled = false;
        assert!((w.effective_swing(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_yoyo_clamped_to_one() {
        let w = Yoyo::new(0.0, 1.0);
        assert!((w.max_yoyo - 1.0).abs() < 1e-5);
    }

    #[test]
    fn travel_rate_clamped_to_zero() {
        let w = Yoyo::new(10.0, -1.0);
        assert_eq!(w.travel_rate, 0.0);
    }

    #[test]
    fn zero_travel_rate_stays_at_origin() {
        let mut w = Yoyo::new(10.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.yoyo_level, 0.0);
        assert!(w.going_up);
        assert!(!w.just_peaked);
    }

    #[test]
    fn multiple_cycles_stay_bounded() {
        let mut w = w();
        // Run 5 complete cycles (10s each = 100s total)
        for _ in 0..100 {
            w.tick(1.0);
        }
        assert!(w.yoyo_level >= 0.0);
        assert!(w.yoyo_level <= 10.0 + 1e-4);
    }
}
