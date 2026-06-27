use bevy_ecs::prelude::Component;

/// Warmth accumulator with heating and cooling phases. Warmth rises at
/// `heat_rate` per second while `is_heating` is true, and falls at `cool_rate`
/// per second otherwise. Models campfire proximity, cold-zone damage, survival
/// heat meters, or any proximity-based escalating effect.
///
/// `begin_heating()` activates heating mode. No-op when disabled.
///
/// `stop_heating()` deactivates heating mode regardless of `enabled`.
///
/// `tick(dt)` clears `just_peaked` and `just_frosted` first. When enabled and
/// heating: adds `heat_rate * dt` to warmth, fires `just_peaked` on crossing
/// `max_warmth`. When enabled and not heating: subtracts `cool_rate * dt`,
/// fires `just_frosted` when warmth reaches 0.
///
/// `is_warm()` returns `warmth >= max_warmth && enabled`.
///
/// `is_cold()` returns `warmth == 0.0` (not gated by enabled).
///
/// `warmth_fraction()` returns `(warmth / max_warmth).clamp(0.0, 1.0)`.
///
/// `effective_warmth(base)` returns `base * warmth_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(100.0, 10.0, 5.0)`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yule {
    /// Current warmth level. Clamped to [0, max_warmth].
    pub warmth: f32,
    /// Maximum warmth capacity. Clamped >= 0.1.
    pub max_warmth: f32,
    /// Warmth gained per second while heating. Clamped >= 0.0.
    pub heat_rate: f32,
    /// Warmth lost per second while not heating. Clamped >= 0.0.
    pub cool_rate: f32,
    /// Whether this entity is currently in a heat source.
    pub is_heating: bool,
    pub just_peaked: bool,
    pub just_frosted: bool,
    pub enabled: bool,
}

impl Yule {
    pub fn new(max_warmth: f32, heat_rate: f32, cool_rate: f32) -> Self {
        Self {
            warmth: 0.0,
            max_warmth: max_warmth.max(0.1),
            heat_rate: heat_rate.max(0.0),
            cool_rate: cool_rate.max(0.0),
            is_heating: false,
            just_peaked: false,
            just_frosted: false,
            enabled: true,
        }
    }

    /// Enter heat source. No-op when disabled.
    pub fn begin_heating(&mut self) {
        if !self.enabled {
            return;
        }
        self.is_heating = true;
    }

    /// Leave heat source. Always clears heating regardless of enabled state.
    pub fn stop_heating(&mut self) {
        self.is_heating = false;
    }

    /// Advance one frame: clear flags, then heat or cool if enabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_frosted = false;
        if !self.enabled {
            return;
        }
        if self.is_heating {
            if self.warmth < self.max_warmth {
                self.warmth = (self.warmth + self.heat_rate * dt).min(self.max_warmth);
                if self.warmth >= self.max_warmth {
                    self.just_peaked = true;
                }
            }
        } else if self.warmth > 0.0 {
            self.warmth = (self.warmth - self.cool_rate * dt).max(0.0);
            if self.warmth == 0.0 {
                self.just_frosted = true;
            }
        }
    }

    /// `true` when warmth has reached maximum and component is enabled.
    pub fn is_warm(&self) -> bool {
        self.warmth >= self.max_warmth && self.enabled
    }

    /// `true` when warmth is 0 (regardless of enabled).
    pub fn is_cold(&self) -> bool {
        self.warmth == 0.0
    }

    /// Warmth as a fraction of maximum [0.0, 1.0].
    pub fn warmth_fraction(&self) -> f32 {
        if self.max_warmth <= 0.0 {
            return 0.0;
        }
        (self.warmth / self.max_warmth).clamp(0.0, 1.0)
    }

    /// Returns `base * warmth_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_warmth(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.warmth_fraction()
    }
}

