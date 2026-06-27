use bevy_ecs::prelude::Component;

/// Serenity accumulator: builds passively and drops on disturbance. The
/// inverse of `Zeal` (which builds on events, decays passively): Zen grows
/// automatically each tick and shrinks when `disturb()` is called.
///
/// Models calm/flow states, stealth detection windows, or focus mechanics
/// that deepen during quiet periods and shatter when disrupted.
///
/// `disturb(amount)` reduces `zen_level` by `amount` (floored at 0) when
/// enabled and `amount > 0`. Fires `just_broken` if `zen_level` was above 0
/// before the reduction. No-op when disabled or `amount <= 0`.
///
/// `tick(dt)` clears one-frame flags first. If enabled and `zen_level <
/// max_zen`: adds `restore_rate * dt`, capped at `max_zen`. Fires
/// `just_achieved` the first time `zen_level` reaches `max_zen` from below.
/// No-op (beyond flag clear) when disabled or already at max.
///
/// `is_serene()` returns `zen_level >= max_zen && enabled`.
///
/// `is_unsettled()` returns `zen_level == 0.0 && enabled`.
///
/// `zen_fraction()` returns `(zen_level / max_zen).clamp(0.0, 1.0)`.
///
/// `effective_focus(base)` returns `base * (1.0 + zen_fraction())` when
/// enabled — 1× at 0, 2× at max; `base` when disabled.
///
/// Default: `new(10.0, 1.0)` — max 10, restores at 1 unit/second.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zen {
    /// Current serenity level [0, max_zen].
    pub zen_level: f32,
    /// Maximum serenity. Clamped >= 1.0.
    pub max_zen: f32,
    /// Passive restore rate in units/second. Clamped >= 0.0.
    pub restore_rate: f32,
    pub just_achieved: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Zen {
    pub fn new(max_zen: f32, restore_rate: f32) -> Self {
        Self {
            zen_level: 0.0,
            max_zen: max_zen.max(1.0),
            restore_rate: restore_rate.max(0.0),
            just_achieved: false,
            just_broken: false,
            enabled: true,
        }
    }

    /// Reduce `zen_level` by `amount`. Fires `just_broken` when reducing from
    /// above 0. No-op when `amount <= 0` or disabled.
    pub fn disturb(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        if self.zen_level > 0.0 {
            self.just_broken = true;
        }
        self.zen_level = (self.zen_level - amount).max(0.0);
    }

    /// Advance one frame: clear flags, then restore passively. Fires
    /// `just_achieved` on reaching max. No-op (beyond flag clear) when
    /// disabled or already at max.
    pub fn tick(&mut self, dt: f32) {
        self.just_achieved = false;
        self.just_broken = false;

        if !self.enabled {
            return;
        }

        if self.zen_level < self.max_zen {
            self.zen_level = (self.zen_level + self.restore_rate * dt).min(self.max_zen);
            if self.zen_level >= self.max_zen {
                self.just_achieved = true;
            }
        }
    }

    /// `true` when fully serene: `zen_level >= max_zen` and enabled.
    pub fn is_serene(&self) -> bool {
        self.zen_level >= self.max_zen && self.enabled
    }

    /// `true` when completely unsettled: `zen_level == 0.0` and enabled.
    pub fn is_unsettled(&self) -> bool {
        self.zen_level == 0.0 && self.enabled
    }

    /// Serenity as a fraction of maximum [0.0, 1.0].
    pub fn zen_fraction(&self) -> f32 {
        (self.zen_level / self.max_zen).clamp(0.0, 1.0)
    }

    /// Scale `base` by serenity. Returns `base * (1.0 + zen_fraction())`
    /// when enabled — 1× at 0, 2× at max; `base` when disabled.
    pub fn effective_focus(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.zen_fraction())
    }
}

