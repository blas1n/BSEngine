use bevy_ecs::prelude::Component;

/// Two-phase quality accumulator. Models a resource that improves with age up
/// to a peak vintage, then deteriorates symmetrically past it — a tent-shaped
/// quality curve.
///
/// `tick(dt)` clears one-frame flags first, then advances `wine_age` by
/// `age_rate * dt` when enabled. Fires `just_peaked` the first time
/// `wine_age` crosses `peak_vintage`; fires `just_spoiled` the first time it
/// crosses `2.0 * peak_vintage`. No-op (beyond flag clear) when disabled.
///
/// `uncork()` resets `wine_age` to 0 (consume the wine and start a new
/// batch). No-op if already at 0.
///
/// `quality_fraction()` returns the tent-shaped quality value [0.0, 1.0]:
/// - Rising: `wine_age / peak_vintage` while age ≤ peak
/// - Falling: `1.0 - (wine_age - peak) / peak` while age > peak
/// - Spoiled (age > 2×peak): 0.0
///
/// `is_prime()` returns `quality_fraction() >= 0.5 && enabled` — within the
/// top half of the quality curve.
///
/// `is_spoiled()` returns `wine_age > 2.0 * peak_vintage && enabled`.
///
/// `effective_taste(base)` returns `base * (1.0 + quality_fraction())` when
/// enabled — up to 2× at peak; returns `base` unchanged when disabled.
///
/// Unlike all other accumulators in this library, Wine has **no start/stop
/// state** — it ages continuously once enabled. The two-phase tent curve is
/// also unique: every other component either increases monotonically (Worm,
/// Wrest) or can be reset via build/decay (Whelm). Wine can only be reset by
/// consuming it with `uncork()`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wine {
    /// Elapsed aging time. Increases continuously while enabled.
    pub wine_age: f32,
    /// Age at which quality peaks. Clamped >= 1.0.
    pub peak_vintage: f32,
    /// Aging rate per second. Clamped >= 0.0.
    pub age_rate: f32,
    pub just_peaked: bool,
    pub just_spoiled: bool,
    pub enabled: bool,
}

impl Wine {
    pub fn new(peak_vintage: f32, age_rate: f32) -> Self {
        Self {
            wine_age: 0.0,
            peak_vintage: peak_vintage.max(1.0),
            age_rate: age_rate.max(0.0),
            just_peaked: false,
            just_spoiled: false,
            enabled: true,
        }
    }

    /// Consume and reset: sets `wine_age` back to 0. No-op if already 0.
    pub fn uncork(&mut self) {
        if self.wine_age == 0.0 {
            return;
        }
        self.wine_age = 0.0;
    }

    /// Advance one frame: clear flags, then age by `age_rate * dt`.
    /// Fires `just_peaked` and `just_spoiled` at their respective crossings.
    /// No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_spoiled = false;

        if !self.enabled {
            return;
        }

        let prev = self.wine_age;
        self.wine_age += self.age_rate * dt;

        if prev < self.peak_vintage && self.wine_age >= self.peak_vintage {
            self.just_peaked = true;
        }
        if prev < 2.0 * self.peak_vintage && self.wine_age >= 2.0 * self.peak_vintage {
            self.just_spoiled = true;
        }
    }

    /// Tent-shaped quality [0.0, 1.0]. Rises linearly to 1.0 at peak_vintage,
    /// falls back to 0.0 at 2×peak_vintage, stays 0.0 thereafter.
    pub fn quality_fraction(&self) -> f32 {
        if self.wine_age <= self.peak_vintage {
            (self.wine_age / self.peak_vintage).clamp(0.0, 1.0)
        } else {
            let past = self.wine_age - self.peak_vintage;
            (1.0 - past / self.peak_vintage).clamp(0.0, 1.0)
        }
    }

    /// `true` when quality fraction >= 0.5 (in the top half of the curve)
    /// and component is enabled.
    pub fn is_prime(&self) -> bool {
        self.quality_fraction() >= 0.5 && self.enabled
    }

    /// `true` when wine has aged past twice the peak vintage (fully spoiled)
    /// and component is enabled.
    pub fn is_spoiled(&self) -> bool {
        self.wine_age > 2.0 * self.peak_vintage && self.enabled
    }

    /// Scale `base` by quality. Returns `base * (1.0 + quality_fraction())`
    /// when enabled (up to 2× at peak); `base` otherwise.
    pub fn effective_taste(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.quality_fraction())
    }
}