impl Default for Yule {
    fn default() -> Self {
        Self::new(100.0, 10.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yule {
        Yule::new(100.0, 10.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_cold_and_idle() {
        let y = y();
        assert_eq!(y.warmth, 0.0);
        assert!(!y.is_heating);
        assert!(!y.just_peaked);
        assert!(!y.just_frosted);
        assert!(y.is_cold());
    }

    #[test]
    fn max_warmth_clamped_to_point_one() {
        let y = Yule::new(-5.0, 1.0, 1.0);
        assert!((y.max_warmth - 0.1).abs() < 1e-5);
    }

    #[test]
    fn rates_clamped_to_zero() {
        let y = Yule::new(10.0, -3.0, -7.0);
        assert_eq!(y.heat_rate, 0.0);
        assert_eq!(y.cool_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yule::default();
        assert!((y.max_warmth - 100.0).abs() < 1e-5);
        assert!((y.heat_rate - 10.0).abs() < 1e-5);
        assert!((y.cool_rate - 5.0).abs() < 1e-5);
    }

    // --- begin_heating / stop_heating ---

    #[test]
    fn begin_heating_sets_flag() {
        let mut y = y();
        y.begin_heating();
        assert!(y.is_heating);
    }

    #[test]
    fn begin_heating_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.begin_heating();
        assert!(!y.is_heating);
    }

    #[test]
    fn stop_heating_clears_flag() {
        let mut y = y();
        y.begin_heating();
        y.stop_heating();
        assert!(!y.is_heating);
    }

    #[test]
    fn stop_heating_works_when_disabled() {
        let mut y = y();
        y.is_heating = true;
        y.enabled = false;
        y.stop_heating();
        assert!(!y.is_heating);
    }

    // --- tick: heat path ---

    #[test]
    fn tick_heats_when_heating_and_enabled() {
        let mut y = y(); // heat_rate=10
        y.begin_heating();
        y.tick(1.0);
        assert!((y.warmth - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clamps_warmth_at_max() {
        let mut y = y();
        y.warmth = 99.0;
        y.begin_heating();
        y.tick(100.0); // would overshoot
        assert!((y.warmth - 100.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_peaked_on_crossing_max() {
        let mut y = y();
        y.warmth = 95.0;
        y.begin_heating();
        y.tick(1.0); // 95+10=100 => peaked
        assert!(y.just_peaked);
        assert!(y.is_warm());
    }

    #[test]
    fn tick_does_not_refire_just_peaked_already_at_max() {
        let mut y = y();
        y.warmth = 100.0; // already at max
        y.begin_heating();
        y.tick(1.0);
        assert!(!y.just_peaked);
    }

    #[test]
    fn tick_zero_heat_rate_does_not_change_warmth() {
        let mut y = Yule::new(100.0, 0.0, 5.0);
        y.begin_heating();
        y.tick(10.0);
        assert_eq!(y.warmth, 0.0);
    }

    #[test]
    fn tick_does_not_heat_when_disabled() {
        let mut y = y();
        y.is_heating = true; // set directly, skip begin_heating gate
        y.enabled = false;
        y.tick(5.0);
        assert_eq!(y.warmth, 0.0);
    }

    // --- tick: cool path ---

    #[test]
    fn tick_cools_when_not_heating_and_enabled() {
        let mut y = y(); // cool_rate=5
        y.warmth = 50.0;
        y.tick(2.0); // not heating
        assert!((y.warmth - 40.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clamps_warmth_at_zero() {
        let mut y = y();
        y.warmth = 2.0;
        y.tick(100.0);
        assert_eq!(y.warmth, 0.0);
    }

    #[test]
    fn tick_fires_just_frosted_when_reaching_zero() {
        let mut y = y(); // cool_rate=5
        y.warmth = 3.0;
        y.tick(1.0); // 3-5 => 0
        assert!(y.just_frosted);
        assert!(y.is_cold());
    }

    #[test]
    fn tick_does_not_fire_just_frosted_already_at_zero() {
        let mut y = y();
        y.warmth = 0.0;
        y.tick(1.0);
        assert!(!y.just_frosted);
    }

    #[test]
    fn tick_zero_cool_rate_does_not_change_warmth() {
        let mut y = Yule::new(100.0, 10.0, 0.0);
        y.warmth = 50.0;
        y.tick(100.0);
        assert!((y.warmth - 50.0).abs() < 1e-5);
    }

    #[test]
    fn tick_does_not_cool_when_disabled() {
        let mut y = y();
        y.warmth = 50.0;
        y.enabled = false;
        y.tick(5.0);
        assert!((y.warmth - 50.0).abs() < 1e-5);
    }

    // --- tick: flag clearing ---

    #[test]
    fn tick_clears_just_peaked_at_start() {
        let mut y = y();
        y.just_peaked = true;
        y.tick(0.016);
        assert!(!y.just_peaked);
    }

    #[test]
    fn tick_clears_just_frosted_at_start() {
        let mut y = y();
        y.just_frosted = true;
        y.tick(0.016);
        assert!(!y.just_frosted);
    }

    // --- is_warm / is_cold ---

    #[test]
    fn is_warm_false_below_max() {
        let mut y = y();
        y.warmth = 99.9;
        assert!(!y.is_warm());
    }

    #[test]
    fn is_warm_true_at_max() {
        let mut y = y();
        y.warmth = 100.0;
        assert!(y.is_warm());
    }

    #[test]
    fn is_warm_false_when_disabled() {
        let mut y = y();
        y.warmth = 100.0;
        y.enabled = false;
        assert!(!y.is_warm());
    }

    #[test]
    fn is_cold_true_at_zero() {
        assert!(y().is_cold());
    }

    #[test]
    fn is_cold_false_above_zero() {
        let mut y = y();
        y.warmth = 0.001;
        assert!(!y.is_cold());
    }

    // --- warmth_fraction ---

    #[test]
    fn warmth_fraction_zero_when_cold() {
        assert_eq!(y().warmth_fraction(), 0.0);
    }

    #[test]
    fn warmth_fraction_one_at_max() {
        let mut y = y();
        y.warmth = 100.0;
        assert!((y.warmth_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn warmth_fraction_half_at_midpoint() {
        let mut y = y();
        y.warmth = 50.0;
        assert!((y.warmth_fraction() - 0.5).abs() < 1e-4);
    }

    // --- effective_warmth ---

    #[test]
    fn effective_warmth_zero_when_cold() {
        assert_eq!(y().effective_warmth(100.0), 0.0);
    }

    #[test]
    fn effective_warmth_full_at_max() {
        let mut y = y();
        y.warmth = 100.0;
        assert!((y.effective_warmth(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_warmth_half_at_midpoint() {
        let mut y = y();
        y.warmth = 50.0;
        assert!((y.effective_warmth(80.0) - 40.0).abs() < 1e-3);
    }

    #[test]
    fn effective_warmth_zero_when_disabled() {
        let mut y = y();
        y.warmth = 100.0;
        y.enabled = false;
        assert_eq!(y.effective_warmth(100.0), 0.0);
    }

    // --- heat-to-peak-to-cool cycle ---

    #[test]
    fn full_warmth_cycle() {
        let mut y = Yule::new(10.0, 10.0, 10.0);
        y.begin_heating();
        y.tick(1.0); // warmth=10, just_peaked
        assert!(y.just_peaked);
        y.stop_heating();
        y.tick(0.5); // warmth=5
        assert!((y.warmth - 5.0).abs() < 1e-4);
        y.tick(1.0); // warmth=0, just_frosted
        assert!(y.just_frosted);
        assert!(y.is_cold());
    }
}
