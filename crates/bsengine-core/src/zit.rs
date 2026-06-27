use bevy_ecs::prelude::Component;

/// Integer-stack wound/blemish accumulator with inflame threshold. Models
/// stacking minor injuries (think bleed-buildup, poison-stacks, or wound
/// charges) that individually do nothing but collectively trigger a threshold
/// state. Distinct from `Bleed` (continuous rate) and `Counter` (generic
/// integer, no threshold events).
///
/// `apply()` adds one stack if enabled. Fires `just_inflamed` the first time
/// `zit_count` reaches `max_zits`. No-op when disabled.
///
/// `clear_one()` removes one stack if enabled and count > 0. Fires
/// `just_cleared` when count drops to 0. No-op when count is already 0 or
/// when disabled.
///
/// `clear_all()` resets count to 0 immediately regardless of enabled state.
/// Fires `just_cleared` if count was > 0.
///
/// `tick(_dt)` clears one-frame flags only. No time-based changes.
///
/// `is_inflamed()` returns `zit_count >= max_zits && enabled`.
///
/// `is_blemished()` returns `zit_count > 0 && enabled`.
///
/// `stack_fraction()` returns `(zit_count as f32 / max_zits as f32).clamp(0.0, 1.0)`.
///
/// `effective_irritation(base)` returns `base * stack_fraction()` when
/// enabled — 0 at zero stacks, `base` at full inflame; `0.0` when disabled.
///
/// Default: `new(5)` — 5 stacks to inflame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zit {
    /// Current wound stack count [0, ∞). Inflamed when >= max_zits.
    pub zit_count: u32,
    /// Stack count that triggers the inflamed state. Clamped >= 1.
    pub max_zits: u32,
    pub just_inflamed: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Zit {
    pub fn new(max_zits: u32) -> Self {
        Self {
            zit_count: 0,
            max_zits: max_zits.max(1),
            just_inflamed: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Add one wound stack. Fires `just_inflamed` on reaching `max_zits`.
    /// No-op when disabled.
    pub fn apply(&mut self) {
        if !self.enabled {
            return;
        }
        let was_below = self.zit_count < self.max_zits;
        self.zit_count += 1;
        if was_below && self.zit_count >= self.max_zits {
            self.just_inflamed = true;
        }
    }

    /// Remove one wound stack. Fires `just_cleared` when count reaches 0.
    /// No-op when count is 0 or when disabled.
    pub fn clear_one(&mut self) {
        if !self.enabled || self.zit_count == 0 {
            return;
        }
        self.zit_count -= 1;
        if self.zit_count == 0 {
            self.just_cleared = true;
        }
    }

    /// Reset all stacks to 0 immediately. Fires `just_cleared` if there were
    /// any stacks. Ignores the enabled flag (clearing wounds is always safe).
    pub fn clear_all(&mut self) {
        if self.zit_count > 0 {
            self.zit_count = 0;
            self.just_cleared = true;
        }
    }

    /// Advance one frame: clear one-frame flags only. No time-based changes.
    pub fn tick(&mut self, _dt: f32) {
        self.just_inflamed = false;
        self.just_cleared = false;
    }

    /// `true` when stack count has reached or exceeded the inflame threshold
    /// and component is enabled.
    pub fn is_inflamed(&self) -> bool {
        self.zit_count >= self.max_zits && self.enabled
    }

    /// `true` when any stacks are present and component is enabled.
    pub fn is_blemished(&self) -> bool {
        self.zit_count > 0 && self.enabled
    }

    /// Current stacks as a fraction of the inflame threshold [0.0, 1.0].
    pub fn stack_fraction(&self) -> f32 {
        (self.zit_count as f32 / self.max_zits as f32).clamp(0.0, 1.0)
    }

    /// Scale `base` by current stack pressure. Returns `base *
    /// stack_fraction()` when enabled — 0 at empty, `base` at full inflame;
    /// `0.0` when disabled.
    pub fn effective_irritation(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.stack_fraction()
    }
}

impl Default for Zit {
    fn default() -> Self {
        Self::new(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zit {
        Zit::new(5) // 5 stacks to inflame
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let z = z();
        assert_eq!(z.zit_count, 0);
        assert_eq!(z.max_zits, 5);
        assert!(!z.just_inflamed);
        assert!(!z.just_cleared);
        assert!(!z.is_inflamed());
        assert!(!z.is_blemished());
    }

    #[test]
    fn max_zits_clamped_to_one() {
        let z = Zit::new(0);
        assert_eq!(z.max_zits, 1);
    }

    // --- apply ---

    #[test]
    fn apply_increments_count() {
        let mut z = z();
        z.apply();
        assert_eq!(z.zit_count, 1);
    }

    #[test]
    fn apply_sets_blemished() {
        let mut z = z();
        z.apply();
        assert!(z.is_blemished());
    }

    #[test]
    fn apply_fires_just_inflamed_at_threshold() {
        let mut z = Zit::new(3);
        z.apply();
        z.apply();
        assert!(!z.just_inflamed);
        z.apply(); // reaches 3 = max
        assert!(z.just_inflamed);
    }

    #[test]
    fn apply_does_not_refire_just_inflamed_above_threshold() {
        let mut z = Zit::new(3);
        for _ in 0..3 {
            z.apply();
        }
        z.tick(0.016);
        z.apply(); // 4, already inflamed
        assert!(!z.just_inflamed);
    }

    #[test]
    fn apply_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.apply();
        assert_eq!(z.zit_count, 0);
        assert!(!z.just_inflamed);
    }

    #[test]
    fn apply_allows_count_above_max() {
        let mut z = Zit::new(2);
        z.apply();
        z.apply();
        z.apply(); // 3, over max
        assert_eq!(z.zit_count, 3);
        assert!(z.is_inflamed());
    }

    // --- clear_one ---

    #[test]
    fn clear_one_decrements_count() {
        let mut z = z();
        z.apply();
        z.apply();
        z.clear_one();
        assert_eq!(z.zit_count, 1);
    }

    #[test]
    fn clear_one_fires_just_cleared_at_zero() {
        let mut z = z();
        z.apply();
        z.clear_one();
        assert_eq!(z.zit_count, 0);
        assert!(z.just_cleared);
    }

    #[test]
    fn clear_one_no_op_when_already_zero() {
        let mut z = z();
        z.clear_one(); // already 0
        assert!(!z.just_cleared);
        assert_eq!(z.zit_count, 0);
    }

    #[test]
    fn clear_one_no_op_when_disabled() {
        let mut z = z();
        z.apply();
        z.enabled = false;
        z.clear_one();
        assert_eq!(z.zit_count, 1);
        assert!(!z.just_cleared);
    }

    #[test]
    fn clear_one_no_just_cleared_above_zero() {
        let mut z = z();
        z.apply();
        z.apply();
        z.clear_one(); // 1 remaining
        assert!(!z.just_cleared);
    }

    // --- clear_all ---

    #[test]
    fn clear_all_resets_to_zero() {
        let mut z = z();
        for _ in 0..3 {
            z.apply();
        }
        z.clear_all();
        assert_eq!(z.zit_count, 0);
        assert!(z.just_cleared);
    }

    #[test]
    fn clear_all_no_op_when_already_empty() {
        let mut z = z();
        z.clear_all();
        assert!(!z.just_cleared);
    }

    #[test]
    fn clear_all_works_when_disabled() {
        let mut z = z();
        for _ in 0..3 {
            z.apply();
        }
        z.enabled = false;
        z.clear_all(); // should still work
        assert_eq!(z.zit_count, 0);
        assert!(z.just_cleared);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_inflamed() {
        let mut z = Zit::new(2);
        z.apply();
        z.apply(); // inflamed
        z.tick(0.016);
        assert!(!z.just_inflamed);
    }

    #[test]
    fn tick_clears_just_cleared() {
        let mut z = z();
        z.apply();
        z.clear_all();
        z.tick(0.016);
        assert!(!z.just_cleared);
    }

    #[test]
    fn tick_does_not_change_count() {
        let mut z = z();
        z.apply();
        z.apply();
        z.tick(100.0);
        assert_eq!(z.zit_count, 2);
    }

    // --- is_inflamed / is_blemished ---

    #[test]
    fn is_inflamed_false_below_threshold() {
        let mut z = Zit::new(3);
        z.apply();
        z.apply(); // 2 < 3
        assert!(!z.is_inflamed());
    }

    #[test]
    fn is_inflamed_true_at_threshold() {
        let mut z = Zit::new(3);
        for _ in 0..3 {
            z.apply();
        }
        assert!(z.is_inflamed());
    }

    #[test]
    fn is_inflamed_false_when_disabled() {
        let mut z = Zit::new(2);
        z.apply();
        z.apply();
        z.enabled = false;
        assert!(!z.is_inflamed());
    }

    #[test]
    fn is_blemished_false_when_empty() {
        let z = z();
        assert!(!z.is_blemished());
    }

    #[test]
    fn is_blemished_true_with_one_stack() {
        let mut z = z();
        z.apply();
        assert!(z.is_blemished());
    }

    #[test]
    fn is_blemished_false_when_disabled() {
        let mut z = z();
        z.apply();
        z.enabled = false;
        assert!(!z.is_blemished());
    }

    // --- stack_fraction ---

    #[test]
    fn stack_fraction_zero_when_empty() {
        let z = z();
        assert_eq!(z.stack_fraction(), 0.0);
    }

    #[test]
    fn stack_fraction_half_at_mid() {
        let mut z = Zit::new(4);
        z.apply();
        z.apply(); // 2/4=0.5
        assert!((z.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_one_at_threshold() {
        let mut z = Zit::new(3);
        for _ in 0..3 {
            z.apply();
        }
        assert!((z.stack_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_clamped_above_max() {
        let mut z = Zit::new(2);
        for _ in 0..5 {
            z.apply();
        }
        assert!((z.stack_fraction() - 1.0).abs() < 1e-5); // clamped
    }

    // --- effective_irritation ---

    #[test]
    fn effective_irritation_zero_when_empty() {
        let z = z();
        assert_eq!(z.effective_irritation(100.0), 0.0);
    }

    #[test]
    fn effective_irritation_full_at_threshold() {
        let mut z = Zit::new(4);
        for _ in 0..4 {
            z.apply();
        }
        assert!((z.effective_irritation(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_irritation_half_at_midpoint() {
        let mut z = Zit::new(4);
        z.apply();
        z.apply(); // 2/4=0.5
        assert!((z.effective_irritation(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_irritation_zero_when_disabled() {
        let mut z = z();
        for _ in 0..5 {
            z.apply();
        }
        z.enabled = false;
        assert_eq!(z.effective_irritation(100.0), 0.0);
    }

    // --- stack cycle ---

    #[test]
    fn apply_clear_one_cycle() {
        let mut z = Zit::new(3);
        z.apply();
        z.apply();
        z.apply(); // inflamed
        assert!(z.is_inflamed());
        z.clear_one(); // 2
        assert!(!z.is_inflamed());
        z.clear_one(); // 1
        z.clear_one(); // 0 → just_cleared
        assert!(z.just_cleared);
        assert!(!z.is_blemished());
    }
}
