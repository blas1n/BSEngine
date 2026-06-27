use bevy_ecs::prelude::Component;

/// Progressive wasting debuff that ramps up outgoing damage reduction over
/// time. While `shriveled`, `shrivel_fraction` climbs toward 1.0 at
/// `shrivel_rate` per second; when the affliction is removed, the fraction
/// decays back to 0.0 at `recovery_rate` per second.
///
/// `afflict()` sets `shriveled = true` and fires `just_afflicted` on the
/// false → true transition. No-op when already shriveled or disabled.
///
/// `cleanse()` sets `shriveled = false`. No-op when not shriveled or disabled.
///
/// `tick(dt)` clears one-frame flags at start; when shriveled, increments
/// `shrivel_fraction` (capped at 1.0); when not shriveled, decrements it
/// (floored at 0.0) and fires `just_recovered` on the first tick it reaches
/// zero (only if it was > 0 beforehand). No-op when disabled.
///
/// `is_shriveled()` returns `shriveled && enabled`.
///
/// `effective_outgoing(base)` returns
/// `base * (1.0 - shrivel_factor * shrivel_fraction)` when enabled, floored
/// at 0.0; returns `base` when disabled.
///
/// Distinct from `Weaken` (static flat damage reduction, no ramp),
/// `Enervate` (reduces ability effectiveness),
/// `Drain` (leaches HP/mana each tick), and
/// `Corrode` (reduces armour penetration threshold): Shrivel is a
/// **progressive wasting** — the longer the curse persists, the less damage
/// the entity deals; the effect lingers after cleanse and fades gradually.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Shrivel {
    /// Current wasting depth [0.0 = unaffected, 1.0 = fully shriveled].
    pub shrivel_fraction: f32,
    /// Rate at which `shrivel_fraction` increases per second while afflicted. Clamped >= 0.0.
    pub shrivel_rate: f32,
    /// Rate at which `shrivel_fraction` decays per second after cleanse. Clamped >= 0.0.
    pub recovery_rate: f32,
    /// Maximum outgoing damage reduction at full shrivel. Clamped [0.0, 1.0].
    pub shrivel_factor: f32,
    pub shriveled: bool,
    pub just_afflicted: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Shrivel {
    pub fn new(shrivel_rate: f32, recovery_rate: f32, shrivel_factor: f32) -> Self {
        Self {
            shrivel_fraction: 0.0,
            shrivel_rate: shrivel_rate.max(0.0),
            recovery_rate: recovery_rate.max(0.0),
            shrivel_factor: shrivel_factor.clamp(0.0, 1.0),
            shriveled: false,
            just_afflicted: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Apply the shrivel curse. Fires `just_afflicted` on first application.
    /// No-op when already shriveled or disabled.
    pub fn afflict(&mut self) {
        if !self.enabled || self.shriveled {
            return;
        }
        self.shriveled = true;
        self.just_afflicted = true;
    }

    /// Remove the shrivel curse. The fraction continues to decay each `tick`.
    /// No-op when not shriveled or disabled.
    pub fn cleanse(&mut self) {
        if !self.enabled || !self.shriveled {
            return;
        }
        self.shriveled = false;
    }

    /// Advance the shrivel state. Clears one-frame flags first; grows or
    /// decays `shrivel_fraction` depending on whether `shriveled` is set.
    /// Fires `just_recovered` when the fraction reaches 0 after being positive.
    /// No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_afflicted = false;
        self.just_recovered = false;

        if !self.enabled {
            return;
        }

        if self.shriveled {
            self.shrivel_fraction = (self.shrivel_fraction + self.shrivel_rate * dt).min(1.0);
        } else if self.shrivel_fraction > 0.0 {
            let prev = self.shrivel_fraction;
            self.shrivel_fraction = (self.shrivel_fraction - self.recovery_rate * dt).max(0.0);
            if prev > 0.0 && self.shrivel_fraction == 0.0 {
                self.just_recovered = true;
            }
        }
    }

    /// `true` when currently shriveled and the component is enabled.
    pub fn is_shriveled(&self) -> bool {
        self.shriveled && self.enabled
    }

    /// Outgoing damage reduced by accumulated wasting. Returns
    /// `base * (1.0 - shrivel_factor * shrivel_fraction)` when enabled,
    /// floored at 0.0. Returns `base` when disabled.
    pub fn effective_outgoing(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.shrivel_factor * self.shrivel_fraction)).max(0.0)
    }
}

impl Default for Shrivel {
    fn default() -> Self {
        Self::new(0.2, 0.1, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_clean() {
        let s = Shrivel::new(0.2, 0.1, 0.5);
        assert!(!s.shriveled);
        assert_eq!(s.shrivel_fraction, 0.0);
        assert!(!s.is_shriveled());
    }

    #[test]
    fn afflict_sets_shriveled() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.afflict();
        assert!(s.shriveled);
        assert!(s.just_afflicted);
        assert!(s.is_shriveled());
    }

    #[test]
    fn afflict_no_op_when_already_shriveled() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.afflict();
        s.tick(0.0);
        s.afflict();
        assert!(!s.just_afflicted);
    }

