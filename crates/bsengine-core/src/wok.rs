use bevy_ecs::prelude::Component;

/// Event-driven thermal accumulator with threshold-based sear event and
/// passive cool-down. Heat is added explicitly via `stoke()` and dissipates
/// each tick; crossing `sear_threshold` fires `just_seared` once per
/// ascent; `douse()` resets heat instantly.
///
/// Unlike `Overheat` (which tracks exceeded-capacity damage) and `Fuel`
/// (continuous consumption), Wok models a **bidirectional heat profile**:
/// intentional heat-up via external events plus automatic passive cool-down.
/// The sear threshold separates "warm" from "hot enough to cook", allowing
/// game logic to react distinctly to each zone.
///
/// `stoke(amount)` adds heat. If enabled and `amount > 0`: clamps
/// `heat_level` to `max_heat`. Fires `just_seared` the first time
/// `heat_level` crosses `sear_threshold` on the way up. No-op when disabled.
///
/// `douse()` instantly resets `heat_level` to 0 and fires `just_cooled`
/// when previously hot. No-op when already at 0 or disabled.
///
/// `tick(dt)` clears one-frame flags first, then if enabled and
/// `heat_level > 0`: subtracts `cool_rate * dt`, floors at 0. Fires
/// `just_cooled` when reaching 0.
///
/// `is_hot()` returns `heat_level > 0.0 && enabled`.
///
/// `is_searing()` returns `heat_level >= sear_threshold && enabled`.
///
/// `heat_fraction()` returns `(heat_level / max_heat).clamp(0.0, 1.0)`.
///
/// `effective_sear(base)` returns `base * (1.0 + heat_fraction())` when
/// enabled — up to 2× at max heat; returns `base` unchanged when disabled.
///
/// Default: `new(10.0, 1.0, 7.0)` — max 10, cool 1/s, sear threshold 7.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wok {
    /// Current heat level [0, max_heat].
    pub heat_level: f32,
    /// Maximum heat capacity. Clamped >= 1.0.
    pub max_heat: f32,
    /// Passive cool-down rate in heat-units/second. Clamped >= 0.0.
    pub cool_rate: f32,
    /// Heat level at which searing begins. Clamped to [0, max_heat].
    pub sear_threshold: f32,
    pub just_seared: bool,
    pub just_cooled: bool,
    pub enabled: bool,
}

impl Wok {
    pub fn new(max_heat: f32, cool_rate: f32, sear_threshold: f32) -> Self {
        let max_heat = max_heat.max(1.0);
        Self {
            heat_level: 0.0,
            max_heat,
            cool_rate: cool_rate.max(0.0),
            sear_threshold: sear_threshold.clamp(0.0, max_heat),
            just_seared: false,
            just_cooled: false,
            enabled: true,
        }
    }

    /// Add heat. Fires `just_seared` on crossing `sear_threshold` going up.
    /// No-op when disabled or `amount <= 0`.
    pub fn stoke(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let prev = self.heat_level;
        self.heat_level = (self.heat_level + amount).min(self.max_heat);
        if prev < self.sear_threshold && self.heat_level >= self.sear_threshold {
            self.just_seared = true;
        }
    }

    /// Instantly reset heat to 0. Fires `just_cooled`. No-op when already at
    /// 0 or disabled.
    pub fn douse(&mut self) {
        if !self.enabled || self.heat_level == 0.0 {
            return;
        }
        self.heat_level = 0.0;
        self.just_cooled = true;
    }

    /// Advance one frame: clear flags, then cool passively. Fires
    /// `just_cooled` when reaching 0. No-op (beyond flag clear) when disabled
    /// or already at 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_seared = false;
        self.just_cooled = false;

        if !self.enabled || self.heat_level == 0.0 {
            return;
        }

        self.heat_level = (self.heat_level - self.cool_rate * dt).max(0.0);
        if self.heat_level == 0.0 {
            self.just_cooled = true;
        }
    }

    /// `true` when heat is above 0 and component is enabled.
    pub fn is_hot(&self) -> bool {
        self.heat_level > 0.0 && self.enabled
    }

    /// `true` when heat is at or above the sear threshold and component is
    /// enabled.
    pub fn is_searing(&self) -> bool {
        self.heat_level >= self.sear_threshold && self.enabled
    }

    /// Heat level as a fraction of maximum [0.0, 1.0].
    pub fn heat_fraction(&self) -> f32 {
        (self.heat_level / self.max_heat).clamp(0.0, 1.0)
    }

    /// Scale `base` by heat. Returns `base * (1.0 + heat_fraction())` when
    /// enabled — up to 2× at max heat; `base` when disabled.
    pub fn effective_sear(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.heat_fraction())
    }
}

