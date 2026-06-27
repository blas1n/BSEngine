use bevy_ecs::prelude::Component;

/// Wrapping epoch/cycle tracker. `age` advances toward `max_age`; when it
/// reaches `max_age` it wraps back to 0, `epoch` increments, and
/// `just_cycled` fires for that tick.
///
/// Models cosmic time cycles, day/night counters with a total-days field,
/// seasonal loops, respawn cycles, or any repeating-period mechanic that
/// also needs to track how many full periods have elapsed.
///
/// `advance(amount)` immediately increases `age`. If the result reaches or
/// exceeds `max_age`, `age` wraps (`age -= max_age` repeated) and `epoch`
/// increments once per completed period, and `just_cycled` is set. No-op
/// when disabled or `amount <= 0`.
///
/// `tick(dt)` clears `just_cycled` then calls `advance(rate * dt)` when
/// `rate > 0`. The clear-then-set order means `just_cycled` is `true`
/// exactly on the tick in which a period completes.
///
/// `is_ancient()` returns `epoch > 0 && enabled` (has survived at least
/// one full cycle).
///
/// `cycle_fraction()` returns `(age / max_age).clamp(0, 1)`.
///
/// `effective_age(scale)` returns `scale * cycle_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — one epoch every 100 seconds at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yuga {
    pub age: f32,
    pub max_age: f32,
    pub rate: f32,
    pub epoch: u32,
    pub just_cycled: bool,
    pub enabled: bool,
}

impl Yuga {
    pub fn new(max_age: f32, rate: f32) -> Self {
        Self {
            age: 0.0,
            max_age: max_age.max(0.1),
            rate: rate.max(0.0),
            epoch: 0,
            just_cycled: false,
            enabled: true,
        }
    }

    /// Advance `age` by `amount`, wrapping epoch(s) as needed.
    /// No-op when disabled or `amount <= 0`.
    pub fn advance(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.age += amount;
        while self.age >= self.max_age {
            self.age -= self.max_age;
            self.epoch = self.epoch.saturating_add(1);
            self.just_cycled = true;
        }
    }

    /// Clear `just_cycled`, then advance by `rate * dt` when `rate > 0`.
    pub fn tick(&mut self, dt: f32) {
        self.just_cycled = false;
        if self.enabled && self.rate > 0.0 {
            self.advance(self.rate * dt);
        }
    }

    /// `true` when at least one full cycle has been completed and enabled.
    pub fn is_ancient(&self) -> bool {
        self.epoch > 0 && self.enabled
    }

    /// Fraction of the current cycle elapsed [0.0, 1.0].
    pub fn cycle_fraction(&self) -> f32 {
        (self.age / self.max_age).clamp(0.0, 1.0)
    }

    /// Returns `scale * cycle_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_age(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.cycle_fraction()
    }
}

impl Default for Yuga {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yuga {
        Yuga::new(100.0, 1.0) // max=100, rate=1/sec
    }

    // --- construction ---

    #[test]
    fn new_starts_at_zero() {
        let y = y();
        assert_eq!(y.age, 0.0);
        assert_eq!(y.epoch, 0);
        assert!(!y.just_cycled);
        assert!(!y.is_ancient());
    }

