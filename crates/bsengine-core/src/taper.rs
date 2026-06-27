use bevy_ecs::prelude::Component;

/// Battle-fatigue damage penalty that grows the longer a fight drags on
/// without a kill. While `in_combat`, `elapsed_time` accumulates. Outgoing
/// damage is reduced from zero up to `damage_reduction` over
/// `max_reduction_time` seconds, representing mounting exhaustion and
/// frustration when an enemy refuses to fall.
///
/// `engage()` starts accumulating fight time. No-op when already in combat
/// or disabled.
///
/// `disengage()` stops accumulation and resets `elapsed_time`. No-op when
/// not in combat.
///
/// `tick(dt)` clears `just_peaked` at start; when `in_combat`, increments
/// `elapsed_time` (capped at `max_reduction_time`); fires `just_peaked` once
/// on the first tick that reaches the cap. No-op when disabled.
///
/// `is_fully_tapered()` returns `true` when `elapsed_time >= max_reduction_time`,
/// `in_combat`, and `enabled`.
///
/// `taper_fraction()` returns `(elapsed_time / max_reduction_time).clamp(0, 1)`.
///
/// `effective_outgoing(base)` returns `base * (1.0 - damage_reduction * taper_fraction())`
/// when enabled (floored at 0.0); returns `base` when disabled.
///
/// Distinct from `Exhaustion` (cumulative stamina drain from physical exertion
/// that limits actions), `Wrath` (timed voluntary offence boost), `Fervor`
/// (intensity/passion rising under pressure), and `Outlast` (defence ramp for
/// staying in the fight): Taper is a **sustained-engagement damage penalty**
/// — damage erodes with time, penalising drawn-out fights and rewarding
/// finishing enemies quickly.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Taper {
    /// Seconds accumulated in the current combat engagement.
    pub elapsed_time: f32,
    /// Seconds until full damage reduction is applied. Clamped ≥ 1.0.
    pub max_reduction_time: f32,
    /// Maximum fractional outgoing damage penalty at full elapsed time.
    /// Clamped [0.0, 1.0].
    pub damage_reduction: f32,
    pub in_combat: bool,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Taper {
    pub fn new(max_reduction_time: f32, damage_reduction: f32) -> Self {
        Self {
            elapsed_time: 0.0,
            max_reduction_time: max_reduction_time.max(1.0),
            damage_reduction: damage_reduction.clamp(0.0, 1.0),
            in_combat: false,
            just_peaked: false,
            enabled: true,
        }
    }

    /// Enter combat and begin accumulating fight time. Resets `elapsed_time`.
    /// No-op when already in combat or disabled.
    pub fn engage(&mut self) {
        if !self.enabled || self.in_combat {
            return;
        }
        self.in_combat = true;
        self.elapsed_time = 0.0;
    }

    /// Leave combat and reset accumulated time. No-op when not in combat.
    pub fn disengage(&mut self) {
        if !self.in_combat {
            return;
        }
        self.in_combat = false;
        self.elapsed_time = 0.0;
        self.just_peaked = false;
    }

    /// Advance fight timer. Clears `just_peaked` at start; increments
    /// `elapsed_time` while in combat (capped at `max_reduction_time`); fires
    /// `just_peaked` on first reach. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;

        if !self.enabled || !self.in_combat {
            return;
        }

        let was_below_peak = self.elapsed_time < self.max_reduction_time;
        self.elapsed_time = (self.elapsed_time + dt).min(self.max_reduction_time);
        if was_below_peak && self.elapsed_time >= self.max_reduction_time {
            self.just_peaked = true;
        }
    }

    /// `true` when fight time has reached `max_reduction_time`, entity is in
    /// combat, and the component is enabled.
    pub fn is_fully_tapered(&self) -> bool {
        self.elapsed_time >= self.max_reduction_time && self.in_combat && self.enabled
    }

    /// Fraction of maximum fight time elapsed [0.0 = just engaged, 1.0 = fully tapered].
    pub fn taper_fraction(&self) -> f32 {
        (self.elapsed_time / self.max_reduction_time).clamp(0.0, 1.0)
    }

    /// Outgoing damage reduced by accumulated fatigue. Returns
    /// `base * (1.0 - damage_reduction * taper_fraction())` when enabled,
    /// floored at 0.0. Returns `base` when disabled.
    pub fn effective_outgoing(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.damage_reduction * self.taper_fraction())).max(0.0)
    }
}

