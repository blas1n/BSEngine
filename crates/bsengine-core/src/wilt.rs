use bevy_ecs::prelude::Component;

/// Sustained-strain fatigue tracker. `wilt_level` builds while the entity is
/// actively straining and recovers passively when it rests. At full wilt the
/// entity's output is reduced by up to `output_penalty`. Systems call
/// `strain_on()` when the entity begins over-exerting and `strain_off()` when
/// it stops; `tick()` advances the fatigue or recovery each frame.
///
/// `strain_on()` begins accumulating wilt. No-op when already straining or
/// disabled.
///
/// `strain_off()` stops accumulating. No-op when not straining.
///
/// `tick(dt)` clears `just_wilted` and `just_recovered` first; then when
/// straining adds `strain_rate * dt` (capped at `max_wilt`) and fires
/// `just_wilted` on first reach; when resting subtracts `recovery_rate * dt`
/// (floored at 0.0) and fires `just_recovered` when reaching 0.0. No-op when
/// disabled.
///
/// `is_wilted()` returns `wilt_level >= max_wilt && enabled`.
///
/// `wilt_fraction()` returns `(wilt_level / max_wilt).clamp(0.0, 1.0)`.
///
/// `effective_output(base)` returns
/// `(base * (1.0 - output_penalty * wilt_fraction())).max(0.0)` when enabled;
/// returns `base` when disabled.
///
/// Distinct from `Exhaustion` (total energy depletion that blocks actions),
/// `Wear` (equipment degradation), `Burn`/`Overheat` (heat-based damage), and
/// `Stamina` (regenerating action resource): Wilt models **continuous
/// over-exertion fatigue** — a degrading output multiplier that eases when the
/// entity rests between bursts.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wilt {
    /// Current fatigue level [0.0, max_wilt].
    pub wilt_level: f32,
    /// Maximum fatigue before no further accumulation occurs. Clamped >= 1.0.
    pub max_wilt: f32,
    /// Fatigue gain per second while straining. Clamped >= 0.0.
    pub strain_rate: f32,
    /// Fatigue loss per second while resting. Clamped >= 0.0.
    pub recovery_rate: f32,
    /// Maximum fractional output reduction at full wilt. Clamped [0.0, 1.0].
    pub output_penalty: f32,
    pub straining: bool,
    pub just_wilted: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Wilt {
    pub fn new(max_wilt: f32, strain_rate: f32, recovery_rate: f32, output_penalty: f32) -> Self {
        Self {
            wilt_level: 0.0,
            max_wilt: max_wilt.max(1.0),
            strain_rate: strain_rate.max(0.0),
            recovery_rate: recovery_rate.max(0.0),
            output_penalty: output_penalty.clamp(0.0, 1.0),
            straining: false,
            just_wilted: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Begin accumulating wilt. No-op when already straining or disabled.
    pub fn strain_on(&mut self) {
        if !self.enabled || self.straining {
            return;
        }
        self.straining = true;
    }

    /// Stop accumulating wilt. No-op when not straining.
    pub fn strain_off(&mut self) {
        if !self.straining {
            return;
        }
        self.straining = false;
    }

    /// Advance one frame: clear flags; build or recover; fire `just_wilted` /
    /// `just_recovered` at boundaries. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_wilted = false;
        self.just_recovered = false;

        if !self.enabled {
            return;
        }

        if self.straining {
            let was_below = self.wilt_level < self.max_wilt;
            self.wilt_level = (self.wilt_level + self.strain_rate * dt).min(self.max_wilt);
            if was_below && self.wilt_level >= self.max_wilt {
                self.just_wilted = true;
            }
        } else if self.wilt_level > 0.0 && self.recovery_rate > 0.0 {
            let was_positive = self.wilt_level > 0.0;
            self.wilt_level = (self.wilt_level - self.recovery_rate * dt).max(0.0);
            if was_positive && self.wilt_level == 0.0 {
                self.just_recovered = true;
            }
        }
    }

    /// `true` when fatigue has fully accumulated and the component is enabled.
    pub fn is_wilted(&self) -> bool {
        self.wilt_level >= self.max_wilt && self.enabled
    }

    /// Fatigue as a fraction of the maximum [0.0, 1.0].
    pub fn wilt_fraction(&self) -> f32 {
        (self.wilt_level / self.max_wilt).clamp(0.0, 1.0)
    }

    /// Output reduced by current wilt. Returns
    /// `(base * (1.0 - output_penalty * wilt_fraction())).max(0.0)` when
    /// enabled; `base` when disabled.
    pub fn effective_output(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.output_penalty * self.wilt_fraction())).max(0.0)
    }
}

