use bevy_ecs::prelude::Component;

/// Constriction force applied by a single enveloping grip — a serpent's coil,
/// a tentacle, binding vines — that tightens automatically while the source
/// holds on and loosens once released.
///
/// `hold()` begins active constriction (`wrapping = true`). No-op when
/// already holding or disabled.
///
/// `release()` stops active constriction (`wrapping = false`). No-op when
/// not wrapping.
///
/// `tick(dt)` clears one-frame flags, then:
/// - when `wrapping`: increases `wrap_level` by `tighten_rate * dt` (capped
///   at `max_wrap`), firing `just_gripped` on first reach of `max_wrap`;
/// - when `!wrapping`: decreases `wrap_level` by `loosen_rate * dt` (floored
///   0), firing `just_freed` on first reach of 0.0 from positive.
/// No-op when disabled.
///
/// `is_gripped()` returns `wrap_level > 0.0 && enabled`.
///
/// `wrap_fraction()` returns `(wrap_level / max_wrap).clamp(0.0, 1.0)`.
///
/// `effective_speed(base)` returns
/// `(base * (1.0 - mobility_penalty * wrap_fraction())).max(0.0)` when
/// enabled; returns `base` unchanged otherwise.
///
/// Distinct from `Web` (projectile adhesive that stacks per hit and requires
/// explicit struggle to remove), `Entangle` (plant-root immobilisation),
/// `Snare` (ground-trigger trap), and `Root` (instant hard-stop): Wrap models
/// **a single continuous constricting grip** that tightens over time while
/// held, loosens gradually after release, and scales movement penalty
/// proportionally to current tightness.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrap {
    /// Current constriction force [0.0, max_wrap].
    pub wrap_level: f32,
    /// Maximum constriction. Clamped >= 1.0.
    pub max_wrap: f32,
    /// Tightening rate in units per second while holding. Clamped >= 0.0.
    pub tighten_rate: f32,
    /// Loosening rate in units per second after release. Clamped >= 0.0.
    pub loosen_rate: f32,
    /// Movement reduction at full constriction [0.0, 1.0]. Clamped.
    pub mobility_penalty: f32,
    /// Whether the grip source is actively constricting.
    pub wrapping: bool,
    pub just_gripped: bool,
    pub just_freed: bool,
    pub enabled: bool,
}

impl Wrap {
    pub fn new(max_wrap: f32, tighten_rate: f32, loosen_rate: f32, mobility_penalty: f32) -> Self {
        Self {
            wrap_level: 0.0,
            max_wrap: max_wrap.max(1.0),
            tighten_rate: tighten_rate.max(0.0),
            loosen_rate: loosen_rate.max(0.0),
            mobility_penalty: mobility_penalty.clamp(0.0, 1.0),
            wrapping: false,
            just_gripped: false,
            just_freed: false,
            enabled: true,
        }
    }

    /// Begin active constriction. No-op when already holding or disabled.
    pub fn hold(&mut self) {
        if !self.enabled || self.wrapping {
            return;
        }
        self.wrapping = true;
    }

    /// Stop active constriction. No-op when not wrapping.
    pub fn release(&mut self) {
        if !self.wrapping {
            return;
        }
        self.wrapping = false;
    }

    /// Advance one frame: clear flags, then tighten or loosen. No-op when
    /// disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_gripped = false;
        self.just_freed = false;

        if !self.enabled {
            return;
        }
        if self.wrapping {
            let was_below_max = self.wrap_level < self.max_wrap;
            self.wrap_level = (self.wrap_level + self.tighten_rate * dt).min(self.max_wrap);
            if was_below_max && self.wrap_level >= self.max_wrap {
                self.just_gripped = true;
            }
        } else if self.wrap_level > 0.0 {
            let was_positive = self.wrap_level > 0.0;
            self.wrap_level = (self.wrap_level - self.loosen_rate * dt).max(0.0);
            if was_positive && self.wrap_level == 0.0 {
                self.just_freed = true;
            }
        }
    }

    /// `true` when any constriction remains and the component is enabled.
    pub fn is_gripped(&self) -> bool {
        self.wrap_level > 0.0 && self.enabled
    }

    /// Constriction as a fraction of maximum [0.0, 1.0].
    pub fn wrap_fraction(&self) -> f32 {
        (self.wrap_level / self.max_wrap).clamp(0.0, 1.0)
    }

    /// Scale movement `base` by remaining mobility. Returns
    /// `(base * (1 - penalty * fraction)).max(0)` when enabled; `base`
    /// otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.mobility_penalty * self.wrap_fraction())).max(0.0)
    }
}

