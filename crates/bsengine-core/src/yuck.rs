use bevy_ecs::prelude::Component;

/// Self-decaying disgust accumulator. Models repulsion or contamination
/// that is applied by external events and fades naturally each tick.
/// Penalises effectiveness while active — the more disgusting, the worse
/// the output.
///
/// Unlike `Wart` (permanent integer stacks that require explicit removal),
/// Yuck is a continuous float that decays passively: leave it alone and it
/// clears itself. Unlike `Woo` (attraction that amplifies output positively),
/// Yuck is the inverse: it suppresses output to 0 at maximum fouling.
///
/// `taint(amount)` adds disgust. Fires `just_tainted`. No-op when
/// `amount <= 0` or disabled.
///
/// `cleanse()` immediately zeros `yuck_level`. Fires `just_cleansed`. No-op
/// when already clean or disabled.
///
/// `tick(dt)` clears one-frame flags first, then decays `yuck_level` by
/// `decay_rate * dt` (floors at 0) when enabled. Fires `just_cleansed` the
/// first time `yuck_level` reaches 0 via decay. No-op (beyond flag clear)
/// when disabled.
///
/// `is_tainted()` returns `yuck_level > 0.0 && enabled`.
///
/// `is_foul()` returns `yuck_level >= max_yuck && enabled`.
///
/// `yuck_fraction()` returns `(yuck_level / max_yuck).clamp(0.0, 1.0)`.
///
/// `effective_output(base)` returns `base * (1.0 - yuck_fraction())` when
/// enabled — full at 0 disgust, 0 at maximum fouling; `base` when disabled.
///
/// Default: `new(10.0, 1.0)` — max disgust 10, decay 1/s.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yuck {
    /// Current disgust level [0, max_yuck].
    pub yuck_level: f32,
    /// Maximum disgust before fully foul. Clamped >= 1.0.
    pub max_yuck: f32,
    /// Passive decay rate in units/second. Clamped >= 0.0.
    pub decay_rate: f32,
    pub just_tainted: bool,
    pub just_cleansed: bool,
    pub enabled: bool,
}

impl Yuck {
    pub fn new(max_yuck: f32, decay_rate: f32) -> Self {
        Self {
            yuck_level: 0.0,
            max_yuck: max_yuck.max(1.0),
            decay_rate: decay_rate.max(0.0),
            just_tainted: false,
            just_cleansed: false,
            enabled: true,
        }
    }

    /// Apply `amount` of disgust. Clamps at `max_yuck`. Fires `just_tainted`.
    /// No-op when `amount <= 0` or disabled.
    pub fn taint(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.yuck_level = (self.yuck_level + amount).min(self.max_yuck);
        self.just_tainted = true;
    }

    /// Instantly remove all disgust. Fires `just_cleansed`. No-op when
    /// already clean or disabled.
    pub fn cleanse(&mut self) {
        if !self.enabled || self.yuck_level == 0.0 {
            return;
        }
        self.yuck_level = 0.0;
        self.just_cleansed = true;
    }

    /// Advance one frame: clear flags, then decay disgust. Fires
    /// `just_cleansed` the first time `yuck_level` reaches 0 via decay.
    /// No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_tainted = false;
        self.just_cleansed = false;

        if !self.enabled || self.yuck_level == 0.0 {
            return;
        }

        let prev = self.yuck_level;
        self.yuck_level = (self.yuck_level - self.decay_rate * dt).max(0.0);
        if prev > 0.0 && self.yuck_level == 0.0 {
            self.just_cleansed = true;
        }
    }

    /// `true` when any disgust is present and component is enabled.
    pub fn is_tainted(&self) -> bool {
        self.yuck_level > 0.0 && self.enabled
    }

    /// `true` when disgust is at maximum and component is enabled.
    pub fn is_foul(&self) -> bool {
        self.yuck_level >= self.max_yuck && self.enabled
    }

    /// Disgust as a fraction of maximum [0.0, 1.0].
    pub fn yuck_fraction(&self) -> f32 {
        (self.yuck_level / self.max_yuck).clamp(0.0, 1.0)
    }

    /// Scale `base` inversely by disgust. Returns `base * (1.0 -
    /// yuck_fraction())` when enabled — 1× when clean, 0× when fully foul;
    /// `base` when disabled.
    pub fn effective_output(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 - self.yuck_fraction())
    }
}

