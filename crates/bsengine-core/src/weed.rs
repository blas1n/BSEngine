use bevy_ecs::prelude::Component;

/// Passive-only growth accumulator that requires explicit culling to reduce.
/// Models environmental contamination, resource competition, or background
/// nuisance that grows on its own and must be periodically cleared.
///
/// Unlike `Worn` (accepts both event-driven `degrade()` and passive
/// `wear_rate`), Weed **only grows passively** via `grow_rate` in `tick()`.
/// The only way to reduce weed is `cull(amount)` or `cull_all()`. This
/// models situations where an external agent (the player, a tool, a cooldown)
/// must periodically intervene — weed never reduces on its own.
///
/// `tick(dt)` clears one-frame flags first, then if enabled: adds
/// `grow_rate * dt` to `weed_level` (clamped to `max_weed`). Fires
/// `just_overgrown` the first time `weed_level` reaches `max_weed`.
///
/// `cull(amount)` reduces `weed_level` by `amount` (floored to 0.0). Fires
/// `just_cleared` when `weed_level` drops to 0.0. No-op when disabled or
/// already at 0.
///
/// `cull_all()` resets `weed_level` to 0.0 instantly. Fires `just_cleared`.
/// No-op when disabled or already at 0.
///
/// `is_overgrown()` returns `weed_level >= max_weed && enabled`.
///
/// `weed_fraction()` returns `(weed_level / max_weed).clamp(0.0, 1.0)`.
///
/// `effective_yield(base)` returns `base * (1.0 - weed_fraction())` when
/// enabled — weeds choke output toward 0 as they fill; `base` unchanged when
/// disabled.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weed {
    /// Current weed coverage [0, max_weed].
    pub weed_level: f32,
    /// Maximum weed coverage. Clamped >= 1.0.
    pub max_weed: f32,
    /// Passive growth rate per second. Clamped >= 0.0.
    pub grow_rate: f32,
    pub just_overgrown: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Weed {
    pub fn new(max_weed: f32, grow_rate: f32) -> Self {
        Self {
            weed_level: 0.0,
            max_weed: max_weed.max(1.0),
            grow_rate: grow_rate.max(0.0),
            just_overgrown: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Reduce weeds by `amount` (floored to 0). Fires `just_cleared` when
    /// dropping to 0. No-op when disabled or already at 0.
    pub fn cull(&mut self, amount: f32) {
        if !self.enabled || self.weed_level == 0.0 {
            return;
        }
        self.weed_level = (self.weed_level - amount).max(0.0);
        if self.weed_level == 0.0 {
            self.just_cleared = true;
        }
    }

    /// Remove all weeds instantly. Fires `just_cleared`. No-op when disabled
    /// or already at 0.
    pub fn cull_all(&mut self) {
        if !self.enabled || self.weed_level == 0.0 {
            return;
        }
        self.weed_level = 0.0;
        self.just_cleared = true;
    }

    /// Advance one frame: clear flags, then apply passive growth. Fires
    /// `just_overgrown` at first crossing of `max_weed`. No passive growth
    /// when disabled or `grow_rate == 0.0`.
    pub fn tick(&mut self, dt: f32) {
        self.just_overgrown = false;
        self.just_cleared = false;

        if !self.enabled {
            return;
        }

        let prev = self.weed_level;
        self.weed_level = (self.weed_level + self.grow_rate * dt).min(self.max_weed);

        if prev < self.max_weed && self.weed_level >= self.max_weed {
            self.just_overgrown = true;
        }
    }

    /// `true` when weeds are at maximum coverage and component is enabled.
    pub fn is_overgrown(&self) -> bool {
        self.weed_level >= self.max_weed && self.enabled
    }

    /// Weed coverage as a fraction of maximum [0.0, 1.0].
    pub fn weed_fraction(&self) -> f32 {
        (self.weed_level / self.max_weed).clamp(0.0, 1.0)
    }

    /// Scale `base` inversely by weed coverage. Returns
    /// `base * (1.0 - weed_fraction())` when enabled; `base` otherwise.
    pub fn effective_yield(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 - self.weed_fraction())
    }
}

impl Default for Weed {
    fn default() -> Self {
        Self::new(10.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Weed {
        Weed::new(10.0, 1.0) // max=10, grows 1/s
    }

    fn ws() -> Weed {
        Weed::new(10.0, 0.0) // no passive growth for manual tests
    }

    #[test]
    fn new_starts_clear() {
        let w = ws();
        assert_eq!(w.weed_level, 0.0);
        assert!(!w.just_overgrown);
        assert!(!w.just_cleared);
        assert!(!w.is_overgrown());
    }

    // --- tick (passive growth) ---

    #[test]
    fn tick_grows_passively() {
        let mut w = w(); // grow_rate=1.0
        w.tick(3.0); // 3.0
        assert!((w.weed_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_growth_when_rate_zero() {
        let mut w = ws();
        w.tick(100.0);
        assert_eq!(w.weed_level, 0.0);
    }

    #[test]
    fn tick_clamps_at_max() {
        let mut w = w();
        w.tick(100.0); // grow_rate=1.0, 100s → clamped to 10
        assert!((w.weed_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_overgrown_at_max() {
        let mut w = w();
        w.tick(10.0); // exactly at max
        assert!(w.just_overgrown);
    }

    #[test]
    fn tick_fires_just_overgrown_crossing_max() {
        let mut w = w();
        w.tick(7.0); // 7.0
        w.tick(5.0); // crosses 10.0
        assert!(w.just_overgrown);
    }

    #[test]
    fn tick_just_overgrown_clears_next_frame() {
        let mut w = w();
        w.tick(10.0);
        w.tick(0.016);
        assert!(!w.just_overgrown);
    }

    #[test]
    fn tick_does_not_refire_just_overgrown_at_cap() {
        let mut w = w();
        w.tick(10.0); // overgrown
        w.tick(0.016); // clear
        w.tick(1.0); // already at cap
        assert!(!w.just_overgrown);
    }

    #[test]
    fn tick_no_growth_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(10.0);
        assert_eq!(w.weed_level, 0.0);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = ws();
        w.just_overgrown = true;
        w.just_cleared = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_overgrown);
        assert!(!w.just_cleared);
    }

    // --- cull ---

    #[test]
    fn cull_reduces_level() {
        let mut w = ws();
        w.weed_level = 7.0;
        w.cull(3.0);
        assert!((w.weed_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn cull_floors_at_zero() {
        let mut w = ws();
        w.weed_level = 3.0;
        w.cull(10.0);
        assert_eq!(w.weed_level, 0.0);
    }

    #[test]
    fn cull_fires_just_cleared_when_reaching_zero() {
        let mut w = ws();
        w.weed_level = 5.0;
        w.cull(5.0); // exactly zero
        assert!(w.just_cleared);
    }

    #[test]
    fn cull_fires_just_cleared_when_overshooting_zero() {
        let mut w = ws();
        w.weed_level = 3.0;
        w.cull(10.0); // overshoots
        assert!(w.just_cleared);
    }

    #[test]
    fn cull_does_not_fire_just_cleared_partial() {
        let mut w = ws();
        w.weed_level = 7.0;
        w.cull(3.0); // still 4.0 remaining
        assert!(!w.just_cleared);
    }

    #[test]
    fn cull_no_op_when_already_zero() {
        let mut w = ws();
        w.cull(5.0);
        assert!(!w.just_cleared);
        assert_eq!(w.weed_level, 0.0);
    }

    #[test]
    fn cull_no_op_when_disabled() {
        let mut w = ws();
        w.weed_level = 5.0;
        w.enabled = false;
        w.cull(5.0);
        assert!((w.weed_level - 5.0).abs() < 1e-4);
        assert!(!w.just_cleared);
    }

    // --- cull_all ---

    #[test]
    fn cull_all_resets_to_zero() {
        let mut w = ws();
        w.weed_level = 8.0;
        w.cull_all();
        assert_eq!(w.weed_level, 0.0);
    }

    #[test]
    fn cull_all_fires_just_cleared() {
        let mut w = ws();
        w.weed_level = 5.0;
        w.cull_all();
        assert!(w.just_cleared);
    }

    #[test]
    fn cull_all_no_op_when_already_zero() {
        let mut w = ws();
        w.cull_all();
        assert!(!w.just_cleared);
    }

    #[test]
    fn cull_all_no_op_when_disabled() {
        let mut w = ws();
        w.weed_level = 5.0;
        w.enabled = false;
        w.cull_all();
        assert!((w.weed_level - 5.0).abs() < 1e-4);
        assert!(!w.just_cleared);
    }

    // --- is_overgrown / weed_fraction ---

    #[test]
    fn is_overgrown_true_at_max() {
        let mut w = ws();
        w.weed_level = 10.0;
        assert!(w.is_overgrown());
    }

    #[test]
    fn is_overgrown_false_below_max() {
        let mut w = ws();
        w.weed_level = 9.9;
        assert!(!w.is_overgrown());
    }

    #[test]
    fn is_overgrown_false_when_disabled() {
        let mut w = ws();
        w.weed_level = 10.0;
        w.enabled = false;
        assert!(!w.is_overgrown());
    }

    #[test]
    fn weed_fraction_zero_when_clear() {
        let w = ws();
        assert_eq!(w.weed_fraction(), 0.0);
    }

    #[test]
    fn weed_fraction_half_at_midpoint() {
        let mut w = ws();
        w.weed_level = 5.0;
        assert!((w.weed_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn weed_fraction_one_at_max() {
        let mut w = ws();
        w.weed_level = 10.0;
        assert!((w.weed_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_yield ---

    #[test]
    fn effective_yield_passthrough_when_clear() {
        let w = ws();
        assert!((w.effective_yield(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_yield_halved_at_half_coverage() {
        let mut w = ws();
        w.weed_level = 5.0; // fraction=0.5 → 100*(1-0.5)=50
        assert!((w.effective_yield(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_yield_zero_when_overgrown() {
        let mut w = ws();
        w.weed_level = 10.0; // fraction=1.0 → 0
        assert!((w.effective_yield(100.0) - 0.0).abs() < 1e-3);
    }

    #[test]
    fn effective_yield_passthrough_when_disabled() {
        let mut w = ws();
        w.weed_level = 10.0;
        w.enabled = false;
        assert!((w.effective_yield(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_weed_clamped_to_one() {
        let w = Weed::new(0.0, 1.0);
        assert!((w.max_weed - 1.0).abs() < 1e-5);
    }

    #[test]
    fn grow_rate_clamped_to_zero() {
        let w = Weed::new(10.0, -1.0);
        assert_eq!(w.grow_rate, 0.0);
    }

    // --- combined ---

    #[test]
    fn grow_cull_cycle() {
        let mut w = w(); // grow 1/s
        w.tick(5.0); // 5.0
        w.cull(3.0); // 2.0
        w.tick(2.0); // 4.0
        assert!((w.weed_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn cull_all_then_regrow() {
        let mut w = w();
        w.tick(8.0); // 8.0
        w.cull_all(); // cleared
        w.tick(3.0); // 3.0
        assert!((w.weed_level - 3.0).abs() < 1e-4);
        assert!(!w.is_overgrown());
    }
}
