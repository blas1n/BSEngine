use bevy_ecs::prelude::Component;

/// Involuntary pain-response that temporarily degrades an entity's action
/// speed. Systems increase `wince_level` via `flinch(amount)` when the entity
/// receives painful stimulus; the level decays automatically each frame,
/// restoring full capability once it returns to zero.
///
/// `flinch(amount)` increases `wince_level` by `amount` (capped at
/// `max_wince`). Fires `just_winced` on the first reach of `max_wince`. No-op
/// when disabled or `amount <= 0`.
///
/// `tick(dt)` clears one-frame flags then decays `wince_level` by
/// `decay_rate * dt` (floored 0), firing `just_recovered` when the level
/// first reaches 0.0 from positive. No-op when disabled.
///
/// `is_wincing()` returns `wince_level > 0.0 && enabled`.
///
/// `wince_fraction()` returns `(wince_level / max_wince).clamp(0.0, 1.0)`.
///
/// `effective_action(base)` returns
/// `(base * (1.0 - action_penalty * wince_fraction())).max(0.0)` when
/// enabled; returns `base` unchanged otherwise.
///
/// Distinct from `Flinch` (reflex recoil/dodge that repositions the entity),
/// `Stagger` (multi-frame stumble that breaks animation), `Stun` (full
/// incapacitation with hard duration), and `Daze` (confusion penalty to
/// accuracy): Wince is a **proportional pain-response** that scales action
/// throughput continuously with current pain intensity, fading automatically
/// as the entity endures the sensation.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wince {
    /// Current pain response level [0.0, max_wince].
    pub wince_level: f32,
    /// Level at which the entity is fully wincing. Clamped >= 1.0.
    pub max_wince: f32,
    /// Recovery speed in units per second. Clamped >= 0.0.
    pub decay_rate: f32,
    /// Action speed reduction at maximum wince [0.0, 1.0]. Clamped.
    pub action_penalty: f32,
    pub just_winced: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Wince {
    pub fn new(max_wince: f32, decay_rate: f32, action_penalty: f32) -> Self {
        Self {
            wince_level: 0.0,
            max_wince: max_wince.max(1.0),
            decay_rate: decay_rate.max(0.0),
            action_penalty: action_penalty.clamp(0.0, 1.0),
            just_winced: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Apply pain stimulus. Fires `just_winced` on first reach of `max_wince`.
    /// No-op when disabled or `amount <= 0`.
    pub fn flinch(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below_max = self.wince_level < self.max_wince;
        self.wince_level = (self.wince_level + amount).min(self.max_wince);
        if was_below_max && self.wince_level >= self.max_wince {
            self.just_winced = true;
        }
    }

    /// Advance one frame: clear flags, then decay `wince_level` by
    /// `decay_rate * dt`. Fires `just_recovered` on first reach of 0.0. No-op
    /// when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_winced = false;
        self.just_recovered = false;

        if !self.enabled {
            return;
        }
        if self.wince_level > 0.0 {
            let was_positive = self.wince_level > 0.0;
            self.wince_level = (self.wince_level - self.decay_rate * dt).max(0.0);
            if was_positive && self.wince_level == 0.0 {
                self.just_recovered = true;
            }
        }
    }

    /// `true` when any pain response remains and the component is enabled.
    pub fn is_wincing(&self) -> bool {
        self.wince_level > 0.0 && self.enabled
    }

    /// Pain response as a fraction of maximum [0.0, 1.0].
    pub fn wince_fraction(&self) -> f32 {
        (self.wince_level / self.max_wince).clamp(0.0, 1.0)
    }

    /// Scale action `base` by remaining action capacity. Returns
    /// `(base * (1 - penalty * fraction)).max(0)` when enabled; `base`
    /// otherwise.
    pub fn effective_action(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.action_penalty * self.wince_fraction())).max(0.0)
    }
}