impl Default for Wok {
    fn default() -> Self {
        Self::new(10.0, 1.0, 7.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wok {
        Wok::new(10.0, 1.0, 7.0) // max=10, cool 1/s, sear at 7
    }

    #[test]
    fn new_starts_cold() {
        let w = w();
        assert_eq!(w.heat_level, 0.0);
        assert!(!w.just_seared);
        assert!(!w.just_cooled);
        assert!(!w.is_hot());
        assert!(!w.is_searing());
    }

    // --- stoke ---

    #[test]
    fn stoke_increases_heat() {
        let mut w = w();
        w.stoke(3.0);
        assert!((w.heat_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn stoke_clamps_at_max() {
        let mut w = w();
        w.stoke(20.0);
        assert!((w.heat_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn stoke_fires_just_seared_at_threshold() {
        let mut w = w(); // sear_threshold=7
        w.stoke(7.0); // exactly at threshold
        assert!(w.just_seared);
    }

    #[test]
    fn stoke_fires_just_seared_crossing_threshold() {
        let mut w = w();
        w.stoke(5.0); // below 7
        w.stoke(4.0); // crosses 7
        assert!(w.just_seared);
    }

    #[test]
    fn stoke_does_not_refire_just_seared_above_threshold() {
        let mut w = w();
        w.stoke(8.0); // crosses threshold, just_seared=true
        w.just_seared = false; // clear manually
        w.stoke(1.0); // still above threshold, no re-fire
        assert!(!w.just_seared);
    }

    #[test]
    fn stoke_no_op_for_zero_amount() {
        let mut w = w();
        w.stoke(0.0);
        assert_eq!(w.heat_level, 0.0);
        assert!(!w.just_seared);
    }

    #[test]
    fn stoke_no_op_for_negative_amount() {
        let mut w = w();
        w.stoke(-5.0);
        assert_eq!(w.heat_level, 0.0);
    }

    #[test]
    fn stoke_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.stoke(5.0);
        assert_eq!(w.heat_level, 0.0);
        assert!(!w.just_seared);
    }

    // --- douse ---

    #[test]
    fn douse_resets_heat() {
        let mut w = w();
        w.stoke(8.0);
        w.douse();
        assert_eq!(w.heat_level, 0.0);
    }

    #[test]
    fn douse_fires_just_cooled() {
        let mut w = w();
        w.stoke(5.0);
        w.douse();
        assert!(w.just_cooled);
    }

    #[test]
    fn douse_no_op_when_already_cold() {
        let mut w = w();
        w.douse(); // already 0
        assert!(!w.just_cooled);
    }

    #[test]
    fn douse_no_op_when_disabled() {
        let mut w = w();
        w.stoke(5.0);
        w.enabled = false;
        w.douse();
        assert!((w.heat_level - 5.0).abs() < 1e-5);
        assert!(!w.just_cooled);
    }

    // --- tick ---

    #[test]
    fn tick_cools_passively() {
        let mut w = w(); // cool_rate=1/s
        w.stoke(5.0); // 5.0
        w.tick(2.0); // 5 - 2 = 3
        assert!((w.heat_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_cooled_at_zero() {
        let mut w = w();
        w.stoke(2.0);
        w.tick(2.0); // exactly reaches 0
        assert!(w.just_cooled);
        assert_eq!(w.heat_level, 0.0);
    }

    #[test]
    fn tick_fires_just_cooled_crossing_zero() {
        let mut w = w();
        w.stoke(1.0);
        w.tick(5.0); // crosses 0
        assert!(w.just_cooled);
        assert_eq!(w.heat_level, 0.0);
    }

    #[test]
    fn tick_just_cooled_clears_next_frame() {
        let mut w = w();
        w.stoke(1.0);
        w.tick(2.0); // just_cooled=true
        w.tick(0.016);
        assert!(!w.just_cooled);
    }

    #[test]
    fn tick_clears_just_seared() {
        let mut w = w();
        w.stoke(8.0); // just_seared=true
        w.tick(0.016);
        assert!(!w.just_seared);
    }

    #[test]
    fn tick_no_op_when_already_cold() {
        let mut w = w();
        w.tick(10.0);
        assert_eq!(w.heat_level, 0.0);
        assert!(!w.just_cooled);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.stoke(5.0);
        w.enabled = false;
        w.tick(10.0);
        assert!((w.heat_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_seared = true;
        w.just_cooled = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_seared);
        assert!(!w.just_cooled);
    }

    // --- is_hot / is_searing ---

    #[test]
    fn is_hot_false_when_cold() {
        let w = w();
        assert!(!w.is_hot());
    }

    #[test]
    fn is_hot_true_when_warm() {
        let mut w = w();
        w.stoke(1.0);
        assert!(w.is_hot());
    }

    #[test]
    fn is_hot_false_when_disabled() {
        let mut w = w();
        w.stoke(3.0);
        w.enabled = false;
        assert!(!w.is_hot());
    }

    #[test]
    fn is_searing_false_below_threshold() {
        let mut w = w(); // threshold=7
        w.stoke(5.0);
        assert!(!w.is_searing());
    }

    #[test]
    fn is_searing_true_at_threshold() {
        let mut w = w();
        w.stoke(7.0);
        assert!(w.is_searing());
    }

    #[test]
    fn is_searing_false_when_disabled() {
        let mut w = w();
        w.stoke(9.0);
        w.enabled = false;
        assert!(!w.is_searing());
    }

    // --- heat_fraction ---

    #[test]
    fn heat_fraction_zero_when_cold() {
        let w = w();
        assert_eq!(w.heat_fraction(), 0.0);
    }

    #[test]
    fn heat_fraction_at_half() {
        let mut w = w(); // max=10
        w.stoke(5.0); // 5/10=0.5
        assert!((w.heat_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn heat_fraction_one_at_max() {
        let mut w = w();
        w.stoke(10.0);
        assert!((w.heat_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_sear ---

    #[test]
    fn effective_sear_passthrough_when_cold() {
        let w = w();
        assert!((w.effective_sear(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_sear_scaled_at_half() {
        let mut w = w();
        w.stoke(5.0); // fraction=0.5 → 100*(1+0.5)=150
        assert!((w.effective_sear(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_sear_doubled_at_max() {
        let mut w = w();
        w.stoke(10.0); // fraction=1.0 → 100*(1+1)=200
        assert!((w.effective_sear(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_sear_passthrough_when_disabled() {
        let mut w = w();
        w.stoke(10.0);
        w.enabled = false;
        assert!((w.effective_sear(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_heat_clamped_to_one() {
        let w = Wok::new(0.0, 1.0, 5.0);
        assert!((w.max_heat - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cool_rate_clamped_to_zero() {
        let w = Wok::new(10.0, -1.0, 5.0);
        assert_eq!(w.cool_rate, 0.0);
    }

    #[test]
    fn sear_threshold_clamped_to_max() {
        let w = Wok::new(10.0, 1.0, 20.0); // threshold > max → clamped to 10
        assert!((w.sear_threshold - 10.0).abs() < 1e-5);
    }

    #[test]
    fn sear_threshold_clamped_to_zero() {
        let w = Wok::new(10.0, 1.0, -1.0);
        assert_eq!(w.sear_threshold, 0.0);
    }

    #[test]
    fn zero_cool_rate_stays_hot() {
        let mut w = Wok::new(10.0, 0.0, 7.0);
        w.stoke(8.0);
        w.tick(100.0);
        assert!((w.heat_level - 8.0).abs() < 1e-4);
    }

    // --- stoke-cool cycle ---

    #[test]
    fn stoke_cool_resear_cycle() {
        let mut w = w(); // sear=7
        w.stoke(8.0); // just_seared
        w.tick(2.0); // 6.0 — below sear
                     // heat drops below sear threshold after tick
        assert!(!w.is_searing());
        w.stoke(2.0); // 8.0 — crosses 7 again
        assert!(w.just_seared); // fires again on re-entry
    }
}