impl Default for Yuck {
    fn default() -> Self {
        Self::new(10.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yuck {
        Yuck::new(10.0, 1.0) // max=10, decay 1/s
    }

    // --- construction ---

    #[test]
    fn new_starts_clean() {
        let y = y();
        assert_eq!(y.yuck_level, 0.0);
        assert!(!y.just_tainted);
        assert!(!y.just_cleansed);
        assert!(!y.is_tainted());
        assert!(!y.is_foul());
    }

    #[test]
    fn max_yuck_clamped_to_one() {
        let y = Yuck::new(0.0, 1.0);
        assert!((y.max_yuck - 1.0).abs() < 1e-5);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let y = Yuck::new(10.0, -2.0);
        assert_eq!(y.decay_rate, 0.0);
    }

    // --- taint ---

    #[test]
    fn taint_increases_level() {
        let mut y = y();
        y.taint(4.0);
        assert!((y.yuck_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn taint_fires_just_tainted() {
        let mut y = y();
        y.taint(3.0);
        assert!(y.just_tainted);
    }

    #[test]
    fn taint_clamps_at_max() {
        let mut y = y(); // max=10
        y.taint(15.0);
        assert!((y.yuck_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn taint_activates_is_tainted() {
        let mut y = y();
        y.taint(1.0);
        assert!(y.is_tainted());
    }

    #[test]
    fn taint_activates_is_foul_at_max() {
        let mut y = y();
        y.taint(10.0);
        assert!(y.is_foul());
    }

    #[test]
    fn taint_no_op_with_zero() {
        let mut y = y();
        y.taint(0.0);
        assert_eq!(y.yuck_level, 0.0);
        assert!(!y.just_tainted);
    }

    #[test]
    fn taint_no_op_with_negative() {
        let mut y = y();
        y.taint(-1.0);
        assert_eq!(y.yuck_level, 0.0);
        assert!(!y.just_tainted);
    }

    #[test]
    fn taint_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.taint(5.0);
        assert_eq!(y.yuck_level, 0.0);
        assert!(!y.just_tainted);
    }

    // --- cleanse ---

    #[test]
    fn cleanse_zeros_level() {
        let mut y = y();
        y.taint(7.0);
        y.cleanse();
        assert_eq!(y.yuck_level, 0.0);
    }

    #[test]
    fn cleanse_fires_just_cleansed() {
        let mut y = y();
        y.taint(5.0);
        y.cleanse();
        assert!(y.just_cleansed);
    }

    #[test]
    fn cleanse_no_op_when_already_clean() {
        let mut y = y();
        y.cleanse();
        assert!(!y.just_cleansed);
    }

    #[test]
    fn cleanse_no_op_when_disabled() {
        let mut y = y();
        y.taint(5.0);
        y.enabled = false;
        y.cleanse();
        assert!((y.yuck_level - 5.0).abs() < 1e-4);
        assert!(!y.just_cleansed);
    }

    // --- tick ---

    #[test]
    fn tick_decays_passively() {
        let mut y = y(); // decay=1/s
        y.taint(8.0);
        y.tick(3.0); // 8-3=5
        assert!((y.yuck_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut y = y();
        y.taint(3.0);
        y.tick(10.0); // over
        assert_eq!(y.yuck_level, 0.0);
    }

    #[test]
    fn tick_fires_just_cleansed_when_reaching_zero() {
        let mut y = y();
        y.taint(2.0);
        y.tick(2.0); // exactly reaches 0
        assert!(y.just_cleansed);
    }

    #[test]
    fn tick_fires_just_cleansed_crossing_zero() {
        let mut y = y();
        y.taint(1.0);
        y.tick(5.0); // crosses 0
        assert!(y.just_cleansed);
    }

    #[test]
    fn tick_just_cleansed_clears_next_frame() {
        let mut y = y();
        y.taint(1.0);
        y.tick(1.0); // just_cleansed=true
        y.tick(0.016);
        assert!(!y.just_cleansed);
    }

    #[test]
    fn tick_does_not_refire_just_cleansed_when_already_clean() {
        let mut y = y();
        y.tick(1.0); // already clean
        assert!(!y.just_cleansed);
    }

    #[test]
    fn tick_clears_just_tainted() {
        let mut y = y();
        y.taint(5.0); // just_tainted=true
        y.tick(0.016);
        assert!(!y.just_tainted);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut y = y();
        y.taint(5.0);
        y.enabled = false;
        y.tick(5.0);
        assert!((y.yuck_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut y = y();
        y.just_tainted = true;
        y.just_cleansed = true;
        y.enabled = false;
        y.tick(0.016);
        assert!(!y.just_tainted);
        assert!(!y.just_cleansed);
    }

    #[test]
    fn zero_decay_rate_never_clears() {
        let mut y = Yuck::new(10.0, 0.0);
        y.taint(5.0);
        y.tick(100.0);
        assert!((y.yuck_level - 5.0).abs() < 1e-4);
    }

    // --- is_tainted / is_foul ---

    #[test]
    fn is_tainted_false_when_clean() {
        let y = y();
        assert!(!y.is_tainted());
    }

    #[test]
    fn is_tainted_true_after_taint() {
        let mut y = y();
        y.taint(1.0);
        assert!(y.is_tainted());
    }

    #[test]
    fn is_tainted_false_when_disabled() {
        let mut y = y();
        y.taint(5.0);
        y.enabled = false;
        assert!(!y.is_tainted());
    }

    #[test]
    fn is_foul_false_when_partial() {
        let mut y = y();
        y.taint(5.0);
        assert!(!y.is_foul());
    }

    #[test]
    fn is_foul_true_at_max() {
        let mut y = y();
        y.taint(10.0);
        assert!(y.is_foul());
    }

    #[test]
    fn is_foul_false_when_disabled() {
        let mut y = y();
        y.taint(10.0);
        y.enabled = false;
        assert!(!y.is_foul());
    }

    // --- yuck_fraction ---

    #[test]
    fn yuck_fraction_zero_when_clean() {
        let y = y();
        assert_eq!(y.yuck_fraction(), 0.0);
    }

    #[test]
    fn yuck_fraction_at_half() {
        let mut y = y(); // max=10
        y.taint(5.0); // 5/10=0.5
        assert!((y.yuck_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn yuck_fraction_one_at_max() {
        let mut y = y();
        y.taint(10.0);
        assert!((y.yuck_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_output ---

    #[test]
    fn effective_output_passthrough_when_clean() {
        let y = y();
        assert!((y.effective_output(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_output_halved_at_half_disgust() {
        let mut y = y();
        y.taint(5.0); // fraction=0.5 → 100*(1-0.5)=50
        assert!((y.effective_output(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_zero_at_max_disgust() {
        let mut y = y();
        y.taint(10.0); // fraction=1.0 → 100*(1-1)=0
        assert!((y.effective_output(100.0) - 0.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_passthrough_when_disabled() {
        let mut y = y();
        y.taint(10.0);
        y.enabled = false;
        assert!((y.effective_output(100.0) - 100.0).abs() < 1e-4);
    }

    // --- taint + decay cycle ---

    #[test]
    fn taint_then_decay_cycle() {
        let mut y = y();
        y.taint(10.0); // foul
        y.tick(5.0); // 5.0
        assert!((y.yuck_level - 5.0).abs() < 1e-4);
        y.tick(5.0); // 0.0
        assert!(y.just_cleansed);
    }

    #[test]
    fn retaint_after_cleanse() {
        let mut y = y();
        y.taint(10.0);
        y.cleanse();
        y.tick(0.016); // clear flags
        y.taint(5.0);
        assert!(y.just_tainted);
        assert!((y.yuck_level - 5.0).abs() < 1e-4);
    }
}
