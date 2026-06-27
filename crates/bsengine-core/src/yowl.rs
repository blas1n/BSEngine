use bevy_ecs::prelude::Component;

/// Pain-reactive distress accumulator. Models the build-up of involuntary
/// distress from taking damage; the louder the accumulated pain, the stronger
/// the entity's desperate cry effect.
///
/// `hurt(amount)` adds `amount` to `yowl_level` (capped at `max_yowl`); fires
/// `just_yowled` the first time the level reaches the cap in one damage burst.
/// No-op when disabled.
///
/// `tick(dt)` clears `just_yowled` first, then decays `yowl_level` by
/// `recovery_rate * dt` (floored at 0). No-op (beyond flag clear) when
/// disabled.
///
/// `is_distressed()` returns `yowl_level >= max_yowl && enabled`.
///
/// `pain_fraction()` returns `(yowl_level / max_yowl).clamp(0.0, 1.0)`.
///
/// `effective_cry(base)` returns `base * (1.0 + pain_fraction())` when
/// enabled — cry scales continuously with accumulated pain; returns `base`
/// unchanged when disabled.
///
/// Unlike `Whelm` (explicit surge/subside) or `Wrest` (explicit strain/ease),
/// Yowl has **no start-state verb** — pain is pushed in from external damage
/// events via `hurt()` and the level decays passively over time. The entity
/// cannot choose to yowl; it reacts to what was done to it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yowl {
    /// Accumulated distress level [0.0, max_yowl].
    pub yowl_level: f32,
    /// Maximum distress level. Clamped >= 1.0.
    pub max_yowl: f32,
    /// Distress decay per second. Clamped >= 0.0.
    pub recovery_rate: f32,
    pub just_yowled: bool,
    pub enabled: bool,
}

impl Yowl {
    pub fn new(max_yowl: f32, recovery_rate: f32) -> Self {
        Self {
            yowl_level: 0.0,
            max_yowl: max_yowl.max(1.0),
            recovery_rate: recovery_rate.max(0.0),
            just_yowled: false,
            enabled: true,
        }
    }

    /// Push pain into the accumulator. Fires `just_yowled` the first time
    /// `yowl_level` reaches `max_yowl`. No-op when disabled.
    pub fn hurt(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        let was_below = self.yowl_level < self.max_yowl;
        self.yowl_level = (self.yowl_level + amount).min(self.max_yowl);
        if was_below && self.yowl_level >= self.max_yowl {
            self.just_yowled = true;
        }
    }

    /// Advance one frame: clear flag, then decay distress.
    /// No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_yowled = false;

        if !self.enabled {
            return;
        }

        if self.yowl_level > 0.0 {
            self.yowl_level = (self.yowl_level - self.recovery_rate * dt).max(0.0);
        }
    }

    /// `true` when distress is at maximum and component is enabled.
    pub fn is_distressed(&self) -> bool {
        self.yowl_level >= self.max_yowl && self.enabled
    }

    /// Distress as a fraction of maximum [0.0, 1.0].
    pub fn pain_fraction(&self) -> f32 {
        (self.yowl_level / self.max_yowl).clamp(0.0, 1.0)
    }

    /// Scale `base` by accumulated distress. Returns
    /// `base * (1.0 + pain_fraction())` when enabled; `base` otherwise.
    pub fn effective_cry(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.pain_fraction())
    }
}