    #[test]
    fn afflict_no_op_when_disabled() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.enabled = false;
        s.afflict();
        assert!(!s.shriveled);
    }

    #[test]
    fn cleanse_removes_shriveled() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.afflict();
        s.cleanse();
        assert!(!s.shriveled);
    }

    #[test]
    fn cleanse_no_op_when_not_shriveled() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.cleanse(); // should not panic
        assert!(!s.shriveled);
    }

    #[test]
    fn cleanse_no_op_when_disabled() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.afflict();
        s.enabled = false;
        s.cleanse();
        // enabled=false makes cleanse no-op so shriveled stays true
        assert!(s.shriveled);
    }

    #[test]
    fn tick_increases_fraction_while_shriveled() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.afflict();
        s.tick(1.0);
        assert!((s.shrivel_fraction - 0.2).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_fraction_at_one() {
        let mut s = Shrivel::new(1.0, 0.1, 0.5);
        s.afflict();
        s.tick(10.0);
        assert!((s.shrivel_fraction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_fraction_after_cleanse() {
        let mut s = Shrivel::new(1.0, 0.5, 0.5);
        s.afflict();
        s.tick(1.0); // fraction = 1.0
        s.cleanse();
        s.tick(1.0); // fraction = 0.5
        assert!((s.shrivel_fraction - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_recovered_when_fraction_reaches_zero() {
        let mut s = Shrivel::new(1.0, 1.0, 0.5);
        s.afflict();
        s.tick(0.5); // fraction = 0.5
        s.cleanse();
        s.tick(0.5); // fraction = 0.0
        assert!(s.just_recovered);
    }

    #[test]
    fn tick_no_just_recovered_while_fraction_still_positive() {
        let mut s = Shrivel::new(1.0, 0.5, 0.5);
        s.afflict();
        s.tick(1.0); // fraction = 1.0
        s.cleanse();
        s.tick(0.5); // fraction = 0.5 — not recovered yet
        assert!(!s.just_recovered);
    }

    #[test]
    fn tick_no_just_recovered_if_fraction_was_already_zero() {
        let mut s = Shrivel::new(0.2, 0.5, 0.5);
        // never afflicted → fraction = 0 the whole time
        s.tick(1.0);
        assert!(!s.just_recovered);
    }

    #[test]
    fn tick_clears_just_afflicted_next_frame() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.afflict();
        s.tick(0.016);
        assert!(!s.just_afflicted);
    }

    #[test]
    fn tick_clears_just_recovered_next_frame() {
        let mut s = Shrivel::new(1.0, 1.0, 0.5);
        s.afflict();
        s.tick(1.0); // fraction=1.0
        s.cleanse();
        s.tick(1.0); // just_recovered=true
        s.tick(0.016); // cleared
        assert!(!s.just_recovered);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.afflict();
        s.enabled = false;
        s.tick(1.0);
        assert_eq!(s.shrivel_fraction, 0.0); // no change
    }

    #[test]
    fn tick_no_increase_when_clean_and_fraction_is_zero() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.tick(5.0);
        assert_eq!(s.shrivel_fraction, 0.0);
    }

    #[test]
    fn is_shriveled_false_when_disabled() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.shriveled = true;
        s.enabled = false;
        assert!(!s.is_shriveled());
    }

    #[test]
    fn effective_outgoing_reduced_by_fraction() {
        let mut s = Shrivel::new(1.0, 0.1, 0.5);
        s.afflict();
        s.tick(1.0); // fraction = 1.0
                     // 100 * (1.0 - 0.5 * 1.0) = 50
        assert!((s.effective_outgoing(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_partial_shrivel() {
        let mut s = Shrivel::new(1.0, 0.1, 0.5);
        s.afflict();
        s.tick(0.5); // fraction = 0.5
                     // 100 * (1.0 - 0.5 * 0.5) = 100 * 0.75 = 75
        assert!((s.effective_outgoing(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_base_when_fraction_zero() {
        let s = Shrivel::new(0.2, 0.1, 0.5);
        assert!((s.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_base_when_disabled() {
        let mut s = Shrivel::new(0.2, 0.1, 0.5);
        s.shrivel_fraction = 1.0;
        s.enabled = false;
        assert!((s.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_floored_at_zero() {
        let mut s = Shrivel::new(1.0, 0.0, 1.0);
        s.afflict();
        s.tick(1.0); // fraction = 1.0; factor = 1.0 → 100 * 0 = 0
        assert_eq!(s.effective_outgoing(100.0), 0.0);
    }

    #[test]
    fn shrivel_rate_clamped_to_zero() {
        let s = Shrivel::new(-0.5, 0.1, 0.5);
        assert_eq!(s.shrivel_rate, 0.0);
    }

    #[test]
    fn recovery_rate_clamped_to_zero() {
        let s = Shrivel::new(0.2, -0.5, 0.5);
        assert_eq!(s.recovery_rate, 0.0);
    }

    #[test]
    fn shrivel_factor_clamped_to_one() {
        let s = Shrivel::new(0.2, 0.1, 2.0);
        assert!((s.shrivel_factor - 1.0).abs() < 1e-5);
    }

    #[test]
    fn shrivel_factor_clamped_to_zero() {
        let s = Shrivel::new(0.2, 0.1, -0.5);
        assert_eq!(s.shrivel_factor, 0.0);
    }

    #[test]
    fn re_afflict_after_recovery_fires_just_afflicted() {
        let mut s = Shrivel::new(1.0, 1.0, 0.5);
        s.afflict();
        s.tick(0.0); // clear flags
        s.cleanse();
        s.tick(10.0); // fully recovered
        s.afflict();
        assert!(s.just_afflicted);
    }

    #[test]
    fn fraction_persists_after_cleanse_until_fully_recovered() {
        let mut s = Shrivel::new(1.0, 0.5, 0.5);
        s.afflict();
        s.tick(1.0); // fraction = 1.0
        s.cleanse();
        // still affects output until fully recovered
        assert!(s.shrivel_fraction > 0.0);
        assert!(s.effective_outgoing(100.0) < 100.0);
    }
}
