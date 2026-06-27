use bevy_ecs::prelude::Component;

/// Combat-endurance defense ramp that rewards staying in the fight. While
/// `in_combat`, `combat_time` accumulates. The entity's damage reduction
/// scales from zero up to `defense_bonus` over `max_bonus_time` seconds,
/// encouraging sustained engagement over hit-and-run tactics.
///
/// `enter_combat()` starts accumulating combat time when the entity enters
/// combat for the first time. No-op if already in combat or disabled.
///
/// `exit_combat()` clears `in_combat` and resets `combat_time` to zero.
/// No-op when not in combat.
///
/// `tick(dt)` clears `just_peaked` at start; when `in_combat`, increments
/// `combat_time` (capped at `max_bonus_time`); fires `just_peaked` once on
/// the first tick that reaches the cap. No-op when disabled.
///
/// `is_peaking()` returns `true` when `combat_time >= max_bonus_time`,
/// `in_combat`, and `enabled`.
///
/// `resilience_fraction()` returns `(combat_time / max_bonus_time).clamp(0, 1)`.
///
/// `effective_incoming(base)` returns `base * (1.0 - defense_bonus * resilience_fraction())`
/// when enabled (capped so damage is never negative); returns `base` when disabled.
///
/// Distinct from `Survive` (death-prevention with a one-time save), `Guard`
/// (block-stance that trades mobility for defence), `Barrier` (absorbing a
/// finite shield pool), and `Toughness` (static flat damage reduction): Outlast
/// is a **dynamic time-in-combat defence ramp** — resilience builds while you
/// stay and fight, then resets the moment you disengage.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Outlast {
    /// Seconds accumulated in the current combat engagement.
    pub combat_time: f32,
    /// Seconds to reach full defense bonus. Clamped ≥ 1.0.
    pub max_bonus_time: f32,
    /// Maximum fractional damage reduction at full combat time. Clamped [0.0, 1.0].
    pub defense_bonus: f32,
    pub in_combat: bool,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Outlast {
    pub fn new(max_bonus_time: f32, defense_bonus: f32) -> Self {
        Self {
            combat_time: 0.0,
            max_bonus_time: max_bonus_time.max(1.0),
            defense_bonus: defense_bonus.clamp(0.0, 1.0),
            in_combat: false,
            just_peaked: false,
            enabled: true,
        }
    }

    /// Enter combat and begin accumulating time. No-op when already in combat
    /// or disabled.
    pub fn enter_combat(&mut self) {
        if !self.enabled || self.in_combat {
            return;
        }
        self.in_combat = true;
        self.combat_time = 0.0;
    }

    /// Leave combat and reset all accumulated time. No-op when not in combat.
    pub fn exit_combat(&mut self) {
        if !self.in_combat {
            return;
        }
        self.in_combat = false;
        self.combat_time = 0.0;
        self.just_peaked = false;
    }

    /// Advance combat timer. Clears `just_peaked` at start; increments
    /// `combat_time` while in combat; fires `just_peaked` on first reach of
    /// `max_bonus_time`. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;

        if !self.enabled || !self.in_combat {
            return;
        }

        let was_below_peak = self.combat_time < self.max_bonus_time;
        self.combat_time = (self.combat_time + dt).min(self.max_bonus_time);
        if was_below_peak && self.combat_time >= self.max_bonus_time {
            self.just_peaked = true;
        }
    }

    /// `true` when combat time has reached `max_bonus_time`, entity is in
    /// combat, and the component is enabled.
    pub fn is_peaking(&self) -> bool {
        self.combat_time >= self.max_bonus_time && self.in_combat && self.enabled
    }

    /// Fraction of maximum combat time elapsed [0.0 = just entered, 1.0 = peak].
    pub fn resilience_fraction(&self) -> f32 {
        (self.combat_time / self.max_bonus_time).clamp(0.0, 1.0)
    }

    /// Incoming damage reduced by accumulated resilience. Returns
    /// `base * (1.0 - defense_bonus * resilience_fraction())` when enabled,
    /// floored at 0.0. Returns `base` when disabled.
    pub fn effective_incoming(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.defense_bonus * self.resilience_fraction())).max(0.0)
    }
}

