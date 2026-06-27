use bevy_ecs::prelude::Component;

/// Satiation tracker. Stores current `fullness` in [0, `max_fullness`].
/// `eat(amount)` increases fullness; hunger drains it passively via
/// `tick(dt)`. Models feeding mechanics, stamina recovery gating, creature
/// behaviour thresholds, or any resource that depletes over time and
/// replenishes on consumption.
///
/// `eat(amount)` increases `fullness` by `amount`. Fires `just_sated` when
/// fullness first reaches `max_fullness`. No-op when disabled, already
/// fully sated, or `amount <= 0`.
///
/// `tick(dt)` clears `just_sated` and `just_starved`, then (when enabled)
/// drains `fullness` by `hunger_rate * dt`. Fires `just_starved` when
/// fullness reaches 0.
///
/// `is_sated()` returns `fullness >= max_fullness && enabled`.
///
/// `is_starving()` returns `fullness == 0.0` (not gated by `enabled`).
///
/// `fullness_fraction()` returns `(fullness / max_fullness).clamp(0, 1)`.
///
/// `effective_vigor(base)` returns `base * fullness_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — full at construction.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yum {
    pub fullness: f32,
    pub max_fullness: f32,
    pub hunger_rate: f32,
    pub just_sated: bool,
    pub just_starved: bool,
    pub enabled: bool,
}

impl Yum {
    pub fn new(max_fullness: f32, hunger_rate: f32) -> Self {
        let max_fullness = max_fullness.max(0.1);
        let hunger_rate = hunger_rate.max(0.0);
        Self {
            fullness: max_fullness,
            max_fullness,
            hunger_rate,
            just_sated: false,
            just_starved: false,
            enabled: true,
        }
    }

    /// Increase fullness. Fires `just_sated` on reaching `max_fullness`.
    /// No-op when disabled, already sated, or `amount <= 0`.
    pub fn eat(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fullness >= self.max_fullness {
            return;
        }
        self.fullness = (self.fullness + amount).min(self.max_fullness);
        if self.fullness >= self.max_fullness {
            self.just_sated = true;
        }
    }

    /// Advance one frame: clear flags, then drain fullness when enabled.
    /// Fires `just_starved` when fullness first reaches 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_sated = false;
        self.just_starved = false;
        if self.enabled && self.hunger_rate > 0.0 && self.fullness > 0.0 {
            self.fullness = (self.fullness - self.hunger_rate * dt).max(0.0);
            if self.fullness == 0.0 {
                self.just_starved = true;
            }
        }
    }

    /// `true` when fullness is at max and component is enabled.
    pub fn is_sated(&self) -> bool {
        self.fullness >= self.max_fullness && self.enabled
    }

    /// `true` when fullness is exactly 0 (not gated by `enabled`).
    pub fn is_starving(&self) -> bool {
        self.fullness == 0.0
    }

    /// Fraction of max fullness [0.0, 1.0].
    pub fn fullness_fraction(&self) -> f32 {
        (self.fullness / self.max_fullness).clamp(0.0, 1.0)
    }

    /// Returns `base * fullness_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vigor(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.fullness_fraction()
    }
}

impl Default for Yum {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yum {
        Yum::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_full() {
        let y = y();
        assert!((y.fullness - 100.0).abs() < 1e-5);
        assert!(y.is_sated());
        assert!(!y.is_starving());
    }