impl Default for Taper {
    fn default() -> Self {
        Self::new(45.0, 0.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_out_of_combat() {
        let t = Taper::new(45.0, 0.4);
        assert!(!t.in_combat);
        assert_eq!(t.elapsed_time, 0.0);
    }

    #[test]
    fn engage_sets_flag() {
        let mut t = Taper::new(45.0, 0.4);
        t.engage();
        assert!(t.in_combat);
    }

    #[test]
    fn engage_resets_timer() {
        let mut t = Taper::new(45.0, 0.4);
        t.engage();
        t.tick(10.0);
        t.disengage();
        t.engage();
        assert_eq!(t.elapsed_time, 0.0);
    }

    #[test]
    fn engage_no_op_when_already_in_combat() {
        let mut t = Taper::new(45.0, 0.4);
        t.engage();
        t.tick(10.0);
        t.engage(); // should not reset timer
        assert!((t.elapsed_time - 10.0).abs() < 1e-5);
    }

    #[test]
    fn engage_no_op_when_disabled() {
        let mut t = Taper::new(45.0, 0.4);
        t.enabled = false;
        t.engage();
        assert!(!t.in_combat);
    }

    #[test]
    fn disengage_clears_flag_and_timer() {
        let mut t = Taper::new(45.0, 0.4);
        t.engage();
        t.tick(20.0);
        t.disengage();
        assert!(!t.in_combat);
        assert_eq!(t.elapsed_time, 0.0);
    }

    #[test]
    fn disengage_no_op_when_not_in_combat() {
        let mut t = Taper::new(45.0, 0.4);
        t.disengage(); // should not panic
        assert!(!t.in_combat);
    }

    #[test]
    fn tick_accumulates_elapsed_time() {
        let mut t = Taper::new(45.0, 0.4);
        t.engage();
        t.tick(10.0);
        assert!((t.elapsed_time - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_max_reduction_time() {
        let mut t = Taper::new(30.0, 0.4);
        t.engage();
        t.tick(100.0);
        assert!((t.elapsed_time - 30.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_peaked_on_reaching_max() {
        let mut t = Taper::new(10.0, 0.4);
        t.engage();
        t.tick(10.0);
        assert!(t.just_peaked);
        assert!(t.is_fully_tapered());
    }

    #[test]
    fn tick_no_just_peaked_when_already_peaked() {
        let mut t = Taper::new(5.0, 0.4);
        t.engage();
        t.tick(5.0); // peaks
        t.tick(0.016); // still peaked, flag cleared
        assert!(!t.just_peaked);
    }

    #[test]
    fn tick_no_op_when_not_in_combat() {
        let mut t = Taper::new(45.0, 0.4);
        t.tick(100.0);
        assert_eq!(t.elapsed_time, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut t = Taper::new(45.0, 0.4);
        t.engage();
        t.enabled = false;
        t.tick(100.0);
        assert_eq!(t.elapsed_time, 0.0);
    }

    #[test]
    fn tick_clears_just_peaked_each_frame() {
        let mut t = Taper::new(5.0, 0.4);
        t.engage();
        t.tick(5.0); // just_peaked = true
        t.tick(0.016); // cleared
        assert!(!t.just_peaked);
    }

    #[test]
    fn is_fully_tapered_true_at_max() {
        let mut t = Taper::new(10.0, 0.4);
        t.engage();
        t.tick(10.0);
        assert!(t.is_fully_tapered());
    }

    #[test]
    fn is_fully_tapered_false_before_max() {
        let mut t = Taper::new(10.0, 0.4);
        t.engage();
        t.tick(5.0);
        assert!(!t.is_fully_tapered());
    }

    #[test]
    fn is_fully_tapered_false_when_disabled() {
        let mut t = Taper::new(5.0, 0.4);
        t.engage();
        t.tick(5.0);
        t.enabled = false;
        assert!(!t.is_fully_tapered());
    }

    #[test]
    fn is_fully_tapered_false_out_of_combat() {
        let mut t = Taper::new(5.0, 0.4);
        t.engage();
        t.tick(5.0);
        t.disengage();
        assert!(!t.is_fully_tapered());
    }

    #[test]
    fn taper_fraction_at_zero() {
        let t = Taper::new(45.0, 0.4);
        assert_eq!(t.taper_fraction(), 0.0);
    }

    #[test]
    fn taper_fraction_at_half() {
        let mut t = Taper::new(20.0, 0.4);
        t.engage();
        t.tick(10.0);
        assert!((t.taper_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn taper_fraction_at_full() {
        let mut t = Taper::new(10.0, 0.4);
        t.engage();
        t.tick(10.0);
        assert!((t.taper_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_no_penalty_at_start() {
        let mut t = Taper::new(45.0, 0.4);
        t.engage();
        // elapsed = 0, no reduction
        assert!((t.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_half_penalty_at_half_time() {
        let mut t = Taper::new(20.0, 0.4);
        t.engage();
        t.tick(10.0); // 0.5 fraction → 0.4 * 0.5 = 0.2 reduction
                      // 100 * (1 - 0.2) = 80
        assert!((t.effective_outgoing(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_full_penalty_at_peak() {
        let mut t = Taper::new(10.0, 0.5);
        t.engage();
        t.tick(10.0); // full taper → 0.5 reduction
                      // 100 * (1 - 0.5) = 50
        assert!((t.effective_outgoing(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_base_when_disabled() {
        let mut t = Taper::new(10.0, 0.5);
        t.engage();
        t.tick(10.0);
        t.enabled = false;
        assert!((t.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_floored_at_zero() {
        let mut t = Taper::new(5.0, 1.0);
        t.engage();
        t.tick(5.0); // full taper, 100% reduction
        assert_eq!(t.effective_outgoing(50.0), 0.0);
    }

    #[test]
    fn elapsed_time_resets_on_re_engage() {
        let mut t = Taper::new(30.0, 0.4);
        t.engage();
        t.tick(30.0);
        t.disengage();
        t.engage();
        assert_eq!(t.elapsed_time, 0.0);
        assert_eq!(t.taper_fraction(), 0.0);
    }

    #[test]
    fn max_reduction_time_clamped_to_one() {
        let t = Taper::new(0.0, 0.4);
        assert!((t.max_reduction_time - 1.0).abs() < 1e-5);
    }

    #[test]
    fn damage_reduction_clamped_to_one() {
        let t = Taper::new(30.0, 2.0);
        assert!((t.damage_reduction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn damage_reduction_clamped_to_zero() {
        let t = Taper::new(30.0, -0.5);
        assert_eq!(t.damage_reduction, 0.0);
    }
}