impl Default for Yowl {
    fn default() -> Self {
        Self::new(10.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yowl {
        Yowl::new(10.0, 2.0)
    }

    #[test]
    fn new_starts_calm() {
        let y = y();
        assert_eq!(y.yowl_level, 0.0);
        assert!(!y.just_yowled);
        assert!(!y.is_distressed());
    }

    #[test]
    fn hurt_increases_level() {
        let mut y = y();
        y.hurt(3.0);
        assert!((y.yowl_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn hurt_caps_at_max() {
        let mut y = y();
        y.hurt(100.0);
        assert!((y.yowl_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn hurt_fires_just_yowled_at_max() {
        let mut y = y();
        y.hurt(10.0);
        assert!(y.just_yowled);
    }

    #[test]
    fn hurt_fires_just_yowled_crossing_max() {
        let mut y = y();
        y.hurt(7.0); // 7.0
        y.hurt(5.0); // crosses max
        assert!(y.just_yowled);
    }

    #[test]
    fn hurt_no_re_fire_when_already_at_max() {
        let mut y = y();
        y.hurt(10.0); // at max, fires
        y.just_yowled = false; // simulate tick clearing it
        y.hurt(3.0); // already at max, no re-fire
        assert!(!y.just_yowled);
        assert!((y.yowl_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn hurt_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.hurt(10.0);
        assert_eq!(y.yowl_level, 0.0);
        assert!(!y.just_yowled);
    }

    #[test]
    fn tick_clears_just_yowled() {
        let mut y = y();
        y.hurt(10.0);
        y.tick(0.016);
        assert!(!y.just_yowled);
    }

    #[test]
    fn tick_decays_level() {
        let mut y = y(); // recovery_rate=2.0
        y.hurt(6.0); // 6.0
        y.tick(1.0); // 6.0 - 2.0 = 4.0
        assert!((y.yowl_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut y = y();
        y.hurt(3.0);
        y.tick(100.0); // floors at 0
        assert_eq!(y.yowl_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled_no_decay() {
        let mut y = y();
        y.hurt(6.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.yowl_level - 6.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clears_flag_even_when_disabled() {
        let mut y = y();
        y.just_yowled = true;
        y.enabled = false;
        y.tick(0.016);
        assert!(!y.just_yowled);
    }

    #[test]
    fn is_distressed_true_at_max() {
        let mut y = y();
        y.hurt(10.0);
        assert!(y.is_distressed());
    }

    #[test]
    fn is_distressed_false_below_max() {
        let mut y = y();
        y.hurt(5.0);
        assert!(!y.is_distressed());
    }

    #[test]
    fn is_distressed_false_when_disabled() {
        let mut y = y();
        y.hurt(10.0);
        y.enabled = false;
        assert!(!y.is_distressed());
    }

    #[test]
    fn is_distressed_false_after_recovery() {
        let mut y = y();
        y.hurt(10.0);
        y.tick(100.0);
        assert!(!y.is_distressed());
    }

    #[test]
    fn pain_fraction_zero_when_calm() {
        let y = y();
        assert_eq!(y.pain_fraction(), 0.0);
    }

    #[test]
    fn pain_fraction_half_at_midpoint() {
        let mut y = y();
        y.hurt(5.0);
        assert!((y.pain_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn pain_fraction_one_at_max() {
        let mut y = y();
        y.hurt(10.0);
        assert!((y.pain_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_cry_base_when_calm() {
        let y = y();
        assert!((y.effective_cry(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_cry_scaled_at_half_pain() {
        let mut y = y();
        y.hurt(5.0); // fraction = 0.5
                     // 100 * (1 + 0.5) = 150
        assert!((y.effective_cry(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_cry_doubled_at_full_pain() {
        let mut y = y();
        y.hurt(10.0); // fraction = 1.0
                      // 100 * (1 + 1.0) = 200
        assert!((y.effective_cry(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_cry_passthrough_when_disabled() {
        let mut y = y();
        y.hurt(10.0);
        y.enabled = false;
        assert!((y.effective_cry(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_yowl_clamped_to_one() {
        let y = Yowl::new(0.0, 2.0);
        assert!((y.max_yowl - 1.0).abs() < 1e-5);
    }

    #[test]
    fn recovery_rate_clamped_to_zero() {
        let y = Yowl::new(10.0, -3.0);
        assert_eq!(y.recovery_rate, 0.0);
    }

    #[test]
    fn hurt_then_recover_then_hurt_cycle() {
        let mut y = y(); // recovery_rate=2.0
        y.hurt(10.0); // max
        y.tick(5.0); // fully recovered (0.0)
        assert_eq!(y.yowl_level, 0.0);
        y.hurt(10.0); // back to max — should fire just_yowled again
        assert!(y.just_yowled);
    }

    #[test]
    fn incremental_hurt_accumulates_correctly() {
        let mut y = y();
        y.hurt(2.0);
        y.hurt(3.0);
        y.hurt(4.0);
        assert!((y.yowl_level - 9.0).abs() < 1e-4);
    }

    #[test]
    fn just_yowled_not_set_on_partial_hurt() {
        let mut y = y();
        y.hurt(7.0); // partial
        assert!(!y.just_yowled);
    }
}
