use bevy_ecs::prelude::Component;

/// Stacking discrete-flaw counter. Models persistent qualitative defects that
/// accumulate from external events and persist until explicitly removed —
/// curses, debuffs, injuries, structural cracks.
///
/// Unlike `Yelp` (integer stacks with timer-based decay) and `Worn` (float
/// durability with optional passive wear), Wart stacks are **permanent until
/// explicitly removed** — there is no automatic clearing or decay. This
/// models defects that an entity must actively remedy, not ones that heal on
/// their own.
///
/// `apply()` adds one wart. Fires `just_warted`. Fires `just_maxed` the
/// first time `wart_count` reaches `max_warts`. No-op when at cap or
/// disabled.
///
/// `remove()` removes one wart. No-op when at 0 or disabled.
///
/// `remove_all()` clears all warts instantly. Fires `just_cleansed`. No-op
/// when already at 0 or disabled.
///
/// `tick(dt)` clears one-frame flags only. No time-based logic.
///
/// `is_afflicted()` returns `wart_count > 0 && enabled`.
///
/// `is_maxed()` returns `wart_count >= max_warts && enabled`.
///
/// `wart_fraction()` returns `(wart_count as f32 / max_warts as f32).clamp(0.0, 1.0)`.
///
/// `effective_ability(base)` returns `base * (1.0 - wart_fraction())` when
/// enabled — each wart cuts output; at full affliction, output is 0; returns
/// `base` unchanged when disabled.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wart {
    /// Current flaw count [0, max_warts].
    pub wart_count: u32,
    /// Maximum flaws before fully afflicted. Clamped >= 1.
    pub max_warts: u32,
    pub just_warted: bool,
    pub just_maxed: bool,
    pub just_cleansed: bool,
    pub enabled: bool,
}

impl Wart {
    pub fn new(max_warts: u32) -> Self {
        Self {
            wart_count: 0,
            max_warts: max_warts.max(1),
            just_warted: false,
            just_maxed: false,
            just_cleansed: false,
            enabled: true,
        }
    }

    /// Add one flaw. Fires `just_warted`. Fires `just_maxed` on first
    /// reaching cap. No-op at cap or when disabled.
    pub fn apply(&mut self) {
        if !self.enabled || self.wart_count >= self.max_warts {
            return;
        }
        self.wart_count += 1;
        self.just_warted = true;
        if self.wart_count >= self.max_warts {
            self.just_maxed = true;
        }
    }

    /// Remove one flaw. No-op when at 0 or disabled.
    pub fn remove(&mut self) {
        if !self.enabled || self.wart_count == 0 {
            return;
        }
        self.wart_count -= 1;
    }

    /// Remove all flaws instantly. Fires `just_cleansed`. No-op when at 0 or
    /// disabled.
    pub fn remove_all(&mut self) {
        if !self.enabled || self.wart_count == 0 {
            return;
        }
        self.wart_count = 0;
        self.just_cleansed = true;
    }

    /// Advance one frame: clear one-frame flags only. No time-based changes.
    pub fn tick(&mut self, _dt: f32) {
        self.just_warted = false;
        self.just_maxed = false;
        self.just_cleansed = false;
    }

    /// `true` when at least one flaw is present and component is enabled.
    pub fn is_afflicted(&self) -> bool {
        self.wart_count > 0 && self.enabled
    }

    /// `true` when at maximum flaws and component is enabled.
    pub fn is_maxed(&self) -> bool {
        self.wart_count >= self.max_warts && self.enabled
    }

    /// Flaw count as a fraction of maximum [0.0, 1.0].
    pub fn wart_fraction(&self) -> f32 {
        (self.wart_count as f32 / self.max_warts as f32).clamp(0.0, 1.0)
    }

    /// Scale `base` inversely by flaw count. Returns
    /// `base * (1.0 - wart_fraction())` when enabled; `base` otherwise.
    pub fn effective_ability(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 - self.wart_fraction())
    }
}