    #[test]
    fn new_clamps_max_age() {
        let y = Yuga::new(-5.0, 1.0);
        assert!((y.max_age - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_rate() {
        let y = Yuga::new(100.0, -3.0);
        assert_eq!(y.rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yuga::default();
        assert!((y.max_age - 100.0).abs() < 1e-5);
        assert!((y.rate - 1.0).abs() < 1e-5);
    }

    // --- advance ---

    #[test]
    fn advance_increases_age() {
        let mut y = y();
        y.advance(30.0);
        assert!((y.age - 30.0).abs() < 1e-3);
        assert_eq!(y.epoch, 0);
        assert!(!y.just_cycled);
    }

    #[test]
    fn advance_wraps_on_reaching_max() {
        let mut y = y();
        y.advance(100.0); // exactly one cycle
        assert!((y.age).abs() < 1e-3); // wraps to 0
        assert_eq!(y.epoch, 1);
        assert!(y.just_cycled);
    }

    #[test]
    fn advance_wraps_with_remainder() {
        let mut y = y();
        y.advance(110.0); // one full cycle + 10 leftover
        assert!((y.age - 10.0).abs() < 1e-3);
        assert_eq!(y.epoch, 1);
        assert!(y.just_cycled);
    }

    #[test]
    fn advance_multiple_wraps() {
        let mut y = y();
        y.advance(250.0); // 2.5 cycles
        assert!((y.age - 50.0).abs() < 1e-3);
        assert_eq!(y.epoch, 2);
        assert!(y.just_cycled);
    }

    #[test]
    fn advance_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.advance(200.0);
        assert_eq!(y.age, 0.0);
        assert_eq!(y.epoch, 0);
    }

    #[test]
    fn advance_no_op_when_amount_zero() {
        let mut y = y();
        y.advance(0.0);
        assert_eq!(y.age, 0.0);
        assert!(!y.just_cycled);
    }

    #[test]
    fn advance_no_op_when_amount_negative() {
        let mut y = y();
        y.advance(-10.0);
        assert_eq!(y.age, 0.0);
    }

    #[test]
    fn advance_accumulates_epoch_across_calls() {
        let mut y = y();
        y.advance(100.0); // epoch 1
        y.advance(100.0); // epoch 2
        assert_eq!(y.epoch, 2);
    }

    #[test]
    fn advance_does_not_set_just_cycled_without_wrap() {
        let mut y = y();
        y.advance(50.0);
        assert!(!y.just_cycled);
    }

    // --- tick ---

    #[test]
    fn tick_advances_by_rate_times_dt() {
        let mut y = y(); // rate=1
        y.tick(25.0); // 0 + 1*25 = 25
        assert!((y.age - 25.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_cycled_on_wrap() {
        let mut y = y();
        y.age = 90.0;
        y.tick(15.0); // 90 + 15 = 105 → wraps
        assert!(y.just_cycled);
        assert_eq!(y.epoch, 1);
    }

    #[test]
    fn tick_clears_just_cycled_next_frame() {
        let mut y = y();
        y.age = 90.0;
        y.tick(15.0); // just_cycled fires
        y.tick(1.0); // cleared
        assert!(!y.just_cycled);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.tick(100.0);
        assert_eq!(y.age, 0.0);
        assert_eq!(y.epoch, 0);
    }

    #[test]
    fn tick_no_op_when_rate_zero() {
        let mut y = Yuga::new(100.0, 0.0);
        y.age = 40.0;
        y.tick(100.0);
        assert!((y.age - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tick_large_dt_wraps_multiple_epochs() {
        let mut y = y();
        y.tick(350.0); // 3.5 cycles
        assert_eq!(y.epoch, 3);
        assert!((y.age - 50.0).abs() < 1e-2);
        assert!(y.just_cycled);
    }

    #[test]
    fn tick_clears_just_cycled_even_when_no_wrap() {
        let mut y = y();
        y.just_cycled = true;
        y.tick(1.0); // no wrap, but clears at start
        assert!(!y.just_cycled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = Yuga::new(100.0, 10.0);
        y.tick(3.0); // 10*3 = 30
        assert!((y.age - 30.0).abs() < 1e-3);
    }

    // --- is_ancient ---

    #[test]
    fn is_ancient_false_at_start() {
        assert!(!y().is_ancient());
    }

    #[test]
    fn is_ancient_true_after_cycle() {
        let mut y = y();
        y.advance(100.0);
        assert!(y.is_ancient());
    }

    #[test]
    fn is_ancient_gated_by_enabled() {
        let mut y = y();
        y.advance(100.0);
        y.enabled = false;
        assert!(!y.is_ancient());
    }

    // --- cycle_fraction ---

    #[test]
    fn cycle_fraction_zero_at_start() {
        assert_eq!(y().cycle_fraction(), 0.0);
    }

    #[test]
    fn cycle_fraction_half_at_midpoint() {
        let mut y = y();
        y.age = 50.0;
        assert!((y.cycle_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn cycle_fraction_one_at_max() {
        let mut y = y();
        y.age = 100.0;
        assert!((y.cycle_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_age ---

    #[test]
    fn effective_age_zero_at_start() {
        assert_eq!(y().effective_age(100.0), 0.0);
    }

    #[test]
    fn effective_age_scales_with_fraction() {
        let mut y = y();
        y.age = 75.0;
        assert!((y.effective_age(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_age_zero_when_disabled() {
        let mut y = y();
        y.age = 50.0;
        y.enabled = false;
        assert_eq!(y.effective_age(100.0), 0.0);
    }
}
