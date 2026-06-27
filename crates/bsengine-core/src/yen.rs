use bevy_ecs::prelude::Component;

/// Passive craving accumulator. Models an autonomous desire that builds at a
/// constant rate each tick and must be periodically satisfied to avoid
/// overflow. Unlike hunger (which reduces effectiveness), craving
/// **amplifies** — the more you want something, the harder you pursue it.
///
/// The passive growth direction makes Yen unique: every other accumulator
/// in this library either requires external events to build (Wok, Woo, Zest)
/// or decays on its own (Young, Zest). Yen fills on its own; the entity's
/// behaviour must drive the drain.
///
/// `tick(dt)` clears one-frame flags first, then advances `yen_level` by
/// `yen_rate * dt` when enabled, clamping at `max_yen`. Fires `just_yearned`
/// the first time `yen_level` reaches `max_yen`. No-op (beyond flag clear)
/// when disabled.
///
/// `satisfy(amount)` reduces `yen_level` by `amount` (floors at 0). Fires
/// `just_satisfied`. No-op when `amount <= 0` or disabled.
///
/// `is_craving()` returns `yen_level > 0.0 && enabled`.
///
/// `is_yearning()` returns `yen_level >= max_yen && enabled` — at
/// maximum unsatisfied desire.
///
/// `yen_fraction()` returns `(yen_level / max_yen).clamp(0.0, 1.0)`.
///
/// `effective_drive(base)` returns `base * (1.0 + yen_fraction())` when
/// enabled — 2× at peak craving, 1× when fully satisfied; `base` when
/// disabled.
///
/// Default: `new(10.0, 1.0)` — max craving 10, growth 1/s.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yen {
    /// Current craving level [0, max_yen].
    pub yen_level: f32,
    /// Maximum craving before yearning. Clamped >= 1.0.
    pub max_yen: f32,
    /// Passive craving growth rate in units/second. Clamped >= 0.0.
    pub yen_rate: f32,
    pub just_yearned: bool,
    pub just_satisfied: bool,
    pub enabled: bool,
}

impl Yen {
    pub fn new(max_yen: f32, yen_rate: f32) -> Self {
        Self {
            yen_level: 0.0,
            max_yen: max_yen.max(1.0),
            yen_rate: yen_rate.max(0.0),
            just_yearned: false,
            just_satisfied: false,
            enabled: true,
        }
    }

    /// Reduce craving by `amount` (floors at 0). Fires `just_satisfied`.
    /// No-op when `amount <= 0` or disabled.
    pub fn satisfy(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.yen_level = (self.yen_level - amount).max(0.0);
        self.just_satisfied = true;
    }

    /// Advance one frame: clear flags, then grow craving. Fires
    /// `just_yearned` the first time level reaches max. No-op (beyond flag
    /// clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_yearned = false;
        self.just_satisfied = false;

        if !self.enabled {
            return;
        }

        let prev = self.yen_level;
        self.yen_level = (self.yen_level + self.yen_rate * dt).min(self.max_yen);
        if prev < self.max_yen && self.yen_level >= self.max_yen {
            self.just_yearned = true;
        }
    }

    /// `true` when craving is non-zero and component is enabled.
    pub fn is_craving(&self) -> bool {
        self.yen_level > 0.0 && self.enabled
    }

    /// `true` when craving is at maximum and component is enabled.
    pub fn is_yearning(&self) -> bool {
        self.yen_level >= self.max_yen && self.enabled
    }

    /// Craving as a fraction of maximum [0.0, 1.0].
    pub fn yen_fraction(&self) -> f32 {
        (self.yen_level / self.max_yen).clamp(0.0, 1.0)
    }

    /// Scale `base` by craving intensity. Returns `base * (1.0 +
    /// yen_fraction())` when enabled — 2× at peak yearning, 1× when
    /// satiated; `base` when disabled.
    pub fn effective_drive(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.yen_fraction())
    }
}