impl Default for Wrap {
    fn default() -> Self {
        Self::new(10.0, 3.0, 2.0, 0.9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wrap {
        Wrap::new(10.0, 5.0, 2.0, 0.5)
    }

    #[test]
    fn new_starts_free_and_not_wrapping() {
        let w = Wrap::new(10.0, 5.0, 2.0, 0.5);
        assert_eq!(w.wrap_level, 0.0);
        assert!(!w.wrapping);
        assert!(!w.is_gripped());
        assert!(!w.just_gripped);
    }

    #[test]
    fn hold_sets_wrapping() {
        let mut w = w();
        w.hold();
        assert!(w.wrapping);
    }

    #[test]
    fn hold_no_op_when_already_wrapping() {
        let mut w = w();
        w.hold();
        w.hold(); // second call — no error, still wrapping
        assert!(w.wrapping);
    }

    #[test]
    fn hold_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.hold();
        assert!(!w.wrapping);
    }

    #[test]
    fn release_clears_wrapping() {
        let mut w = w();
        w.hold();
        w.release();
        assert!(!w.wrapping);
    }

    #[test]
    fn release_no_op_when_not_wrapping() {
        let mut w = w();
        w.release(); // no error
        assert!(!w.wrapping);
    }

    #[test]
    fn tick_tightens_when_wrapping() {
        let mut w = w(); // tighten_rate = 5.0
        w.hold();
        w.tick(1.0); // 5.0 * 1.0 = 5.0
        assert!((w.wrap_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max_wrap() {
        let mut w = w();
        w.hold();
        w.tick(10.0); // 5.0 * 10.0 = 50 → capped at 10
        assert!((w.wrap_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_gripped_on_first_max() {
        let mut w = w();
        w.hold();
        w.tick(10.0); // pushes to max
        assert!(w.just_gripped);
    }

    #[test]
    fn tick_no_just_gripped_when_already_at_max() {
        let mut w = w();
        w.hold();
        w.tick(10.0); // just_gripped fires
        w.tick(0.016); // clears; already at max, no re-fire
        assert!(!w.just_gripped);
    }

    #[test]
    fn tick_no_just_gripped_when_below_max() {
        let mut w = w();
        w.hold();
        w.tick(0.1); // 5.0 * 0.1 = 0.5, below max
        assert!(!w.just_gripped);
    }

    #[test]
    fn tick_loosens_when_not_wrapping() {
        let mut w = w(); // loosen_rate = 2.0
        w.hold();
        w.tick(2.0); // 10.0
        w.tick(0.016); // clear just_gripped
        w.release();
        w.tick(1.0); // 10.0 - 2.0 = 8.0
        assert!((w.wrap_level - 8.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_at_zero_when_loosening() {
        let mut w = Wrap::new(10.0, 5.0, 20.0, 0.5);
        w.hold();
        w.tick(0.5); // 2.5
        w.release();
        w.tick(1.0); // 20.0 * 1.0 → floors at 0
        assert_eq!(w.wrap_level, 0.0);
    }

    #[test]
    fn tick_fires_just_freed_when_loosen_reaches_zero() {
        let mut w = Wrap::new(10.0, 5.0, 20.0, 0.5);
        w.hold();
        w.tick(0.5); // 2.5
        w.tick(0.016); // clear flags
        w.release();
        w.tick(1.0); // loosen to 0
        assert!(w.just_freed);
        assert!(!w.is_gripped());
    }

    #[test]
    fn tick_no_just_freed_when_level_remains() {
        let mut w = Wrap::new(10.0, 5.0, 1.0, 0.5);
        w.hold();
        w.tick(2.0); // 10.0
        w.tick(0.016);
        w.release();
        w.tick(1.0); // 10.0 - 1.0 = 9.0, still gripped
        assert!(!w.just_freed);
    }

    #[test]
    fn tick_clears_just_gripped() {
        let mut w = w();
        w.hold();
        w.tick(10.0); // just_gripped
        w.tick(0.016); // cleared
        assert!(!w.just_gripped);
    }

    #[test]
    fn tick_clears_just_freed() {
        let mut w = Wrap::new(10.0, 5.0, 20.0, 0.5);
        w.hold();
        w.tick(0.5);
        w.release();
        w.tick(1.0); // just_freed
        w.tick(0.016); // cleared
        assert!(!w.just_freed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.hold();
        w.enabled = false;
        w.tick(5.0); // should not tighten
        assert_eq!(w.wrap_level, 0.0);
    }

    #[test]
    fn tick_no_loosen_when_disabled() {
        let mut w = Wrap::new(10.0, 5.0, 2.0, 0.5);
        w.hold();
        w.enabled = true;
        w.tick(2.0); // 10.0
        w.release();
        w.enabled = false;
        w.tick(5.0); // should not loosen
        assert!((w.wrap_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_tighten_when_not_wrapping() {
        let mut w = w();
        w.tick(5.0); // not wrapping, wrap_level stays 0
        assert_eq!(w.wrap_level, 0.0);
    }

    #[test]
    fn is_gripped_true_when_positive() {
        let mut w = w();
        w.hold();
        w.tick(0.1);
        assert!(w.is_gripped());
    }

    #[test]
    fn is_gripped_false_when_zero() {
        let w = w();
        assert!(!w.is_gripped());
    }

    #[test]
    fn is_gripped_false_when_disabled() {
        let mut w = w();
        w.hold();
        w.tick(1.0);
        w.enabled = false;
        assert!(!w.is_gripped());
    }

    #[test]
    fn wrap_fraction_zero_when_free() {
        let w = w();
        assert_eq!(w.wrap_fraction(), 0.0);
    }

    #[test]
    fn wrap_fraction_half_at_midpoint() {
        let mut w = w();
        w.hold();
        w.tick(1.0); // 5.0 / 10.0 = 0.5
        assert!((w.wrap_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn wrap_fraction_one_at_max() {
        let mut w = w();
        w.hold();
        w.tick(10.0);
        assert!((w.wrap_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_full_when_free() {
        let w = Wrap::new(10.0, 5.0, 2.0, 0.5);
        assert!((w.effective_speed(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_half_at_full_grip_with_0_5_penalty() {
        let mut w = Wrap::new(10.0, 5.0, 2.0, 0.5);
        w.hold();
        w.tick(10.0); // max
        assert!((w.effective_speed(100.0) - 50.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_zero_at_full_grip_with_1_0_penalty() {
        let mut w = Wrap::new(10.0, 5.0, 2.0, 1.0);
        w.hold();
        w.tick(10.0);
        assert!((w.effective_speed(100.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_partial_at_half_grip() {
        let mut w = Wrap::new(10.0, 5.0, 2.0, 0.5);
        w.hold();
        w.tick(1.0); // 5.0 → fraction 0.5
                     // 100 * (1 - 0.5*0.5) = 75
        assert!((w.effective_speed(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_passthrough_when_disabled() {
        let mut w = Wrap::new(10.0, 5.0, 2.0, 1.0);
        w.hold();
        w.tick(10.0);
        w.enabled = false;
        assert!((w.effective_speed(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_floored_at_zero() {
        let mut w = Wrap::new(10.0, 5.0, 2.0, 1.0);
        w.hold();
        w.tick(10.0);
        assert!(w.effective_speed(100.0) >= 0.0);
    }

    #[test]
    fn max_wrap_clamped_to_one() {
        let w = Wrap::new(0.0, 5.0, 2.0, 0.5);
        assert!((w.max_wrap - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tighten_rate_clamped_to_zero() {
        let w = Wrap::new(10.0, -5.0, 2.0, 0.5);
        assert_eq!(w.tighten_rate, 0.0);
    }

    #[test]
    fn loosen_rate_clamped_to_zero() {
        let w = Wrap::new(10.0, 5.0, -2.0, 0.5);
        assert_eq!(w.loosen_rate, 0.0);
    }

    #[test]
    fn mobility_penalty_clamped_high() {
        let w = Wrap::new(10.0, 5.0, 2.0, 2.0);
        assert!((w.mobility_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn mobility_penalty_clamped_low() {
        let w = Wrap::new(10.0, 5.0, 2.0, -1.0);
        assert_eq!(w.mobility_penalty, 0.0);
    }

    #[test]
    fn hold_release_hold_cycle() {
        let mut w = w();
        w.hold();
        w.tick(1.0); // 5.0
        w.tick(0.016);
        w.release();
        w.tick(1.0); // 5.0 - 2.0 = 3.0
        w.hold();
        w.tick(1.0); // 3.0 + 5.0 = 8.0
        assert!((w.wrap_level - 8.0).abs() < 1e-4);
    }
}
