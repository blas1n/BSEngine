use bevy_ecs::prelude::Component;

/// Dual-input durability degradation tracker. Models material or stamina
/// wear that accumulates from both explicit `degrade()` events AND passive
/// tick-based wear — whichever inputs are active.
///
/// Unlike `Wist` (event-only, no tick logic), `Worn` supports optional
/// passive wear via `wear_rate`. Set `wear_rate` to 0.0 for pure
/// event-driven degradation. Unlike `Worm` (passive build, no event input),
/// `Worn` accepts arbitrary event-driven degradation amounts alongside the
/// tick rate.
///
/// `degrade(amount)` adds `amount` to `worn_level` (clamped to `max_worn`),
/// fires `just_wore`. Fires `just_broke` the first time `worn_level` reaches
/// `max_worn`. No-op when disabled or `amount <= 0.0`.
///
/// `repair(amount)` reduces `worn_level` by `amount` (floored to 0.0).
/// No-op when disabled or already at 0.
///
/// `tick(dt)` clears one-frame flags first, then if enabled and
/// `wear_rate > 0.0`: adds `wear_rate * dt` to `worn_level` (clamped,
/// fires `just_broke` at crossing). No passive wear when `wear_rate == 0.0`.
///
/// `is_broken()` returns `worn_level >= max_worn && enabled`.
///
/// `wear_fraction()` returns `(worn_level / max_worn).clamp(0.0, 1.0)`.
///
/// `effective_strength(base)` returns `base * (1.0 - wear_fraction())` when
/// enabled — degrades toward 0 as wear increases; `base` unchanged when
/// disabled.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Worn {
    /// Current wear level [0, max_worn].
    pub worn_level: f32,
    /// Maximum wear before broken. Clamped >= 1.0.
    pub max_worn: f32,
    /// Passive wear added per second by tick(). Clamped >= 0.0.
    pub wear_rate: f32,
    pub just_wore: bool,
    pub just_broke: bool,
    pub enabled: bool,
}

impl Worn {
    pub fn new(max_worn: f32, wear_rate: f32) -> Self {
        Self {
            worn_level: 0.0,
            max_worn: max_worn.max(1.0),
            wear_rate: wear_rate.max(0.0),
            just_wore: false,
            just_broke: false,
            enabled: true,
        }
    }

    /// Add `amount` of wear. Fires `just_wore`. Fires `just_broke` on first
    /// reaching max. No-op when disabled or `amount <= 0.0`.
    pub fn degrade(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let prev = self.worn_level;
        self.worn_level = (self.worn_level + amount).min(self.max_worn);
        self.just_wore = true;
        if prev < self.max_worn && self.worn_level >= self.max_worn {
            self.just_broke = true;
        }
    }

    /// Reduce wear by `amount` (floored to 0.0). No-op when disabled or
    /// already at 0.
    pub fn repair(&mut self, amount: f32) {
        if !self.enabled || self.worn_level == 0.0 {
            return;
        }
        self.worn_level = (self.worn_level - amount).max(0.0);
    }

    /// Advance one frame: clear flags, then apply passive wear_rate * dt.
    /// Fires `just_broke` if passive wear crosses max_worn this tick.
    /// No passive wear when `wear_rate == 0.0`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wore = false;
        self.just_broke = false;

        if !self.enabled || self.wear_rate == 0.0 {
            return;
        }

        let prev = self.worn_level;
        self.worn_level = (self.worn_level + self.wear_rate * dt).min(self.max_worn);

        if prev < self.max_worn && self.worn_level >= self.max_worn {
            self.just_broke = true;
        }
    }

    /// `true` when worn to maximum and component is enabled.
    pub fn is_broken(&self) -> bool {
        self.worn_level >= self.max_worn && self.enabled
    }

    /// Wear as a fraction of maximum [0.0, 1.0].
    pub fn wear_fraction(&self) -> f32 {
        (self.worn_level / self.max_worn).clamp(0.0, 1.0)
    }

    /// Scale `base` inversely by wear. Returns `base * (1.0 - wear_fraction())`
    /// when enabled; `base` unchanged when disabled.
    pub fn effective_strength(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 - self.wear_fraction())
    }
}