impl Default for Yen {
    fn default() -> Self {
        Self::new(10.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yen {
        Yen::new(10.0, 1.0) // max=10, rate=1/s
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let y = y();
        assert_eq!(y.yen_level, 0.0);
        assert!(!y.just_yearned);
        assert!(!y.just_satisfied);
        assert!(!y.is_craving());
        assert!(!y.is_yearning());
    }

    #[test]
    fn max_yen_clamped_to_one() {
        let y = Yen::new(0.0, 1.0);
        assert!((y.max_yen - 1.0).abs() < 1e-5);
    }

    #[test]
    fn yen_rate_clamped_to_zero() {
        let y = Yen::new(10.0, -5.0);
        assert_eq!(y.yen_rate, 0.0);
    }

    // --- tick ---

    #[test]
    fn tick_grows_passively() {
        let mut y = y(); // rate=1/s
        y.tick(3.0); // 0+3=3
        assert!((y.yen_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clamps_at_max() {
        let mut y = y(); // max=10
        y.tick(20.0); // would be 20, clamped to 10
        assert!((y.yen_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_yearned_on_reaching_max() {
        let mut y = y(); // max=10, rate=1
        y.tick(10.0); // exactly reaches 10
        assert!(y.just_yearned);
    }

    #[test]
    fn tick_fires_just_yearned_crossing_max() {
        let mut y = y();
        y.tick(7.0); // 7.0
        y.tick(5.0); // crosses 10.0
        assert!(y.just_yearned);
    }

    #[test]
    fn tick_just_yearned_clears_next_frame() {
        let mut y = y();
        y.tick(10.0); // just_yearned=true
        y.tick(0.016);
        assert!(!y.just_yearned);
    }

    #[test]
    fn tick_does_not_refire_just_yearned_when_already_maxed() {
        let mut y = y();
        y.tick(10.0); // yearned
        y.tick(1.0); // no further growth
        assert!(!y.just_yearned);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.tick(5.0);
        assert_eq!(y.yen_level, 0.0);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut y = y();
        y.just_yearned = true;
        y.just_satisfied = true;
        y.enabled = false;
        y.tick(0.016);
        assert!(!y.just_yearned);
        assert!(!y.just_satisfied);
    }

    #[test]
    fn zero_rate_never_grows() {
        let mut y = Yen::new(10.0, 0.0);
        y.tick(100.0);
        assert_eq!(y.yen_level, 0.0);
    }

    // --- satisfy ---

    #[test]
    fn satisfy_reduces_level() {
        let mut y = y();
        y.tick(8.0); // 8.0
        y.satisfy(3.0); // 5.0
        assert!((y.yen_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn satisfy_fires_just_satisfied() {
        let mut y = y();
        y.tick(5.0);
        y.satisfy(2.0);
        assert!(y.just_satisfied);
    }

    #[test]
    fn satisfy_floors_at_zero() {
        let mut y = y();
        y.tick(3.0); // 3.0
        y.satisfy(10.0); // would go negative, clamped to 0
        assert_eq!(y.yen_level, 0.0);
    }

    #[test]
    fn satisfy_no_op_with_zero_amount() {
        let mut y = y();
        y.tick(5.0);
        y.satisfy(0.0);
        assert!(!y.just_satisfied);
        assert!((y.yen_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn satisfy_no_op_with_negative_amount() {
        let mut y = y();
        y.tick(5.0);
        y.satisfy(-1.0);
        assert!(!y.just_satisfied);
        assert!((y.yen_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn satisfy_no_op_when_disabled() {
        let mut y = y();
        y.tick(5.0);
        y.enabled = false;
        y.satisfy(5.0);
        assert!((y.yen_level - 5.0).abs() < 1e-4);
        assert!(!y.just_satisfied);
    }

    #[test]
    fn satisfy_when_already_empty() {
        let mut y = y();
        y.satisfy(5.0); // level=0, floors at 0, fires flag
        assert!(y.just_satisfied);
        assert_eq!(y.yen_level, 0.0);
    }

    // --- is_craving / is_yearning ---

    #[test]
    fn is_craving_false_when_empty() {
        let y = y();
        assert!(!y.is_craving());
    }

    #[test]
    fn is_craving_true_after_partial_growth() {
        let mut y = y();
        y.tick(3.0);
        assert!(y.is_craving());
    }

    #[test]
    fn is_craving_false_when_disabled() {
        let mut y = y();
        y.tick(5.0);
        y.enabled = false;
        assert!(!y.is_craving());
    }

    #[test]
    fn is_yearning_false_when_partial() {
        let mut y = y();
        y.tick(5.0);
        assert!(!y.is_yearning());
    }

    #[test]
    fn is_yearning_true_at_max() {
        let mut y = y();
        y.tick(10.0);
        assert!(y.is_yearning());
    }

    #[test]
    fn is_yearning_false_when_disabled() {
        let mut y = y();
        y.tick(10.0);
        y.enabled = false;
        assert!(!y.is_yearning());
    }

    // --- yen_fraction ---

    #[test]
    fn yen_fraction_zero_when_empty() {
        let y = y();
        assert_eq!(y.yen_fraction(), 0.0);
    }

    #[test]
    fn yen_fraction_at_half() {
        let mut y = y(); // max=10
        y.tick(5.0); // 5/10=0.5
        assert!((y.yen_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn yen_fraction_one_at_max() {
        let mut y = y();
        y.tick(10.0);
        assert!((y.yen_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_drive ---

    #[test]
    fn effective_drive_passthrough_when_empty() {
        let y = y(); // fraction=0 → 100*(1+0)=100
        assert!((y.effective_drive(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_drive_at_half_craving() {
        let mut y = y();
        y.tick(5.0); // fraction=0.5 → 100*(1+0.5)=150
        assert!((y.effective_drive(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_drive_doubled_at_max() {
        let mut y = y();
        y.tick(10.0); // fraction=1.0 → 100*(1+1)=200
        assert!((y.effective_drive(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_drive_passthrough_when_disabled() {
        let mut y = y();
        y.tick(10.0);
        y.enabled = false;
        assert!((y.effective_drive(100.0) - 100.0).abs() < 1e-4);
    }

    // --- tick + satisfy cycle ---

    #[test]
    fn grow_then_satisfy_cycle() {
        let mut y = y();
        y.tick(10.0); // full
        y.satisfy(10.0); // emptied
        assert_eq!(y.yen_level, 0.0);
        y.tick(5.0); // grows again
        assert!((y.yen_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn partial_satisfaction_allows_continued_growth() {
        let mut y = y();
        y.tick(8.0); // 8.0
        y.satisfy(4.0); // 4.0
        y.tick(2.0); // 6.0
        assert!((y.yen_level - 6.0).abs() < 1e-4);
    }
}