impl Default for Wince {
    fn default() -> Self {
        Self::new(10.0, 5.0, 0.6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wince {
        Wince::new(10.0, 0.0, 0.5)
    }

    #[test]
    fn new_starts_calm() {
        let w = Wince::new(10.0, 0.0, 0.5);
        assert_eq!(w.wince_level, 0.0);
        assert!(!w.is_wincing());
        assert!(!w.just_winced);
    }

    #[test]
    fn flinch_increases_wince_level() {
        let mut w = w();
        w.flinch(4.0);
        assert!((w.wince_level - 4.0).abs() < 1e-5);
    }

    #[test]
    fn flinch_caps_at_max_wince() {
        let mut w = w();
        w.flinch(20.0);
        assert!((w.wince_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn flinch_fires_just_winced_on_first_max() {
        let mut w = w();
        w.flinch(10.0);
        assert!(w.just_winced);
        assert!(w.is_wincing());
    }

    #[test]
    fn flinch_no_just_winced_when_already_at_max() {
        let mut w = w();
        w.flinch(10.0);
        w.tick(0.016);
        w.flinch(1.0); // still at max, no re-fire
        assert!(!w.just_winced);
    }

    #[test]
    fn flinch_no_just_winced_below_max() {
        let mut w = w();
        w.flinch(5.0); // below max
        assert!(!w.just_winced);
        assert!(w.is_wincing());
    }

    #[test]
    fn flinch_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.flinch(5.0);
        assert_eq!(w.wince_level, 0.0);
    }

    #[test]
    fn flinch_no_op_when_amount_zero() {
        let mut w = w();
        w.flinch(0.0);
        assert_eq!(w.wince_level, 0.0);
    }

    #[test]
    fn flinch_no_op_when_amount_negative() {
        let mut w = w();
        w.flinch(-3.0);
        assert_eq!(w.wince_level, 0.0);
    }

    #[test]
    fn tick_clears_just_winced() {
        let mut w = w();
        w.flinch(10.0);
        w.tick(0.016);
        assert!(!w.just_winced);
    }

    #[test]
    fn tick_decays_wince_level() {
        let mut w = Wince::new(10.0, 2.0, 0.5);
        w.flinch(6.0);
        w.tick(1.0); // decay: 2.0*1.0 = 2.0, 6.0 → 4.0
        assert!((w.wince_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_wince_at_zero() {
        let mut w = Wince::new(10.0, 20.0, 0.5);
        w.flinch(3.0);
        w.tick(1.0); // decay 20.0 would overshoot, floor at 0
        assert_eq!(w.wince_level, 0.0);
    }

    #[test]
    fn tick_fires_just_recovered_when_wince_reaches_zero() {
        let mut w = Wince::new(10.0, 20.0, 0.5);
        w.flinch(3.0);
        w.tick(0.016); // clear just_winced (if any)... wait, 3.0 < 10.0 so no just_winced
        w.tick(1.0); // full decay, just_recovered
        assert!(w.just_recovered);
        assert!(!w.is_wincing());
    }

    #[test]
    fn tick_no_just_recovered_when_level_remains() {
        let mut w = Wince::new(10.0, 1.0, 0.5);
        w.flinch(8.0);
        w.tick(1.0); // 8.0 → 7.0, still wincing
        assert!(!w.just_recovered);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = Wince::new(10.0, 5.0, 0.5);
        w.enabled = false;
        w.flinch(5.0);
        w.enabled = true;
        w.flinch(5.0); // set to 5
        w.enabled = false;
        w.tick(10.0); // should not decay
        assert!((w.wince_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_zero_decay_no_change() {
        let mut w = Wince::new(10.0, 0.0, 0.5);
        w.flinch(5.0);
        w.tick(100.0);
        assert!((w.wince_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_recovered_next_frame() {
        let mut w = Wince::new(10.0, 20.0, 0.5);
        w.flinch(3.0);
        w.tick(1.0); // just_recovered fires
        assert!(w.just_recovered);
        w.tick(0.016); // cleared
        assert!(!w.just_recovered);
    }

    #[test]
    fn is_wincing_true_when_positive() {
        let mut w = w();
        w.flinch(1.0);
        assert!(w.is_wincing());
    }

    #[test]
    fn is_wincing_false_when_zero() {
        let w = w();
        assert!(!w.is_wincing());
    }

    #[test]
    fn is_wincing_false_when_disabled() {
        let mut w = w();
        w.flinch(5.0);
        w.enabled = false;
        assert!(!w.is_wincing());
    }

    #[test]
    fn wince_fraction_zero_when_calm() {
        let w = w();
        assert_eq!(w.wince_fraction(), 0.0);
    }

    #[test]
    fn wince_fraction_half_at_midpoint() {
        let mut w = w();
        w.flinch(5.0);
        assert!((w.wince_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn wince_fraction_one_at_max() {
        let mut w = w();
        w.flinch(10.0);
        assert!((w.wince_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_action_full_at_zero_wince() {
        let w = Wince::new(10.0, 0.0, 0.5);
        assert!((w.effective_action(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_action_half_at_full_wince_with_0_5_penalty() {
        let mut w = Wince::new(10.0, 0.0, 0.5);
        w.flinch(10.0);
        assert!((w.effective_action(100.0) - 50.0).abs() < 1e-4);
    }

    #[test]
    fn effective_action_zero_at_full_wince_with_1_0_penalty() {
        let mut w = Wince::new(10.0, 0.0, 1.0);
        w.flinch(10.0);
        assert!((w.effective_action(100.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn effective_action_partial_at_half_wince() {
        let mut w = Wince::new(10.0, 0.0, 0.5);
        w.flinch(5.0); // fraction = 0.5
                       // 100 * (1 - 0.5*0.5) = 75
        assert!((w.effective_action(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_action_passthrough_when_disabled() {
        let mut w = Wince::new(10.0, 0.0, 1.0);
        w.flinch(10.0);
        w.enabled = false;
        assert!((w.effective_action(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_action_floored_at_zero() {
        let mut w = Wince::new(10.0, 0.0, 1.0);
        w.flinch(10.0);
        assert!(w.effective_action(100.0) >= 0.0);
    }

    #[test]
    fn max_wince_clamped_to_one() {
        let w = Wince::new(0.0, 0.0, 0.5);
        assert!((w.max_wince - 1.0).abs() < 1e-5);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let w = Wince::new(10.0, -5.0, 0.5);
        assert_eq!(w.decay_rate, 0.0);
    }

    #[test]
    fn action_penalty_clamped_high() {
        let w = Wince::new(10.0, 0.0, 2.0);
        assert!((w.action_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn action_penalty_clamped_low() {
        let w = Wince::new(10.0, 0.0, -1.0);
        assert_eq!(w.action_penalty, 0.0);
    }

    #[test]
    fn multiple_flinches_accumulate() {
        let mut w = w();
        w.flinch(2.0);
        w.flinch(3.0);
        w.flinch(1.5);
        assert!((w.wince_level - 6.5).abs() < 1e-4);
    }

    #[test]
    fn flinch_then_decay_then_flinch_again() {
        let mut w = Wince::new(10.0, 5.0, 0.5);
        w.flinch(10.0); // just_winced
        w.tick(2.0); // 10 → 0, just_recovered
        assert!(w.just_recovered);
        w.flinch(10.0); // just_winced again
        assert!(w.just_winced);
    }

    #[test]
    fn just_winced_fires_only_on_crossing_max() {
        let mut w = w();
        w.flinch(9.0); // below max, no just_winced
        assert!(!w.just_winced);
        w.flinch(1.0); // crosses max, just_winced
        assert!(w.just_winced);
    }
}
