use bevy_ecs::prelude::Component;

/// Hesitation accumulator with instant-override reset. Models an entity's
/// accumulated cowardice/avoidance that reduces its effective resolve.
///
/// Unlike `Yowl` (pain decays passively only) and `Wrest` (two-state
/// strain/ease), Wimp combines three mechanisms: external `flinch()` pushes
/// in hesitation, `defy()` instantly resets it (with an override flag), and
/// passive recovery decays it toward 0 over time.
///
/// `effective_resolve` is **inverse** — the only component in this library
/// where the output *reduces* rather than scales up the input.
///
/// `flinch(amount)` adds `amount` to `wimp_level` (clamped to `max_wimp`),
/// fires `just_flinched`. No-op when disabled.
///
/// `defy()` instantly resets `wimp_level` to 0 and fires `just_overcame`.
/// No-op when already at 0 or disabled.
///
/// `tick(dt)` clears one-frame flags first, then if enabled and
/// `wimp_level > 0`: decays by `recover_rate * dt` (floor 0). No other state
/// changes when disabled.
///
/// `is_hesitant()` returns `wimp_level > 0.0 && enabled`.
///
/// `wimp_fraction()` returns `(wimp_level / max_wimp).clamp(0.0, 1.0)`.
///
/// `effective_resolve(base)` returns `base * (1.0 - wimp_fraction())` when
/// enabled — at peak hesitation the return is 0; returns `base` unchanged
/// when disabled.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wimp {
    /// Current hesitation level [0, max_wimp].
    pub wimp_level: f32,
    /// Maximum hesitation capacity. Clamped >= 1.0.
    pub max_wimp: f32,
    /// Passive recovery rate per second. Clamped >= 0.0.
    pub recover_rate: f32,
    pub just_flinched: bool,
    pub just_overcame: bool,
    pub enabled: bool,
}

impl Wimp {
    pub fn new(max_wimp: f32, recover_rate: f32) -> Self {
        Self {
            wimp_level: 0.0,
            max_wimp: max_wimp.max(1.0),
            recover_rate: recover_rate.max(0.0),
            just_flinched: false,
            just_overcame: false,
            enabled: true,
        }
    }

    /// Add hesitation. Clamped to max_wimp. Fires `just_flinched`.
    /// No-op when disabled.
    pub fn flinch(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.wimp_level = (self.wimp_level + amount).clamp(0.0, self.max_wimp);
        self.just_flinched = true;
    }

    /// Instantly override all hesitation: resets `wimp_level` to 0 and fires
    /// `just_overcame`. No-op when already at 0 or disabled.
    pub fn defy(&mut self) {
        if !self.enabled || self.wimp_level == 0.0 {
            return;
        }
        self.wimp_level = 0.0;
        self.just_overcame = true;
    }

    /// Advance one frame: clear flags, then decay `wimp_level` by
    /// `recover_rate * dt` toward 0. No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_flinched = false;
        self.just_overcame = false;

        if !self.enabled {
            return;
        }

        if self.wimp_level > 0.0 {
            self.wimp_level = (self.wimp_level - self.recover_rate * dt).max(0.0);
        }
    }

    /// `true` when hesitation is above 0 and component is enabled.
    pub fn is_hesitant(&self) -> bool {
        self.wimp_level > 0.0 && self.enabled
    }

    /// Hesitation as a fraction of maximum [0.0, 1.0].
    pub fn wimp_fraction(&self) -> f32 {
        (self.wimp_level / self.max_wimp).clamp(0.0, 1.0)
    }

    /// Scale `base` inversely by hesitation. Returns
    /// `base * (1.0 - wimp_fraction())` when enabled — 0.0 at full
    /// hesitation; `base` unchanged when disabled.
    pub fn effective_resolve(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 - self.wimp_fraction())
    }
}

