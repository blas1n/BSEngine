use bevy_ecs::prelude::Component;

/// Rhythmic evasion flow that builds while the entity bobs and weaves and
/// fades once the pattern breaks. Scales incoming-damage reduction
/// proportionally to current flow, rewarding sustained evasive movement.
///
/// `begin()` starts the evasion pattern (`weaving = true`). No-op when
/// already weaving or disabled.
///
/// `halt()` stops the evasion pattern (`weaving = false`). No-op when not
/// weaving.
///
/// `tick(dt)` clears one-frame flags, then:
/// - when `weaving`: increases `weave_level` by `buildup_rate * dt` (capped
///   at `max_weave`), firing `just_peaked` on first reach of `max_weave`;
/// - when `!weaving`: decreases `weave_level` by `falloff_rate * dt` (floored
///   0), firing `just_broken` on first drop to 0 from positive.
/// No-op when disabled.
///
/// `is_evading()` returns `weave_level >= max_weave && enabled`.
///
/// `weave_fraction()` returns `(weave_level / max_weave).clamp(0.0, 1.0)`.
///
/// `effective_damage(incoming)` returns
/// `incoming * (1.0 - dodge_bonus * weave_fraction()).max(0.0)` when enabled;
/// returns `incoming` unchanged otherwise.
///
/// Distinct from `Dodge` (binary one-shot full-avoidance triggered by input),
/// `Deflect` (projectile angle redirect), `Parry` (melee counter-window), and
/// `Evade` (stamina-gated dash out of harm): Weave models a **sustained
/// rhythmic evasion flow** — the longer the entity maintains the pattern the
/// more effective the mitigation, until the pattern breaks and the flow drains.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weave {
    /// Current evasion flow [0.0, max_weave].
    pub weave_level: f32,
    /// Maximum evasion flow. Clamped >= 1.0.
    pub max_weave: f32,
    /// Flow gained per second while actively weaving. Clamped >= 0.0.
    pub buildup_rate: f32,
    /// Flow lost per second when pattern breaks. Clamped >= 0.0.
    pub falloff_rate: f32,
    /// Damage reduction at full evasion flow [0.0, 1.0]. Clamped.
    pub dodge_bonus: f32,
    /// Whether the entity is actively in the evasion pattern.
    pub weaving: bool,
    pub just_peaked: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Weave {
    pub fn new(max_weave: f32, buildup_rate: f32, falloff_rate: f32, dodge_bonus: f32) -> Self {
        Self {
            weave_level: 0.0,
            max_weave: max_weave.max(1.0),
            buildup_rate: buildup_rate.max(0.0),
            falloff_rate: falloff_rate.max(0.0),
            dodge_bonus: dodge_bonus.clamp(0.0, 1.0),
            weaving: false,
            just_peaked: false,
            just_broken: false,
            enabled: true,
        }
    }

    /// Start the evasion pattern. No-op when already weaving or disabled.
    pub fn begin(&mut self) {
        if !self.enabled || self.weaving {
            return;
        }
        self.weaving = true;
    }

    /// Stop the evasion pattern. No-op when not weaving.
    pub fn halt(&mut self) {
        if !self.weaving {
            return;
        }
        self.weaving = false;
    }

    /// Advance one frame: clear flags, then build or drain evasion flow. No-op
    /// when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_broken = false;

        if !self.enabled {
            return;
        }
        if self.weaving {
            let was_below_max = self.weave_level < self.max_weave;
            self.weave_level = (self.weave_level + self.buildup_rate * dt).min(self.max_weave);
            if was_below_max && self.weave_level >= self.max_weave {
                self.just_peaked = true;
            }
        } else if self.weave_level > 0.0 {
            let was_positive = self.weave_level > 0.0;
            self.weave_level = (self.weave_level - self.falloff_rate * dt).max(0.0);
            if was_positive && self.weave_level == 0.0 {
                self.just_broken = true;
            }
        }
    }

    /// `true` when evasion flow is at maximum and the component is enabled.
    pub fn is_evading(&self) -> bool {
        self.weave_level >= self.max_weave && self.enabled
    }

    /// Evasion flow as a fraction of maximum [0.0, 1.0].
    pub fn weave_fraction(&self) -> f32 {
        (self.weave_level / self.max_weave).clamp(0.0, 1.0)
    }

    /// Reduce `incoming` damage by current evasion flow. Returns
    /// `incoming * (1 - dodge_bonus * fraction).max(0)` when enabled;
    /// `incoming` otherwise.
    pub fn effective_damage(&self, incoming: f32) -> f32 {
        if !self.enabled {
            return incoming;
        }
        (incoming * (1.0 - self.dodge_bonus * self.weave_fraction())).max(0.0)
    }
}

