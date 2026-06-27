use bevy_ecs::prelude::Component;

/// Recurring seasonal-cycle tracker. `elapsed` advances through `period`
/// then wraps to 0. Fires `just_cycled` on each wrap and `just_new_season`
/// whenever the season index (0–3) changes or the cycle wraps.
///
/// Seasons are equal quarters of the period:
/// - 0 (spring): elapsed in [0, period/4)
/// - 1 (summer): elapsed in [period/4, period/2)
/// - 2 (autumn): elapsed in [period/2, 3*period/4)
/// - 3 (winter): elapsed in [3*period/4, period)
///
/// `advance(amount)` adds to elapsed when enabled; wraps and fires flags.
/// No-op when disabled or amount ≤ 0.
///
/// `tick(dt)` clears `just_cycled` and `just_new_season` at the start of each
/// frame, then calls `advance(dt)`.
///
/// `season()` returns the current season 0–3.
///
/// `year_fraction()` returns `elapsed / period` in [0.0, 1.0).
///
/// `season_fraction()` returns the fraction through the current season
/// [0.0, 1.0).
///
/// `is_season(s)` returns `season() == s`.
///
/// Default: `new(360.0)` — a 360-second year, starts at elapsed 0.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Year {
    pub elapsed: f32,
    pub period: f32,
    pub just_cycled: bool,
    pub just_new_season: bool,
    pub enabled: bool,
}

impl Year {
    pub fn new(period: f32) -> Self {
        Self {
            elapsed: 0.0,
            period: period.max(0.1),
            just_cycled: false,
            just_new_season: false,
            enabled: true,
        }
    }

    /// Add `amount` to elapsed. Wraps at `period`, fires `just_cycled`.
    /// Fires `just_new_season` when the season index changes or on wrap.
    /// No-op when disabled or amount ≤ 0.
    pub fn advance(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let prev_season = self.season();
        self.elapsed += amount;
        if self.elapsed >= self.period {
            self.elapsed %= self.period;
            self.just_cycled = true;
        }
        let new_season = self.season();
        if new_season != prev_season || self.just_cycled {
            self.just_new_season = true;
        }
    }

    /// Advance one frame: clear flags, then call `advance(dt)`.
    pub fn tick(&mut self, dt: f32) {
        self.just_cycled = false;
        self.just_new_season = false;
        self.advance(dt);
    }

    /// Current season: 0=spring, 1=summer, 2=autumn, 3=winter.
    pub fn season(&self) -> u32 {
        ((self.elapsed / self.period * 4.0) as u32).min(3)
    }

    /// Fraction of the full cycle elapsed [0.0, 1.0).
    pub fn year_fraction(&self) -> f32 {
        (self.elapsed / self.period).clamp(0.0, 1.0)
    }

    /// Fraction elapsed within the current season [0.0, 1.0).
    pub fn season_fraction(&self) -> f32 {
        (self.elapsed / self.period * 4.0).fract()
    }

    /// `true` when the current season matches `s`.
    pub fn is_season(&self, s: u32) -> bool {
        self.season() == s
    }
}