impl Default for Worn {
    fn default() -> Self {
        Self::new(10.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Worn {
        Worn::new(10.0, 0.0) // max=10, no passive wear
    }

    fn wp() -> Worn {
        Worn::new(10.0, 1.0) // max=10, passive wear 1/s
    }

    #[test]
    fn new_starts_fresh() {
        let w = w();
        assert_eq!(w.worn_level, 0.0);
        assert!(!w.just_wore);
        assert!(!w.just_broke);
        assert!(!w.is_broken());
    }

    // --- degrade ---

    #[test]
    fn degrade_increases_level() {
        let mut w = w();
        w.degrade(4.0);
        assert!((w.worn_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn degrade_fires_just_wore() {
        let mut w = w();
        w.degrade(1.0);
        assert!(w.just_wore);
    }

    #[test]
    fn degrade_clamps_to_max() {
        let mut w = w();
        w.degrade(20.0);
        assert!((w.worn_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn degrade_fires_just_broke_at_max() {
        let mut w = w();
        w.degrade(10.0);
        assert!(w.just_broke);
    }

    #[test]
    fn degrade_fires_just_broke_crossing_max() {
        let mut w = w();
        w.degrade(7.0);
        w.tick(0.016); // clear flags
        w.degrade(5.0); // crosses 10
        assert!(w.just_broke);
    }

    #[test]
    fn degrade_does_not_refire_just_broke_at_cap() {
        let mut w = w();
        w.degrade(10.0); // breaks
        w.tick(0.016);
        w.degrade(1.0); // already at max
        assert!(!w.just_broke);
    }

    #[test]
    fn degrade_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.degrade(5.0);
        assert_eq!(w.worn_level, 0.0);
        assert!(!w.just_wore);
    }

    #[test]
    fn degrade_no_op_when_amount_zero() {
        let mut w = w();
        w.degrade(0.0);
        assert_eq!(w.worn_level, 0.0);
        assert!(!w.just_wore);
    }

    #[test]
    fn degrade_no_op_when_amount_negative() {
        let mut w = w();
        w.degrade(-3.0);
        assert_eq!(w.worn_level, 0.0);
        assert!(!w.just_wore);
    }

    // --- repair ---

    #[test]
    fn repair_reduces_level() {
        let mut w = w();
        w.degrade(7.0);
        w.repair(3.0);
        assert!((w.worn_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn repair_floors_at_zero() {
        let mut w = w();
        w.degrade(3.0);
        w.repair(10.0);
        assert_eq!(w.worn_level, 0.0);
    }

    #[test]
    fn repair_no_op_when_disabled() {
        let mut w = w();
        w.degrade(5.0);
        w.enabled = false;
        w.repair(3.0);
        assert!((w.worn_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn repair_no_op_when_already_zero() {
        let mut w = w();
        w.repair(5.0); // level=0, no-op
        assert_eq!(w.worn_level, 0.0);
    }

    // --- tick (passive wear) ---

    #[test]
    fn tick_clears_just_wore() {
        let mut w = w();
        w.degrade(1.0);
        w.tick(0.016);
        assert!(!w.just_wore);
    }

    #[test]
    fn tick_clears_just_broke() {
        let mut w = w();
        w.degrade(10.0);
        w.tick(0.016);
        assert!(!w.just_broke);
    }

    #[test]
    fn tick_passive_wear_increases_level() {
        let mut w = wp(); // wear_rate=1.0
        w.tick(3.0); // 3.0
        assert!((w.worn_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_passive_wear_when_rate_zero() {
        let mut w = w(); // wear_rate=0.0
        w.tick(100.0);
        assert_eq!(w.worn_level, 0.0);
    }

    #[test]
    fn tick_fires_just_broke_on_passive_crossing() {
        let mut w = wp(); // rate=1/s, max=10
        w.tick(10.0); // crosses max
        assert!(w.just_broke);
    }

    #[test]
    fn tick_no_passive_wear_when_disabled() {
        let mut w = wp();
        w.enabled = false;
        w.tick(10.0);
        assert_eq!(w.worn_level, 0.0);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_wore = true;
        w.just_broke = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_wore);
        assert!(!w.just_broke);
    }

    // --- is_broken / wear_fraction ---

    #[test]
    fn is_broken_true_at_max() {
        let mut w = w();
        w.degrade(10.0);
        assert!(w.is_broken());
    }

    #[test]
    fn is_broken_false_below_max() {
        let mut w = w();
        w.degrade(9.9);
        assert!(!w.is_broken());
    }

    #[test]
    fn is_broken_false_when_disabled() {
        let mut w = w();
        w.degrade(10.0);
        w.enabled = false;
        assert!(!w.is_broken());
    }

    #[test]
    fn wear_fraction_zero_when_fresh() {
        let w = w();
        assert_eq!(w.wear_fraction(), 0.0);
    }

    #[test]
    fn wear_fraction_half_at_midpoint() {
        let mut w = w();
        w.degrade(5.0);
        assert!((w.wear_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn wear_fraction_one_at_max() {
        let mut w = w();
        w.degrade(10.0);
        assert!((w.wear_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_strength ---

    #[test]
    fn effective_strength_passthrough_when_fresh() {
        let w = w();
        assert!((w.effective_strength(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_strength_halved_at_half_wear() {
        let mut w = w();
        w.degrade(5.0); // fraction=0.5 → 100*(1-0.5)=50
        assert!((w.effective_strength(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_strength_zero_when_broken() {
        let mut w = w();
        w.degrade(10.0); // fraction=1.0 → 100*(1-1.0)=0
        assert!((w.effective_strength(100.0) - 0.0).abs() < 1e-3);
    }

    #[test]
    fn effective_strength_passthrough_when_disabled() {
        let mut w = w();
        w.degrade(10.0);
        w.enabled = false;
        assert!((w.effective_strength(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_worn_clamped_to_one() {
        let w = Worn::new(0.0, 0.0);
        assert!((w.max_worn - 1.0).abs() < 1e-5);
    }

    #[test]
    fn wear_rate_clamped_to_zero() {
        let w = Worn::new(10.0, -1.0);
        assert_eq!(w.wear_rate, 0.0);
    }

    // --- combined degrade + repair + tick ---

    #[test]
    fn degrade_repair_cycle() {
        let mut w = w();
        w.degrade(8.0);
        w.repair(5.0); // 3.0
        w.degrade(2.0); // 5.0
        assert!((w.worn_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn passive_and_event_wear_combined() {
        let mut w = Worn::new(10.0, 1.0); // passive 1/s
        w.tick(3.0); // 3.0
        w.degrade(2.0); // 5.0
        assert!((w.worn_level - 5.0).abs() < 1e-4);
    }
}