impl Default for Wine {
    fn default() -> Self {
        Self::new(60.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wine {
        Wine::new(10.0, 1.0) // peaks at 10s, spoils at 20s
    }

    #[test]
    fn new_starts_young() {
        let w = w();
        assert_eq!(w.wine_age, 0.0);
        assert!(!w.just_peaked);
        assert!(!w.just_spoiled);
        assert!(!w.is_prime());
        assert!(!w.is_spoiled());
    }

    #[test]
    fn tick_advances_age() {
        let mut w = w(); // age_rate=1.0
        w.tick(3.0); // 3.0
        assert!((w.wine_age - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(10.0);
        assert_eq!(w.wine_age, 0.0);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_peaked = true;
        w.just_spoiled = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_peaked);
        assert!(!w.just_spoiled);
    }

    #[test]
    fn tick_fires_just_peaked_at_peak_vintage() {
        let mut w = w(); // peak=10
        w.tick(10.0); // exactly at peak
        assert!(w.just_peaked);
    }

    #[test]
    fn tick_fires_just_peaked_crossing_peak() {
        let mut w = w();
        w.tick(7.0); // 7.0 — before peak
        w.tick(5.0); // crosses 10.0
        assert!(w.just_peaked);
    }

    #[test]
    fn tick_just_peaked_clears_next_frame() {
        let mut w = w();
        w.tick(10.0); // peaked
        w.tick(0.016); // cleared
        assert!(!w.just_peaked);
    }

    #[test]
    fn tick_fires_just_spoiled_at_two_peak() {
        let mut w = w(); // peak=10, spoils at 20
        w.tick(20.0);
        assert!(w.just_spoiled);
    }

    #[test]
    fn tick_fires_just_spoiled_crossing_two_peak() {
        let mut w = w();
        w.tick(15.0); // before 2×peak
        w.tick(6.0); // crosses 20.0
        assert!(w.just_spoiled);
    }

    #[test]
    fn tick_just_spoiled_clears_next_frame() {
        let mut w = w();
        w.tick(20.0); // spoiled
        w.tick(0.016);
        assert!(!w.just_spoiled);
    }

    #[test]
    fn tick_can_fire_both_just_peaked_and_just_spoiled_in_same_tick() {
        let mut w = w(); // peak=10
                         // One big tick covers both crossings
        w.tick(100.0);
        // Both should have fired (prev=0 < 10, and prev=0 < 20)
        assert!(w.just_peaked);
        assert!(w.just_spoiled);
    }

    #[test]
    fn uncork_resets_age() {
        let mut w = w();
        w.tick(5.0);
        w.uncork();
        assert_eq!(w.wine_age, 0.0);
    }

    #[test]
    fn uncork_no_op_when_already_zero() {
        let mut w = w();
        w.uncork(); // no-op
        assert_eq!(w.wine_age, 0.0);
    }

    #[test]
    fn quality_fraction_zero_when_young() {
        let w = w();
        assert_eq!(w.quality_fraction(), 0.0);
    }

    #[test]
    fn quality_fraction_half_at_half_peak() {
        let mut w = w(); // peak=10
        w.wine_age = 5.0; // 5/10 = 0.5
        assert!((w.quality_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn quality_fraction_one_at_peak() {
        let mut w = w();
        w.wine_age = 10.0;
        assert!((w.quality_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn quality_fraction_half_on_descent() {
        let mut w = w(); // peak=10, descends from 10→20
        w.wine_age = 15.0; // past=(15-10)=5, 1-5/10=0.5
        assert!((w.quality_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn quality_fraction_zero_when_spoiled() {
        let mut w = w();
        w.wine_age = 20.0; // exactly at 2×peak
        assert!((w.quality_fraction() - 0.0).abs() < 1e-4);
    }

    #[test]
    fn quality_fraction_zero_past_spoil() {
        let mut w = w();
        w.wine_age = 50.0; // way past
        assert_eq!(w.quality_fraction(), 0.0);
    }

    #[test]
    fn is_prime_true_at_peak() {
        let mut w = w();
        w.wine_age = 10.0; // quality=1.0 >= 0.5
        assert!(w.is_prime());
    }

    #[test]
    fn is_prime_true_above_half_quality_rising() {
        let mut w = w(); // peak=10
        w.wine_age = 5.0; // quality=0.5 → exactly at threshold
        assert!(w.is_prime());
    }

    #[test]
    fn is_prime_false_below_half_quality() {
        let mut w = w();
        w.wine_age = 4.9; // quality=0.49 < 0.5
        assert!(!w.is_prime());
    }

    #[test]
    fn is_prime_false_when_disabled() {
        let mut w = w();
        w.wine_age = 10.0;
        w.enabled = false;
        assert!(!w.is_prime());
    }

    #[test]
    fn is_spoiled_true_past_two_peak() {
        let mut w = w();
        w.wine_age = 20.01;
        assert!(w.is_spoiled());
    }

    #[test]
    fn is_spoiled_false_at_exactly_two_peak() {
        let mut w = w();
        w.wine_age = 20.0; // boundary: not strictly greater
        assert!(!w.is_spoiled());
    }

    #[test]
    fn is_spoiled_false_when_disabled() {
        let mut w = w();
        w.wine_age = 25.0;
        w.enabled = false;
        assert!(!w.is_spoiled());
    }

    #[test]
    fn effective_taste_base_when_young() {
        let w = w();
        assert!((w.effective_taste(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_taste_scaled_at_half_quality() {
        let mut w = w();
        w.wine_age = 5.0; // quality=0.5 → 100*(1+0.5)=150
        assert!((w.effective_taste(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_taste_doubled_at_peak_quality() {
        let mut w = w();
        w.wine_age = 10.0; // quality=1.0 → 100*(1+1.0)=200
        assert!((w.effective_taste(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_taste_base_when_spoiled() {
        let mut w = w();
        w.wine_age = 25.0; // quality=0.0 → passthrough
        assert!((w.effective_taste(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_taste_passthrough_when_disabled() {
        let mut w = w();
        w.wine_age = 10.0;
        w.enabled = false;
        assert!((w.effective_taste(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn peak_vintage_clamped_to_one() {
        let w = Wine::new(0.0, 1.0);
        assert!((w.peak_vintage - 1.0).abs() < 1e-5);
    }

    #[test]
    fn age_rate_clamped_to_zero() {
        let w = Wine::new(10.0, -1.0);
        assert_eq!(w.age_rate, 0.0);
    }

    #[test]
    fn uncork_then_age_again() {
        let mut w = w();
        w.tick(8.0); // 8.0 — rising
        w.uncork(); // reset
        w.tick(5.0); // 5.0 again
        assert!((w.wine_age - 5.0).abs() < 1e-4);
        assert!((w.quality_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn symmetry_of_tent_curve() {
        let mut w = w(); // peak=10
        w.wine_age = 3.0; // quality=0.3 (rising)
        let q_rising = w.quality_fraction();
        w.wine_age = 17.0; // past=(17-10)=7, 1-7/10=0.3 (falling)
        let q_falling = w.quality_fraction();
        assert!((q_rising - q_falling).abs() < 1e-4);
    }
}