impl Default for Year {
    fn default() -> Self {
        Self::new(360.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Year {
        Year::new(100.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_at_zero() {
        let y = y();
        assert_eq!(y.elapsed, 0.0);
        assert_eq!(y.season(), 0);
        assert!(!y.just_cycled);
        assert!(!y.just_new_season);
    }

    #[test]
    fn new_clamps_period() {
        let y = Year::new(-5.0);
        assert!((y.period - 0.1).abs() < 1e-5);
    }

    #[test]
    fn default_values() {
        let y = Year::default();
        assert!((y.period - 360.0).abs() < 1e-5);
        assert_eq!(y.elapsed, 0.0);
    }

    // --- advance ---

    #[test]
    fn advance_increases_elapsed() {
        let mut y = y();
        y.advance(10.0);
        assert!((y.elapsed - 10.0).abs() < 1e-4);
    }

    #[test]
    fn advance_wraps_at_period() {
        let mut y = y();
        y.advance(110.0);
        assert!((y.elapsed - 10.0).abs() < 1e-3);
    }

    #[test]
    fn advance_fires_just_cycled_on_wrap() {
        let mut y = y();
        y.advance(100.0);
        assert!(y.just_cycled);
    }

    #[test]
    fn advance_no_just_cycled_below_period() {
        let mut y = y();
        y.advance(50.0);
        assert!(!y.just_cycled);
    }

    #[test]
    fn advance_fires_just_new_season_on_season_cross() {
        let mut y = y();
        y.advance(26.0); // crosses from season 0 (0-25) into season 1 (25-50)
        assert!(y.just_new_season);
        assert_eq!(y.season(), 1);
    }

    #[test]
    fn advance_fires_just_new_season_on_cycle() {
        let mut y = y();
        y.advance(75.0); // into season 3
        y.advance(30.0); // wraps — just_new_season fires
        assert!(y.just_cycled);
        assert!(y.just_new_season);
    }

    #[test]
    fn advance_no_just_new_season_within_season() {
        let mut y = y();
        y.advance(5.0); // season 0
        y.advance(5.0); // still season 0
        assert!(!y.just_new_season);
    }

    #[test]
    fn advance_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.advance(50.0);
        assert_eq!(y.elapsed, 0.0);
    }

    #[test]
    fn advance_no_op_for_zero_amount() {
        let mut y = y();
        y.advance(0.0);
        assert_eq!(y.elapsed, 0.0);
    }

    #[test]
    fn advance_accumulates() {
        let mut y = y();
        y.advance(20.0);
        y.advance(20.0);
        assert!((y.elapsed - 40.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_cycled() {
        let mut y = y();
        y.advance(100.0); // just_cycled
        y.tick(0.0); // clears (with zero advance, no-op on advance)
        assert!(!y.just_cycled);
    }

    #[test]
    fn tick_clears_just_new_season() {
        let mut y = y();
        y.advance(26.0); // just_new_season
        y.tick(0.001);
        assert!(!y.just_new_season);
    }

    #[test]
    fn tick_advances_elapsed() {
        let mut y = y();
        y.tick(30.0);
        assert!((y.elapsed - 30.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_cycled() {
        let mut y = y();
        y.tick(100.0);
        assert!(y.just_cycled);
    }

    #[test]
    fn tick_no_advance_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.tick(50.0);
        assert_eq!(y.elapsed, 0.0);
    }

    // --- season ---

    #[test]
    fn season_zero_at_start() {
        assert_eq!(y().season(), 0);
    }

    #[test]
    fn season_one_at_quarter() {
        let mut y = y();
        y.advance(25.0); // exactly quarter boundary
        assert_eq!(y.season(), 1);
    }

    #[test]
    fn season_two_at_half() {
        let mut y = y();
        y.advance(50.0);
        assert_eq!(y.season(), 2);
    }

    #[test]
    fn season_three_at_three_quarters() {
        let mut y = y();
        y.advance(75.0);
        assert_eq!(y.season(), 3);
    }

    #[test]
    fn season_returns_to_zero_after_cycle() {
        let mut y = y();
        y.advance(99.9);
        assert_eq!(y.season(), 3);
        y.advance(0.2); // wraps
        assert_eq!(y.season(), 0);
    }

    #[test]
    fn is_season_true_when_matching() {
        let y = y();
        assert!(y.is_season(0));
        assert!(!y.is_season(1));
    }

    #[test]
    fn is_season_summer() {
        let mut y = y();
        y.advance(30.0);
        assert!(y.is_season(1));
    }

    // --- year_fraction ---

    #[test]
    fn year_fraction_zero_at_start() {
        assert_eq!(y().year_fraction(), 0.0);
    }

    #[test]
    fn year_fraction_half_at_midpoint() {
        let mut y = y();
        y.advance(50.0);
        assert!((y.year_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn year_fraction_near_one_before_wrap() {
        let mut y = y();
        y.advance(99.0);
        assert!((y.year_fraction() - 0.99).abs() < 1e-3);
    }

    // --- season_fraction ---

    #[test]
    fn season_fraction_zero_at_season_start() {
        let mut y = y();
        y.advance(25.0); // exactly season 1 start
        assert!(y.season_fraction().abs() < 1e-4);
    }

    #[test]
    fn season_fraction_half_within_season() {
        let mut y = y();
        y.advance(12.5); // halfway through season 0 (0-25)
        assert!((y.season_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn season_fraction_transitions_smoothly() {
        let mut y = y();
        y.advance(37.5); // halfway through season 1 (25-50)
        assert_eq!(y.season(), 1);
        assert!((y.season_fraction() - 0.5).abs() < 1e-4);
    }
}