impl Default for Wilt {
    fn default() -> Self {
        Self::new(10.0, 2.0, 1.0, 0.6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_fresh() {
        let w = Wilt::new(10.0, 2.0, 1.0, 0.6);
        assert_eq!(w.wilt_level, 0.0);
        assert!(!w.straining);
        assert!(!w.is_wilted());
    }

    #[test]
    fn strain_on_sets_straining() {
        let mut w = Wilt::new(10.0, 2.0, 1.0, 0.6);
        w.strain_on();
        assert!(w.straining);
    }

    #[test]
    fn strain_on_no_op_when_already_straining() {
        let mut w = Wilt::new(10.0, 2.0, 1.0, 0.6);
        w.strain_on();
        w.wilt_level = 5.0;
        w.strain_on();
        assert_eq!(w.wilt_level, 5.0);
    }

    #[test]
    fn strain_on_no_op_when_disabled() {
        let mut w = Wilt::new(10.0, 2.0, 1.0, 0.6);
        w.enabled = false;
        w.strain_on();
        assert!(!w.straining);
    }

    #[test]
    fn strain_off_clears_straining() {
        let mut w = Wilt::new(10.0, 2.0, 1.0, 0.6);
        w.strain_on();
        w.strain_off();
        assert!(!w.straining);
    }

    #[test]
    fn strain_off_no_op_when_not_straining() {
        let mut w = Wilt::new(10.0, 2.0, 1.0, 0.6);
        w.wilt_level = 3.0;
        w.strain_off();
        assert_eq!(w.wilt_level, 3.0);
        assert!(!w.straining);
    }

    #[test]
    fn tick_builds_when_straining() {
        let mut w = Wilt::new(10.0, 3.0, 0.0, 0.6);
        w.strain_on();
        w.tick(1.0);
        assert!((w.wilt_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_max_wilt() {
        let mut w = Wilt::new(10.0, 20.0, 0.0, 0.6);
        w.strain_on();
        w.tick(1.0);
        assert!((w.wilt_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_wilted_on_first_reach() {
        let mut w = Wilt::new(10.0, 10.0, 0.0, 0.6);
        w.strain_on();
        w.tick(1.0);
        assert!(w.just_wilted);
        assert!(w.is_wilted());
    }

    #[test]
    fn tick_no_just_wilted_when_already_at_max() {
        let mut w = Wilt::new(10.0, 10.0, 0.0, 0.6);
        w.strain_on();
        w.tick(1.0); // just_wilted fires
        w.tick(1.0); // already wilted, flag cleared
        assert!(!w.just_wilted);
    }

    #[test]
    fn tick_recovers_when_resting() {
        let mut w = Wilt::new(10.0, 0.0, 3.0, 0.6);
        w.wilt_level = 6.0;
        w.tick(1.0); // -3.0 → 3.0
        assert!((w.wilt_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_floors_recovery_at_zero() {
        let mut w = Wilt::new(10.0, 0.0, 10.0, 0.6);
        w.wilt_level = 2.0;
        w.tick(1.0);
        assert_eq!(w.wilt_level, 0.0);
    }

    #[test]
    fn tick_fires_just_recovered_when_reaching_zero() {
        let mut w = Wilt::new(10.0, 0.0, 5.0, 0.6);
        w.wilt_level = 2.0;
        w.tick(1.0); // -5 → 0, fires just_recovered
        assert!(w.just_recovered);
    }

    #[test]
    fn tick_no_just_recovered_when_not_at_zero() {
        let mut w = Wilt::new(10.0, 0.0, 1.0, 0.6);
        w.wilt_level = 5.0;
        w.tick(1.0); // 5 - 1 = 4, not zero
        assert!(!w.just_recovered);
    }

    #[test]
    fn tick_clears_just_wilted() {
        let mut w = Wilt::new(10.0, 10.0, 0.0, 0.6);
        w.strain_on();
        w.tick(1.0); // just_wilted = true
        w.tick(0.016);
        assert!(!w.just_wilted);
    }

    #[test]
    fn tick_clears_just_recovered() {
        let mut w = Wilt::new(10.0, 0.0, 10.0, 0.6);
        w.wilt_level = 2.0;
        w.tick(1.0); // just_recovered = true
        w.tick(0.016);
        assert!(!w.just_recovered);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = Wilt::new(10.0, 3.0, 0.0, 0.6);
        w.strain_on();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.wilt_level, 0.0);
    }

    #[test]
    fn tick_no_recovery_when_straining() {
        let mut w = Wilt::new(10.0, 2.0, 5.0, 0.6);
        w.wilt_level = 6.0;
        w.strain_on();
        w.tick(1.0); // builds, does not recover
        assert!((w.wilt_level - 8.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_change_when_resting_at_zero() {
        let mut w = Wilt::new(10.0, 0.0, 2.0, 0.6);
        w.tick(1.0); // no wilt, no recovery needed
        assert_eq!(w.wilt_level, 0.0);
        assert!(!w.just_recovered);
    }

    #[test]
    fn is_wilted_true_at_max() {
        let mut w = Wilt::new(10.0, 10.0, 0.0, 0.6);
        w.strain_on();
        w.tick(1.0);
        assert!(w.is_wilted());
    }

    #[test]
    fn is_wilted_false_below_max() {
        let w = Wilt::new(10.0, 2.0, 0.0, 0.6);
        assert!(!w.is_wilted());
    }

    #[test]
    fn is_wilted_false_when_disabled() {
        let mut w = Wilt::new(10.0, 0.0, 0.0, 0.6);
        w.wilt_level = 10.0;
        w.enabled = false;
        assert!(!w.is_wilted());
    }

    #[test]
    fn wilt_fraction_zero_at_start() {
        let w = Wilt::new(10.0, 2.0, 0.0, 0.6);
        assert_eq!(w.wilt_fraction(), 0.0);
    }

    #[test]
    fn wilt_fraction_half_at_midpoint() {
        let mut w = Wilt::new(10.0, 0.0, 0.0, 0.6);
        w.wilt_level = 5.0;
        assert!((w.wilt_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn wilt_fraction_one_at_max() {
        let mut w = Wilt::new(10.0, 0.0, 0.0, 0.6);
        w.wilt_level = 10.0;
        assert!((w.wilt_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_full_when_fresh() {
        let w = Wilt::new(10.0, 0.0, 0.0, 0.6);
        assert!((w.effective_output(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_at_half_wilt() {
        let mut w = Wilt::new(10.0, 0.0, 0.0, 0.4);
        w.wilt_level = 5.0; // 50% wilt
                            // 100 * (1 - 0.4 * 0.5) = 80
        assert!((w.effective_output(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_at_full_wilt() {
        let mut w = Wilt::new(10.0, 0.0, 0.0, 0.6);
        w.wilt_level = 10.0; // 100% wilt
                             // 100 * (1 - 0.6 * 1.0) = 40
        assert!((w.effective_output(100.0) - 40.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_floored_at_zero() {
        let mut w = Wilt::new(10.0, 0.0, 0.0, 1.0);
        w.wilt_level = 10.0; // full penalty
        assert_eq!(w.effective_output(100.0), 0.0);
    }

    #[test]
    fn effective_output_base_when_disabled() {
        let mut w = Wilt::new(10.0, 0.0, 0.0, 0.6);
        w.wilt_level = 10.0;
        w.enabled = false;
        assert!((w.effective_output(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn max_wilt_clamped_to_one() {
        let w = Wilt::new(0.0, 2.0, 1.0, 0.6);
        assert!((w.max_wilt - 1.0).abs() < 1e-5);
    }

    #[test]
    fn strain_rate_clamped_to_zero() {
        let w = Wilt::new(10.0, -1.0, 1.0, 0.6);
        assert_eq!(w.strain_rate, 0.0);
    }

    #[test]
    fn recovery_rate_clamped_to_zero() {
        let w = Wilt::new(10.0, 2.0, -1.0, 0.6);
        assert_eq!(w.recovery_rate, 0.0);
    }

    #[test]
    fn output_penalty_clamped_to_one() {
        let w = Wilt::new(10.0, 2.0, 1.0, 2.0);
        assert!((w.output_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn output_penalty_clamped_to_zero() {
        let w = Wilt::new(10.0, 2.0, 1.0, -0.5);
        assert_eq!(w.output_penalty, 0.0);
    }

    #[test]
    fn strain_recover_cycle() {
        let mut w = Wilt::new(10.0, 5.0, 3.0, 0.6);
        w.strain_on();
        w.tick(1.0); // level = 5.0
        w.strain_off();
        w.tick(1.0); // -3.0 → 2.0
        assert!((w.wilt_level - 2.0).abs() < 1e-4);
        assert!(!w.straining);
    }

    #[test]
    fn full_cycle_wilts_then_fully_recovers() {
        let mut w = Wilt::new(10.0, 10.0, 10.0, 0.6);
        w.strain_on();
        w.tick(1.0); // just_wilted, level = 10
        assert!(w.just_wilted);
        w.strain_off();
        w.tick(1.0); // level = 0, just_recovered
        assert!(w.just_recovered);
        assert!(!w.is_wilted());
    }

    #[test]
    fn zero_strain_rate_never_wilts() {
        let mut w = Wilt::new(10.0, 0.0, 0.0, 0.6);
        w.strain_on();
        w.tick(100.0);
        assert!(!w.is_wilted());
        assert_eq!(w.wilt_level, 0.0);
    }
}