impl Default for Outlast {
    fn default() -> Self {
        Self::new(60.0, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_out_of_combat() {
        let o = Outlast::new(60.0, 0.3);
        assert!(!o.in_combat);
        assert_eq!(o.combat_time, 0.0);
    }

    #[test]
    fn enter_combat_sets_flag() {
        let mut o = Outlast::new(60.0, 0.3);
        o.enter_combat();
        assert!(o.in_combat);
    }

    #[test]
    fn enter_combat_resets_timer() {
        let mut o = Outlast::new(60.0, 0.3);
        o.enter_combat();
        o.tick(10.0);
        o.exit_combat();
        o.enter_combat();
        assert_eq!(o.combat_time, 0.0);
    }

    #[test]
    fn enter_combat_no_op_when_already_in_combat() {
        let mut o = Outlast::new(60.0, 0.3);
        o.enter_combat();
        o.tick(10.0);
        o.enter_combat(); // should not reset timer
        assert!((o.combat_time - 10.0).abs() < 1e-5);
    }

    #[test]
    fn enter_combat_no_op_when_disabled() {
        let mut o = Outlast::new(60.0, 0.3);
        o.enabled = false;
        o.enter_combat();
        assert!(!o.in_combat);
    }

    #[test]
    fn exit_combat_clears_flag_and_timer() {
        let mut o = Outlast::new(60.0, 0.3);
        o.enter_combat();
        o.tick(20.0);
        o.exit_combat();
        assert!(!o.in_combat);
        assert_eq!(o.combat_time, 0.0);
    }

    #[test]
    fn exit_combat_no_op_when_not_in_combat() {
        let mut o = Outlast::new(60.0, 0.3);
        o.exit_combat(); // should not panic or change state
        assert!(!o.in_combat);
    }

    #[test]
    fn tick_accumulates_combat_time() {
        let mut o = Outlast::new(60.0, 0.3);
        o.enter_combat();
        o.tick(10.0);
        assert!((o.combat_time - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_max_bonus_time() {
        let mut o = Outlast::new(30.0, 0.3);
        o.enter_combat();
        o.tick(100.0);
        assert!((o.combat_time - 30.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_peaked_on_reaching_max() {
        let mut o = Outlast::new(10.0, 0.3);
        o.enter_combat();
        o.tick(10.0);
        assert!(o.just_peaked);
        assert!(o.is_peaking());
    }

    #[test]
    fn tick_no_just_peaked_when_already_peaked() {
        let mut o = Outlast::new(5.0, 0.3);
        o.enter_combat();
        o.tick(5.0); // peaks
        o.tick(0.016); // still peaked, flag cleared
        assert!(!o.just_peaked);
    }

    #[test]
    fn tick_no_op_when_not_in_combat() {
        let mut o = Outlast::new(60.0, 0.3);
        o.tick(100.0);
        assert_eq!(o.combat_time, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut o = Outlast::new(60.0, 0.3);
        o.enter_combat();
        o.enabled = false;
        o.tick(100.0);
        assert_eq!(o.combat_time, 0.0);
    }

    #[test]
    fn tick_clears_just_peaked_each_frame() {
        let mut o = Outlast::new(5.0, 0.3);
        o.enter_combat();
        o.tick(5.0); // just_peaked = true
        o.tick(0.016); // cleared
        assert!(!o.just_peaked);
    }

    #[test]
    fn is_peaking_true_at_max() {
        let mut o = Outlast::new(10.0, 0.3);
        o.enter_combat();
        o.tick(10.0);
        assert!(o.is_peaking());
    }

    #[test]
    fn is_peaking_false_before_max() {
        let mut o = Outlast::new(10.0, 0.3);
        o.enter_combat();
        o.tick(5.0);
        assert!(!o.is_peaking());
    }

    #[test]
    fn is_peaking_false_when_disabled() {
        let mut o = Outlast::new(5.0, 0.3);
        o.enter_combat();
        o.tick(5.0);
        o.enabled = false;
        assert!(!o.is_peaking());
    }

    #[test]
    fn is_peaking_false_out_of_combat() {
        let mut o = Outlast::new(5.0, 0.3);
        o.enter_combat();
        o.tick(5.0);
        o.exit_combat();
        assert!(!o.is_peaking());
    }

    #[test]
    fn resilience_fraction_at_zero() {
        let o = Outlast::new(60.0, 0.3);
        assert_eq!(o.resilience_fraction(), 0.0);
    }

    #[test]
    fn resilience_fraction_at_half() {
        let mut o = Outlast::new(60.0, 0.3);
        o.enter_combat();
        o.tick(30.0);
        assert!((o.resilience_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn resilience_fraction_at_full() {
        let mut o = Outlast::new(30.0, 0.3);
        o.enter_combat();
        o.tick(30.0);
        assert!((o.resilience_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_scales_at_half_resilience() {
        let mut o = Outlast::new(60.0, 0.4);
        o.enter_combat();
        o.tick(30.0); // 0.5 fraction → 0.4 * 0.5 = 0.2 reduction
                      // 100 * (1 - 0.2) = 80
        assert!((o.effective_incoming(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_incoming_full_reduction_at_peak() {
        let mut o = Outlast::new(10.0, 0.5);
        o.enter_combat();
        o.tick(10.0); // full resilience → 0.5 reduction
                      // 100 * (1 - 0.5) = 50
        assert!((o.effective_incoming(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_incoming_base_when_disabled() {
        let mut o = Outlast::new(10.0, 0.5);
        o.enter_combat();
        o.tick(10.0);
        o.enabled = false;
        assert!((o.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_no_reduction_at_zero_combat_time() {
        let mut o = Outlast::new(60.0, 0.5);
        o.enter_combat();
        // no tick — combat_time = 0
        assert!((o.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_floored_at_zero() {
        let mut o = Outlast::new(5.0, 1.0);
        o.enter_combat();
        o.tick(5.0); // full resilience, 100% reduction
        assert_eq!(o.effective_incoming(50.0), 0.0);
    }

    #[test]
    fn combat_time_resets_on_re_enter() {
        let mut o = Outlast::new(30.0, 0.3);
        o.enter_combat();
        o.tick(30.0);
        o.exit_combat();
        o.enter_combat();
        assert_eq!(o.combat_time, 0.0);
        assert!((o.resilience_fraction()).abs() < 1e-5);
    }

    #[test]
    fn max_bonus_time_clamped_to_one() {
        let o = Outlast::new(0.0, 0.3);
        assert!((o.max_bonus_time - 1.0).abs() < 1e-5);
    }

    #[test]
    fn defense_bonus_clamped_to_one() {
        let o = Outlast::new(30.0, 2.0);
        assert!((o.defense_bonus - 1.0).abs() < 1e-5);
    }

    #[test]
    fn defense_bonus_clamped_to_zero() {
        let o = Outlast::new(30.0, -0.5);
        assert_eq!(o.defense_bonus, 0.0);
    }
}