    #[test]
    fn new_clamps_max_fullness() {
        let y = Yum::new(-5.0, 1.0);
        assert!((y.max_fullness - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_hunger_rate() {
        let y = Yum::new(100.0, -3.0);
        assert_eq!(y.hunger_rate, 0.0);
    }

    #[test]
    fn default_uses_sensible_values() {
        let y = Yum::default();
        assert!((y.max_fullness - 100.0).abs() < 1e-5);
        assert!((y.hunger_rate - 5.0).abs() < 1e-5);
    }

    // --- eat ---

    #[test]
    fn eat_increases_fullness() {
        let mut y = y();
        y.fullness = 50.0;
        y.eat(20.0);
        assert!((y.fullness - 70.0).abs() < 1e-4);
    }

    #[test]
    fn eat_clamps_at_max() {
        let mut y = y();
        y.fullness = 80.0;
        y.eat(50.0);
        assert!((y.fullness - 100.0).abs() < 1e-5);
    }

    #[test]
    fn eat_fires_just_sated_at_max() {
        let mut y = y();
        y.fullness = 90.0;
        y.eat(20.0);
        assert!(y.just_sated);
    }

    #[test]
    fn eat_no_op_when_already_sated() {
        let mut y = y();
        y.eat(10.0); // already at max
        assert!(!y.just_sated);
    }

    #[test]
    fn eat_no_op_when_disabled() {
        let mut y = y();
        y.fullness = 50.0;
        y.enabled = false;
        y.eat(20.0);
        assert!((y.fullness - 50.0).abs() < 1e-5);
    }

    #[test]
    fn eat_no_op_for_zero_amount() {
        let mut y = y();
        y.fullness = 50.0;
        y.eat(0.0);
        assert!((y.fullness - 50.0).abs() < 1e-5);
    }

    // --- tick ---

    #[test]
    fn tick_drains_fullness() {
        let mut y = y();
        y.fullness = 50.0;
        y.tick(1.0);
        assert!((y.fullness - 40.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_starved_at_zero() {
        let mut y = y();
        y.fullness = 5.0;
        y.tick(1.0); // 5 - 10*1 = 0, clamped
        assert_eq!(y.fullness, 0.0);
        assert!(y.just_starved);
    }

    #[test]
    fn tick_clears_just_sated() {
        let mut y = y();
        y.fullness = 90.0;
        y.eat(20.0);
        y.tick(0.016);
        assert!(!y.just_sated);
    }

    #[test]
    fn tick_clears_just_starved() {
        let mut y = y();
        y.fullness = 5.0;
        y.tick(1.0);
        y.tick(0.016);
        assert!(!y.just_starved);
    }

    #[test]
    fn tick_does_not_drain_when_disabled() {
        let mut y = y();
        y.fullness = 50.0;
        y.enabled = false;
        y.tick(1.0);
        assert!((y.fullness - 50.0).abs() < 1e-5);
    }

    #[test]
    fn tick_does_not_drain_when_hunger_rate_zero() {
        let mut y = Yum::new(100.0, 0.0);
        y.fullness = 50.0;
        y.tick(10.0);
        assert!((y.fullness - 50.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_op_when_already_starving() {
        let mut y = y();
        y.fullness = 0.0;
        y.tick(1.0);
        assert!(!y.just_starved);
    }

    // --- is_sated / is_starving ---

    #[test]
    fn is_sated_true_when_full_and_enabled() {
        assert!(y().is_sated());
    }

    #[test]
    fn is_sated_false_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(!y.is_sated());
    }

    #[test]
    fn is_sated_false_when_partially_full() {
        let mut y = y();
        y.fullness = 50.0;
        assert!(!y.is_sated());
    }

    #[test]
    fn is_starving_true_when_empty() {
        let mut y = y();
        y.fullness = 0.0;
        assert!(y.is_starving());
    }

    #[test]
    fn is_starving_true_even_when_disabled() {
        let mut y = y();
        y.fullness = 0.0;
        y.enabled = false;
        assert!(y.is_starving());
    }

    // --- fractions / effective ---

    #[test]
    fn fullness_fraction_one_when_full() {
        assert!((y().fullness_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn fullness_fraction_half_at_midpoint() {
        let mut y = y();
        y.fullness = 50.0;
        assert!((y.fullness_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn fullness_fraction_zero_when_empty() {
        let mut y = y();
        y.fullness = 0.0;
        assert_eq!(y.fullness_fraction(), 0.0);
    }

    #[test]
    fn effective_vigor_full_when_sated() {
        assert!((y().effective_vigor(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_vigor_scales_with_fraction() {
        let mut y = y();
        y.fullness = 50.0;
        assert!((y.effective_vigor(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vigor_zero_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert_eq!(y.effective_vigor(100.0), 0.0);
    }
}
