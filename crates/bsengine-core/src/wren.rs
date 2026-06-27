use bevy_ecs::prelude::Component;

/// Passive resource cache with an all-or-nothing harvest gate. Models a
/// small, industrious creature that stockpiles resources over time; the cache
/// can only be spent when completely full.
///
/// `tick(dt)` clears `just_stocked` and `just_yielded` first, then increases
/// `wren_level` by `cache_rate * dt` (capped at `max_wren`); fires
/// `just_stocked` the first time it reaches the cap. No-op (beyond flag
/// clear) when disabled or already at max.
///
/// `harvest()` spends the full cache: fires `just_yielded` and resets
/// `wren_level` to 0. Requires `wren_level >= max_wren` AND enabled; no-op
/// otherwise. This is strictly gated — partial charge yields nothing.
///
/// `is_stocked()` returns `wren_level >= max_wren && enabled`.
///
/// `cache_fraction()` returns `(wren_level / max_wren).clamp(0.0, 1.0)`.
///
/// `effective_yield(base)` returns `base * (1.0 + yield_bonus)` when enabled
/// and fully stocked; returns `base` unchanged otherwise. The bonus is binary
/// — full or nothing — unlike fraction-scaled components.
///
/// Distinct from `Weasel` (partial charge can be spent), `Mana` (manual fill
/// and cost system), and `Fuel` (consumed per-use, not gated by fullness):
/// Wren models a **patient passive stockpile** — the cache fills on its own
/// and the full store unlocks a quality bonus. Spend it only when ready.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wren {
    /// Current cache level [0.0, max_wren].
    pub wren_level: f32,
    /// Cache capacity. Clamped >= 1.0.
    pub max_wren: f32,
    /// Fill rate per second. Clamped >= 0.0.
    pub cache_rate: f32,
    /// Output bonus when harvesting at full stock. Clamped >= 0.0.
    pub yield_bonus: f32,
    pub just_stocked: bool,
    pub just_yielded: bool,
    pub enabled: bool,
}

impl Wren {
    pub fn new(max_wren: f32, cache_rate: f32, yield_bonus: f32) -> Self {
        Self {
            wren_level: 0.0,
            max_wren: max_wren.max(1.0),
            cache_rate: cache_rate.max(0.0),
            yield_bonus: yield_bonus.max(0.0),
            just_stocked: false,
            just_yielded: false,
            enabled: true,
        }
    }

    /// Spend the full cache. Fires `just_yielded` and resets `wren_level`
    /// to 0 only when fully stocked and enabled. No-op otherwise.
    pub fn harvest(&mut self) {
        if !self.enabled || self.wren_level < self.max_wren {
            return;
        }
        self.just_yielded = true;
        self.wren_level = 0.0;
    }

    /// Advance one frame: clear flags, then fill cache if not at max.
    /// No-op (beyond flag clear) when disabled or already full.
    pub fn tick(&mut self, dt: f32) {
        self.just_stocked = false;
        self.just_yielded = false;

        if !self.enabled {
            return;
        }

        if self.wren_level < self.max_wren {
            self.wren_level = (self.wren_level + self.cache_rate * dt).min(self.max_wren);
            if self.wren_level >= self.max_wren {
                self.just_stocked = true;
            }
        }
    }

    /// `true` when cache is full and component is enabled.
    pub fn is_stocked(&self) -> bool {
        self.wren_level >= self.max_wren && self.enabled
    }

    /// Cache level as a fraction [0.0, 1.0].
    pub fn cache_fraction(&self) -> f32 {
        (self.wren_level / self.max_wren).clamp(0.0, 1.0)
    }

    /// Scale `base` by the full-stock bonus. Returns
    /// `base * (1.0 + yield_bonus)` when enabled and fully stocked;
    /// `base` otherwise. Bonus is binary — full cache only.
    pub fn effective_yield(&self, base: f32) -> f32 {
        if !self.enabled || self.wren_level < self.max_wren {
            return base;
        }
        base * (1.0 + self.yield_bonus)
    }
}