impl Default for Wimp {
    fn default() -> Self {
        Self::new(10.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wimp {
        Wimp::new(10.0, 1.0) // max=10, recovers 1/s
    }

    #[test]
    fn new_starts_calm() {
        let w = w();
        assert_eq!(w.wimp_level, 0.0);
        assert!(!w.just_flinched);
        assert!(!w.just_overcame);
        assert!(!w.is_hesitant());
    }

    #[test]
    fn flinch_increases_level() {
        let mut w = w();
        w.flinch(3.0);
        assert!((w.wimp_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn flinch_fires_just_flinched() {
        let mut w = w();
        w.flinch(1.0);
        assert!(w.just_flinched);
    }

    #[test]
    fn flinch_clamps_to_max() {
        let mut w = w(); // max=10
        w.flinch(20.0);
        assert!((w.wimp_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn flinch_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.flinch(5.0);
        assert_eq!(w.wimp_level, 0.0);
        assert!(!w.just_flinched);
    }

    #[test]
    fn defy_resets_to_zero() {
        let mut w = w();
        w.flinch(7.0);
        w.defy();
        assert_eq!(w.wimp_level, 0.0);
    }

    #[test]
    fn defy_fires_just_overcame() {
        let mut w = w();
        w.flinch(5.0);
        w.defy();
        assert!(w.just_overcame);
    }

    #[test]
    fn defy_no_op_when_already_zero() {
        let mut w = w();
        w.defy();
        assert!(!w.just_overcame);
    }

    #[test]
    fn defy_no_op_when_disabled() {
        let mut w = w();
        w.flinch(5.0);
        w.enabled = false;
        w.defy();
        assert!((w.wimp_level - 5.0).abs() < 1e-4);
        assert!(!w.just_overcame);
    }

    #[test]
    fn tick_clears_just_flinched() {
        let mut w = w();
        w.flinch(1.0);
        w.tick(0.016);
        assert!(!w.just_flinched);
    }

    #[test]
    fn tick_clears_just_overcame() {
        let mut w = w();
        w.flinch(1.0);
        w.defy();
        w.tick(0.016);
        assert!(!w.just_overcame);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_flinched = true;
        w.just_overcame = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_flinched);
        assert!(!w.just_overcame);
    }

    #[test]
    fn tick_decays_level() {
        let mut w = w(); // recover_rate=1.0
        w.flinch(5.0);
        w.tick(2.0); // 5 - 2 = 3
        assert!((w.wimp_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut w = w();
        w.flinch(1.0);
        w.tick(10.0); // 1 - 10, clamp to 0
        assert_eq!(w.wimp_level, 0.0);
    }

    #[test]
    fn tick_no_decay_when_disabled() {
        let mut w = w();
        w.flinch(5.0);
        w.enabled = false;
        w.tick(10.0);
        assert!((w.wimp_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_decay_when_already_zero() {
        let mut w = w();
        w.tick(5.0); // wimp_level=0, no change
        assert_eq!(w.wimp_level, 0.0);
    }

    #[test]
    fn is_hesitant_true_when_positive() {
        let mut w = w();
        w.flinch(1.0);
        assert!(w.is_hesitant());
    }

    #[test]
    fn is_hesitant_false_when_zero() {
        let w = w();
        assert!(!w.is_hesitant());
    }

    #[test]
    fn is_hesitant_false_when_disabled() {
        let mut w = w();
        w.flinch(5.0);
        w.enabled = false;
        assert!(!w.is_hesitant());
    }

    #[test]
    fn wimp_fraction_zero_when_calm() {
        let w = w();
        assert_eq!(w.wimp_fraction(), 0.0);
    }

    #[test]
    fn wimp_fraction_half_at_midpoint() {
        let mut w = w(); // max=10
        w.flinch(5.0); // 5/10=0.5
        assert!((w.wimp_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn wimp_fraction_one_at_max() {
        let mut w = w();
        w.flinch(10.0);
        assert!((w.wimp_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_resolve_passthrough_when_calm() {
        let w = w();
        assert!((w.effective_resolve(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_resolve_halved_at_half_wimp() {
        let mut w = w();
        w.flinch(5.0); // fraction=0.5 → 100*(1-0.5)=50
        assert!((w.effective_resolve(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_resolve_zero_at_max_wimp() {
        let mut w = w();
        w.flinch(10.0); // fraction=1.0 → 100*(1-1.0)=0
        assert!((w.effective_resolve(100.0) - 0.0).abs() < 1e-3);
    }

    #[test]
    fn effective_resolve_passthrough_when_disabled() {
        let mut w = w();
        w.flinch(10.0);
        w.enabled = false;
        assert!((w.effective_resolve(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_wimp_clamped_to_one() {
        let w = Wimp::new(0.0, 1.0);
        assert!((w.max_wimp - 1.0).abs() < 1e-5);
    }

    #[test]
    fn recover_rate_clamped_to_zero() {
        let w = Wimp::new(10.0, -1.0);
        assert_eq!(w.recover_rate, 0.0);
    }

    #[test]
    fn flinch_then_defy_then_flinch_again() {
        let mut w = w();
        w.flinch(8.0);
        w.defy();
        assert_eq!(w.wimp_level, 0.0);
        w.flinch(3.0);
        assert!((w.wimp_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn passive_recovery_reaches_zero_exactly() {
        let mut w = Wimp::new(10.0, 2.0); // recovers 2/s
        w.flinch(4.0);
        w.tick(2.0); // 4 - 4 = 0
        assert_eq!(w.wimp_level, 0.0);
        assert!(!w.is_hesitant());
    }

    #[test]
    fn multiple_flinches_accumulate() {
        let mut w = w();
        w.flinch(3.0);
        w.flinch(4.0); // 7
        assert!((w.wimp_level - 7.0).abs() < 1e-4);
    }

    #[test]
    fn flinch_negative_amount_clamps_to_zero() {
        let mut w = w();
        w.flinch(-5.0); // clamped to 0
        assert_eq!(w.wimp_level, 0.0);
        // still fires just_flinched (flinch ran with enable=true)
        assert!(w.just_flinched);
    }
}