impl Default for Zen {
    fn default() -> Self {
        Self::new(10.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zen {
        Zen::new(10.0, 1.0) // max=10, restore=1/s
    }

    // --- construction ---

    #[test]
    fn new_starts_unsettled() {
        let z = z();
        assert_eq!(z.zen_level, 0.0);
        assert!(!z.just_achieved);
        assert!(!z.just_broken);
        assert!(z.is_unsettled());
        assert!(!z.is_serene());
    }

    #[test]
    fn max_zen_clamped_to_one() {
        let z = Zen::new(0.0, 1.0);
        assert!((z.max_zen - 1.0).abs() < 1e-5);
    }

    #[test]
    fn restore_rate_clamped_to_zero() {
        let z = Zen::new(10.0, -1.0);
        assert_eq!(z.restore_rate, 0.0);
    }

    // --- disturb ---

    #[test]
    fn disturb_reduces_zen() {
        let mut z = z();
        z.zen_level = 7.0;
        z.disturb(3.0);
        assert!((z.zen_level - 4.0).abs() < 1e-5);
    }

    #[test]
    fn disturb_floors_at_zero() {
        let mut z = z();
        z.zen_level = 2.0;
        z.disturb(10.0);
        assert_eq!(z.zen_level, 0.0);
    }

    #[test]
    fn disturb_fires_just_broken_when_level_above_zero() {
        let mut z = z();
        z.zen_level = 5.0;
        z.disturb(1.0);
        assert!(z.just_broken);
    }

    #[test]
    fn disturb_no_just_broken_when_already_zero() {
        let mut z = z();
        z.disturb(5.0); // already 0
        assert!(!z.just_broken);
    }

    #[test]
    fn disturb_no_op_at_zero_amount() {
        let mut z = z();
        z.zen_level = 5.0;
        z.disturb(0.0);
        assert!((z.zen_level - 5.0).abs() < 1e-5);
        assert!(!z.just_broken);
    }

    #[test]
    fn disturb_no_op_at_negative_amount() {
        let mut z = z();
        z.zen_level = 5.0;
        z.disturb(-3.0);
        assert!((z.zen_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn disturb_no_op_when_disabled() {
        let mut z = z();
        z.zen_level = 5.0;
        z.enabled = false;
        z.disturb(3.0);
        assert!((z.zen_level - 5.0).abs() < 1e-5);
        assert!(!z.just_broken);
    }

    // --- tick: passive restore ---

    #[test]
    fn tick_restores_passively() {
        let mut z = z(); // restore=1/s
        z.tick(3.0);
        assert!((z.zen_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max() {
        let mut z = z();
        z.tick(100.0);
        assert!((z.zen_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_achieved_at_max() {
        let mut z = z();
        z.tick(10.0);
        assert!(z.just_achieved);
        assert!(z.is_serene());
    }

    #[test]
    fn tick_just_achieved_not_refired_when_already_serene() {
        let mut z = z();
        z.tick(10.0); // reaches max
        z.tick(1.0); // already at max — no refire
        assert!(!z.just_achieved);
        assert!((z.zen_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clears_just_achieved_next_frame() {
        let mut z = z();
        z.tick(10.0); // just_achieved=true
        z.tick(0.016);
        assert!(!z.just_achieved);
    }

    #[test]
    fn tick_clears_just_broken() {
        let mut z = z();
        z.zen_level = 5.0;
        z.disturb(1.0); // just_broken=true
        z.tick(0.016);
        assert!(!z.just_broken);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(10.0);
        assert_eq!(z.zen_level, 0.0);
    }

    #[test]
    fn tick_clears_flags_when_disabled() {
        let mut z = z();
        z.just_achieved = true;
        z.just_broken = true;
        z.enabled = false;
        z.tick(0.016);
        assert!(!z.just_achieved);
        assert!(!z.just_broken);
    }

    #[test]
    fn tick_zero_rate_never_builds() {
        let mut z = Zen::new(10.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.zen_level, 0.0);
        assert!(!z.just_achieved);
    }

    // --- is_serene / is_unsettled ---

    #[test]
    fn is_serene_false_at_start() {
        let z = z();
        assert!(!z.is_serene());
    }

    #[test]
    fn is_serene_true_after_full_restore() {
        let mut z = z();
        z.tick(10.0);
        assert!(z.is_serene());
    }

    #[test]
    fn is_serene_false_when_disabled() {
        let mut z = z();
        z.zen_level = 10.0;
        z.enabled = false;
        assert!(!z.is_serene());
    }

    #[test]
    fn is_unsettled_true_at_start() {
        let z = z();
        assert!(z.is_unsettled());
    }

    #[test]
    fn is_unsettled_false_with_any_zen() {
        let mut z = z();
        z.tick(0.1);
        assert!(!z.is_unsettled());
    }

    #[test]
    fn is_unsettled_false_when_disabled() {
        let z_dis = {
            let mut z = z();
            z.enabled = false;
            z
        };
        assert!(!z_dis.is_unsettled());
    }

    // --- zen_fraction ---

    #[test]
    fn zen_fraction_zero_at_start() {
        let z = z();
        assert_eq!(z.zen_fraction(), 0.0);
    }

    #[test]
    fn zen_fraction_half_at_mid() {
        let mut z = z();
        z.tick(5.0);
        assert!((z.zen_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn zen_fraction_one_when_serene() {
        let mut z = z();
        z.tick(10.0);
        assert!((z.zen_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_focus ---

    #[test]
    fn effective_focus_passthrough_at_zero() {
        let z = z();
        assert!((z.effective_focus(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_focus_at_half_zen() {
        let mut z = z();
        z.tick(5.0); // fraction=0.5 → 100*(1+0.5)=150
        assert!((z.effective_focus(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_focus_doubled_when_serene() {
        let mut z = z();
        z.tick(10.0); // fraction=1.0 → 100*(1+1)=200
        assert!((z.effective_focus(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_focus_passthrough_when_disabled() {
        let mut z = z();
        z.tick(10.0);
        z.enabled = false;
        assert!((z.effective_focus(100.0) - 100.0).abs() < 1e-4);
    }

    // --- disturb/restore cycle ---

    #[test]
    fn disturb_then_restore() {
        let mut z = z();
        z.tick(10.0); // fully serene
        z.disturb(5.0); // back to 5
        assert!((z.zen_level - 5.0).abs() < 1e-5);
        assert!(!z.is_serene());
        z.tick(5.0); // restores back to 10
        assert!(z.is_serene());
    }

    #[test]
    fn repeated_disturb_empties_zen() {
        let mut z = z();
        z.tick(10.0); // full
        z.disturb(10.0);
        assert_eq!(z.zen_level, 0.0);
        assert!(z.is_unsettled());
    }
}