impl Default for Wart {
    fn default() -> Self {
        Self::new(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wart {
        Wart::new(5) // max=5
    }

    #[test]
    fn new_starts_clean() {
        let w = w();
        assert_eq!(w.wart_count, 0);
        assert!(!w.just_warted);
        assert!(!w.just_maxed);
        assert!(!w.just_cleansed);
        assert!(!w.is_afflicted());
        assert!(!w.is_maxed());
    }

    // --- apply ---

    #[test]
    fn apply_increments_count() {
        let mut w = w();
        w.apply();
        assert_eq!(w.wart_count, 1);
    }

    #[test]
    fn apply_fires_just_warted() {
        let mut w = w();
        w.apply();
        assert!(w.just_warted);
    }

    #[test]
    fn apply_fires_just_maxed_at_cap() {
        let mut w = w(); // max=5
        for _ in 0..4 {
            w.apply();
        }
        w.just_warted = false;
        w.just_maxed = false;
        w.apply(); // reaches 5
        assert!(w.just_maxed);
        assert!(w.just_warted);
    }

    #[test]
    fn apply_does_not_refire_just_maxed_after_cap() {
        let mut w = w();
        for _ in 0..5 {
            w.apply();
        }
        w.tick(0.016); // clear flags
        w.apply(); // already at cap, no-op
        assert!(!w.just_warted);
        assert!(!w.just_maxed);
    }

    #[test]
    fn apply_no_op_at_cap() {
        let mut w = w();
        for _ in 0..5 {
            w.apply();
        }
        w.apply(); // over cap, no-op
        assert_eq!(w.wart_count, 5);
    }

    #[test]
    fn apply_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.apply();
        assert_eq!(w.wart_count, 0);
        assert!(!w.just_warted);
    }

    // --- remove ---

    #[test]
    fn remove_decrements_count() {
        let mut w = w();
        w.apply();
        w.apply();
        w.remove();
        assert_eq!(w.wart_count, 1);
    }

    #[test]
    fn remove_no_op_at_zero() {
        let mut w = w();
        w.remove(); // already 0
        assert_eq!(w.wart_count, 0);
    }

    #[test]
    fn remove_no_op_when_disabled() {
        let mut w = w();
        w.apply();
        w.enabled = false;
        w.remove();
        assert_eq!(w.wart_count, 1);
    }

    // --- remove_all ---

    #[test]
    fn remove_all_clears_count() {
        let mut w = w();
        for _ in 0..3 {
            w.apply();
        }
        w.remove_all();
        assert_eq!(w.wart_count, 0);
    }

    #[test]
    fn remove_all_fires_just_cleansed() {
        let mut w = w();
        w.apply();
        w.remove_all();
        assert!(w.just_cleansed);
    }

    #[test]
    fn remove_all_no_op_at_zero() {
        let mut w = w();
        w.remove_all();
        assert!(!w.just_cleansed);
    }

    #[test]
    fn remove_all_no_op_when_disabled() {
        let mut w = w();
        w.apply();
        w.enabled = false;
        w.remove_all();
        assert_eq!(w.wart_count, 1);
        assert!(!w.just_cleansed);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_warted() {
        let mut w = w();
        w.apply();
        w.tick(0.016);
        assert!(!w.just_warted);
    }

    #[test]
    fn tick_clears_just_maxed() {
        let mut w = Wart::new(1);
        w.apply();
        w.tick(0.016);
        assert!(!w.just_maxed);
    }

    #[test]
    fn tick_clears_just_cleansed() {
        let mut w = w();
        w.apply();
        w.remove_all();
        w.tick(0.016);
        assert!(!w.just_cleansed);
    }

    #[test]
    fn tick_does_not_change_count() {
        let mut w = w();
        w.apply();
        w.apply();
        w.tick(1000.0); // no time-based decay
        assert_eq!(w.wart_count, 2);
    }

    // --- is_afflicted / is_maxed ---

    #[test]
    fn is_afflicted_true_with_warts() {
        let mut w = w();
        w.apply();
        assert!(w.is_afflicted());
    }

    #[test]
    fn is_afflicted_false_when_clean() {
        let w = w();
        assert!(!w.is_afflicted());
    }

    #[test]
    fn is_afflicted_false_when_disabled() {
        let mut w = w();
        w.apply();
        w.enabled = false;
        assert!(!w.is_afflicted());
    }

    #[test]
    fn is_maxed_true_at_cap() {
        let mut w = w();
        for _ in 0..5 {
            w.apply();
        }
        assert!(w.is_maxed());
    }

    #[test]
    fn is_maxed_false_below_cap() {
        let mut w = w();
        w.apply();
        assert!(!w.is_maxed());
    }

    #[test]
    fn is_maxed_false_when_disabled() {
        let mut w = w();
        for _ in 0..5 {
            w.apply();
        }
        w.enabled = false;
        assert!(!w.is_maxed());
    }

    // --- wart_fraction ---

    #[test]
    fn wart_fraction_zero_when_clean() {
        let w = w();
        assert_eq!(w.wart_fraction(), 0.0);
    }

    #[test]
    fn wart_fraction_at_partial() {
        let mut w = Wart::new(4); // max=4
        w.apply();
        w.apply(); // 2/4 = 0.5
        assert!((w.wart_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn wart_fraction_one_at_cap() {
        let mut w = w();
        for _ in 0..5 {
            w.apply();
        }
        assert!((w.wart_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_ability ---

    #[test]
    fn effective_ability_passthrough_when_clean() {
        let w = w();
        assert!((w.effective_ability(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_ability_reduced_at_partial() {
        let mut w = Wart::new(4);
        w.apply();
        w.apply(); // fraction=0.5 → 100*(1-0.5)=50
        assert!((w.effective_ability(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_ability_zero_at_max() {
        let mut w = w();
        for _ in 0..5 {
            w.apply();
        } // fraction=1.0 → 0
        assert!((w.effective_ability(100.0) - 0.0).abs() < 1e-3);
    }

    #[test]
    fn effective_ability_passthrough_when_disabled() {
        let mut w = w();
        for _ in 0..5 {
            w.apply();
        }
        w.enabled = false;
        assert!((w.effective_ability(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_warts_clamped_to_one() {
        let w = Wart::new(0);
        assert_eq!(w.max_warts, 1);
    }

    // --- persistent stacks ---

    #[test]
    fn warts_persist_across_many_ticks() {
        let mut w = w();
        w.apply();
        w.apply();
        for _ in 0..100 {
            w.tick(1.0);
        }
        assert_eq!(w.wart_count, 2); // no decay
    }

    #[test]
    fn apply_remove_cycle() {
        let mut w = w();
        w.apply();
        w.apply();
        w.remove();
        assert_eq!(w.wart_count, 1);
        w.apply();
        assert_eq!(w.wart_count, 2);
    }
}