impl Default for Weave {
    fn default() -> Self {
        Self::new(10.0, 4.0, 2.0, 0.6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Weave {
        Weave::new(10.0, 5.0, 2.0, 0.8)
    }

    #[test]
    fn new_starts_still() {
        let w = Weave::new(10.0, 5.0, 2.0, 0.8);
        assert_eq!(w.weave_level, 0.0);
        assert!(!w.weaving);
        assert!(!w.just_peaked);
        assert!(!w.just_broken);
        assert!(!w.is_evading());
    }

    #[test]
    fn begin_sets_weaving() {
        let mut w = w();
        w.begin();
        assert!(w.weaving);
    }

    #[test]
    fn begin_no_op_when_already_weaving() {
        let mut w = w();
        w.begin();
        w.begin();
        assert!(w.weaving);
    }

    #[test]
    fn begin_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.begin();
        assert!(!w.weaving);
    }

    #[test]
    fn halt_clears_weaving() {
        let mut w = w();
        w.begin();
        w.halt();
        assert!(!w.weaving);
    }

    #[test]
    fn halt_no_op_when_not_weaving() {
        let mut w = w();
        w.halt();
        assert!(!w.weaving);
    }

    #[test]
    fn tick_builds_when_weaving() {
        let mut w = w(); // buildup_rate = 5.0
        w.begin();
        w.tick(1.0); // 5.0 * 1.0 = 5.0
        assert!((w.weave_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max() {
        let mut w = w();
        w.begin();
        w.tick(10.0); // 5.0 * 10 → capped at 10
        assert!((w.weave_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_peaked_on_first_max() {
        let mut w = w();
        w.begin();
        w.tick(10.0);
        assert!(w.just_peaked);
    }

    #[test]
    fn tick_no_just_peaked_when_already_at_max() {
        let mut w = w();
        w.begin();
        w.tick(10.0); // just_peaked fires
        w.tick(0.016); // cleared; at max, no re-fire
        assert!(!w.just_peaked);
    }

    #[test]
    fn tick_no_just_peaked_below_max() {
        let mut w = w();
        w.begin();
        w.tick(1.0); // 5.0, below max
        assert!(!w.just_peaked);
    }

    #[test]
    fn tick_drains_when_not_weaving() {
        let mut w = w(); // falloff_rate = 2.0
        w.begin();
        w.tick(2.0); // 10.0
        w.halt();
        w.tick(1.0); // 10.0 - 2.0 = 8.0
        assert!((w.weave_level - 8.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_at_zero_when_draining() {
        let mut w = w();
        w.begin();
        w.tick(1.0); // 5.0
        w.halt();
        w.tick(10.0); // 5.0 - 20.0 → floored 0
        assert_eq!(w.weave_level, 0.0);
    }

    #[test]
    fn tick_fires_just_broken_on_first_zero() {
        let mut w = w();
        w.begin();
        w.tick(1.0); // 5.0
        w.halt();
        w.tick(10.0); // drops to 0
        assert!(w.just_broken);
    }

    #[test]
    fn tick_no_just_broken_when_already_at_zero() {
        let mut w = w();
        w.tick(1.0); // level=0, !weaving → no change
        assert!(!w.just_broken);
    }

    #[test]
    fn tick_no_change_when_stopped_at_zero() {
        let mut w = w();
        w.tick(5.0);
        assert_eq!(w.weave_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = Weave::new(10.0, 5.0, 2.0, 0.8);
        w.enabled = false;
        w.weaving = true; // bypass begin() guard
        w.tick(5.0);
        assert_eq!(w.weave_level, 0.0);
    }

    #[test]
    fn tick_clears_flags() {
        let mut w = w();
        w.begin();
        w.tick(10.0); // just_peaked
        w.tick(0.016); // cleared
        assert!(!w.just_peaked);
    }

    #[test]
    fn is_evading_true_at_max() {
        let mut w = w();
        w.begin();
        w.tick(10.0);
        assert!(w.is_evading());
    }

    #[test]
    fn is_evading_false_below_max() {
        let mut w = w();
        w.begin();
        w.tick(1.0); // 5.0 < 10.0
        assert!(!w.is_evading());
    }

    #[test]
    fn is_evading_false_when_disabled() {
        let mut w = w();
        w.begin();
        w.tick(10.0);
        w.enabled = false;
        assert!(!w.is_evading());
    }

    #[test]
    fn weave_fraction_zero_when_empty() {
        let w = w();
        assert_eq!(w.weave_fraction(), 0.0);
    }

    #[test]
    fn weave_fraction_half_at_midpoint() {
        let mut w = w();
        w.begin();
        w.tick(1.0); // 5.0 / 10.0 = 0.5
        assert!((w.weave_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn weave_fraction_one_at_max() {
        let mut w = w();
        w.begin();
        w.tick(10.0);
        assert!((w.weave_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_damage_full_when_no_flow() {
        let w = w(); // dodge_bonus=0.8, fraction=0
        assert!((w.effective_damage(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_damage_reduced_at_half_flow() {
        let mut w = Weave::new(10.0, 5.0, 2.0, 0.8);
        w.begin();
        w.tick(1.0); // 5.0 → fraction 0.5
                     // 100 * (1 - 0.8*0.5) = 100 * 0.6 = 60
        assert!((w.effective_damage(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_minimised_at_full_flow() {
        let mut w = Weave::new(10.0, 5.0, 2.0, 0.8);
        w.begin();
        w.tick(10.0); // max, fraction=1.0
                      // 100 * (1 - 0.8) = 20
        assert!((w.effective_damage(100.0) - 20.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_floors_at_zero() {
        let mut w = Weave::new(10.0, 5.0, 2.0, 1.0); // full dodge
        w.begin();
        w.tick(10.0);
        assert!((w.effective_damage(100.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn effective_damage_passthrough_when_disabled() {
        let mut w = w();
        w.begin();
        w.tick(10.0);
        w.enabled = false;
        assert!((w.effective_damage(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_weave_clamped_to_one() {
        let w = Weave::new(0.0, 5.0, 2.0, 0.8);
        assert!((w.max_weave - 1.0).abs() < 1e-5);
    }

    #[test]
    fn buildup_rate_clamped_to_zero() {
        let w = Weave::new(10.0, -5.0, 2.0, 0.8);
        assert_eq!(w.buildup_rate, 0.0);
    }

    #[test]
    fn falloff_rate_clamped_to_zero() {
        let w = Weave::new(10.0, 5.0, -2.0, 0.8);
        assert_eq!(w.falloff_rate, 0.0);
    }

    #[test]
    fn dodge_bonus_clamped_to_range() {
        let w_low = Weave::new(10.0, 5.0, 2.0, -0.5);
        assert_eq!(w_low.dodge_bonus, 0.0);
        let w_high = Weave::new(10.0, 5.0, 2.0, 2.0);
        assert!((w_high.dodge_bonus - 1.0).abs() < 1e-5);
    }

    #[test]
    fn begin_halt_begin_resumes_from_current_level() {
        let mut w = w();
        w.begin();
        w.tick(1.0); // 5.0
        w.halt();
        w.tick(1.0); // 5.0 - 2.0 = 3.0
        w.begin();
        w.tick(1.0); // 3.0 + 5.0 = 8.0
        assert!((w.weave_level - 8.0).abs() < 1e-4);
    }

    #[test]
    fn full_drain_then_rebuild_peaks_again() {
        let mut w = w();
        w.begin();
        w.tick(2.0); // 10.0, just_peaked
        w.halt();
        w.tick(5.0); // 0.0, just_broken
        assert!(w.just_broken);
        w.tick(0.016); // clear
        w.begin();
        w.tick(2.0); // 10.0, just_peaked
        assert!(w.just_peaked);
    }
}