impl Default for Wren {
    fn default() -> Self {
        Self::new(10.0, 2.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wren {
        Wren::new(10.0, 2.0, 0.5)
    }

    #[test]
    fn new_starts_empty() {
        let w = w();
        assert_eq!(w.wren_level, 0.0);
        assert!(!w.just_stocked);
        assert!(!w.just_yielded);
        assert!(!w.is_stocked());
    }

    #[test]
    fn tick_fills_cache() {
        let mut w = w(); // cache_rate=2.0
        w.tick(1.0); // 2.0
        assert!((w.wren_level - 2.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max() {
        let mut w = w();
        w.tick(100.0); // capped at 10
        assert!((w.wren_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_stocked_when_reaching_max() {
        let mut w = w(); // cache_rate=2.0 → needs 5s
        w.tick(5.0);
        assert!(w.just_stocked);
    }

    #[test]
    fn tick_clears_just_stocked_next_frame() {
        let mut w = w();
        w.tick(5.0); // stocked
        w.tick(0.016); // clears
        assert!(!w.just_stocked);
    }

    #[test]
    fn tick_just_stocked_fires_only_once() {
        let mut w = w();
        w.tick(5.0); // stocked
        w.tick(0.016); // cleared
        w.tick(1.0); // still at max, no re-fire
        assert!(!w.just_stocked);
    }

    #[test]
    fn tick_no_fill_when_already_at_max() {
        let mut w = w();
        w.tick(5.0); // max
        w.tick(0.016); // clears flags
        w.tick(1.0); // no change
        assert!(!w.just_stocked);
        assert!((w.wren_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(5.0);
        assert_eq!(w.wren_level, 0.0);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_stocked = true;
        w.just_yielded = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_stocked);
        assert!(!w.just_yielded);
    }

    #[test]
    fn harvest_no_op_when_not_stocked() {
        let mut w = w();
        w.tick(2.0); // 4.0 — partial
        w.harvest();
        assert!(!w.just_yielded);
        assert!((w.wren_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn harvest_fires_just_yielded_when_stocked() {
        let mut w = w();
        w.tick(5.0); // max
        w.harvest();
        assert!(w.just_yielded);
    }

    #[test]
    fn harvest_resets_level_to_zero() {
        let mut w = w();
        w.tick(5.0); // max
        w.harvest();
        assert_eq!(w.wren_level, 0.0);
    }

    #[test]
    fn harvest_no_op_when_disabled() {
        let mut w = w();
        w.tick(5.0); // max
        w.enabled = false;
        w.harvest();
        assert!(!w.just_yielded);
        assert!((w.wren_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn is_stocked_true_at_max() {
        let mut w = w();
        w.tick(5.0);
        assert!(w.is_stocked());
    }

    #[test]
    fn is_stocked_false_below_max() {
        let mut w = w();
        w.tick(2.0); // 4.0
        assert!(!w.is_stocked());
    }

    #[test]
    fn is_stocked_false_when_disabled() {
        let mut w = w();
        w.tick(5.0);
        w.enabled = false;
        assert!(!w.is_stocked());
    }

    #[test]
    fn is_stocked_false_after_harvest() {
        let mut w = w();
        w.tick(5.0);
        w.harvest();
        assert!(!w.is_stocked());
    }

    #[test]
    fn cache_fraction_zero_when_empty() {
        let w = w();
        assert_eq!(w.cache_fraction(), 0.0);
    }

    #[test]
    fn cache_fraction_half_at_midpoint() {
        let mut w = w();
        w.wren_level = 5.0;
        assert!((w.cache_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn cache_fraction_one_at_max() {
        let mut w = w();
        w.tick(5.0);
        assert!((w.cache_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_yield_base_when_empty() {
        let w = w();
        assert!((w.effective_yield(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_yield_base_when_partial() {
        let mut w = w();
        w.tick(2.0); // 4.0 — partial, no bonus
        assert!((w.effective_yield(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_yield_boosted_when_stocked() {
        let mut w = Wren::new(10.0, 2.0, 0.5);
        w.tick(5.0); // max
                     // 100 * (1 + 0.5) = 150
        assert!((w.effective_yield(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_yield_passthrough_when_disabled() {
        let mut w = w();
        w.tick(5.0);
        w.enabled = false;
        assert!((w.effective_yield(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_yield_zero_after_harvest() {
        let mut w = w();
        w.tick(5.0);
        w.harvest();
        assert!((w.effective_yield(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_wren_clamped_to_one() {
        let w = Wren::new(0.0, 2.0, 0.5);
        assert!((w.max_wren - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cache_rate_clamped_to_zero() {
        let w = Wren::new(10.0, -2.0, 0.5);
        assert_eq!(w.cache_rate, 0.0);
    }

    #[test]
    fn yield_bonus_clamped_to_zero() {
        let w = Wren::new(10.0, 2.0, -1.0);
        assert_eq!(w.yield_bonus, 0.0);
    }

    #[test]
    fn harvest_then_refill_then_harvest_cycle() {
        let mut w = w(); // cache_rate=2.0
        w.tick(5.0); // max
        w.harvest(); // spent
        w.tick(0.016); // clear flags
        w.tick(5.0); // refill to max
        assert!(w.is_stocked());
        w.harvest(); // second harvest
        assert!(w.just_yielded);
        assert_eq!(w.wren_level, 0.0);
    }

    #[test]
    fn effective_yield_is_binary_not_scaled() {
        let mut w = Wren::new(10.0, 2.0, 0.5);
        w.wren_level = 9.99; // almost full but not quite
                             // Should NOT apply bonus
        assert!((w.effective_yield(100.0) - 100.0).abs() < 1e-4);
        w.wren_level = 10.0; // exactly full
                             // Should apply bonus now
        assert!((w.effective_yield(100.0) - 150.0).abs() < 1e-3);
    }
}
