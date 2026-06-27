use bevy_ecs::prelude::Component;

/// Action-momentum tracker. `verve_level` climbs by `gain_per_action` each
/// time `act()` is called and decays passively at `decay_rate` per second
/// during `tick()`. High verve multiplies any scalar (ability speed, regen
/// rate, effectiveness) via `effective_bonus(base)`.
///
/// `act()` increases `verve_level` by `gain_per_action` (capped at
/// `max_verve`). Fires `just_peaked` on the first reach of `max_verve`.
/// No-op when disabled or `gain_per_action == 0`.
///
/// `tick(dt)` clears `just_peaked` first; decays `verve_level` by
/// `decay_rate * dt` (floored at 0.0). No-op when disabled.
///
/// `is_peaked()` returns `verve_level >= max_verve && enabled`.
///
/// `verve_fraction()` returns `(verve_level / max_verve).clamp(0.0, 1.0)`.
///
/// `effective_bonus(base)` returns
/// `base * (1.0 + action_bonus * verve_fraction())` when enabled; `base`
/// otherwise. This is a pure query — does NOT consume verve.
///
/// Distinct from `Fervor` (attack-speed build-up driven specifically by hit
/// streak), `Venture` (continuous engagement ramp that decays when not in
/// active combat), `Combo` (rapid-hit streak that resets on gaps), and
/// `Momentum` (speed that builds from movement): Verve is a **general
/// action-momentum counter** — any call to `act()` contributes, regardless
/// of action type, making it suitable as a cross-system vitality multiplier.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Verve {
    /// Current vitality level [0.0, max_verve].
    pub verve_level: f32,
    /// Maximum vitality level. Clamped >= 1.0.
    pub max_verve: f32,
    /// Verve gained per `act()` call. Clamped >= 0.0.
    pub gain_per_action: f32,
    /// Verve lost per second during inactivity. Clamped >= 0.0.
    pub decay_rate: f32,
    /// Maximum fractional bonus at full verve. Clamped [0.0, 1.0].
    pub action_bonus: f32,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Verve {
    pub fn new(max_verve: f32, gain_per_action: f32, decay_rate: f32, action_bonus: f32) -> Self {
        Self {
            verve_level: 0.0,
            max_verve: max_verve.max(1.0),
            gain_per_action: gain_per_action.max(0.0),
            decay_rate: decay_rate.max(0.0),
            action_bonus: action_bonus.clamp(0.0, 1.0),
            just_peaked: false,
            enabled: true,
        }
    }

    /// Record an action — increases `verve_level` by `gain_per_action`.
    /// Fires `just_peaked` on first reach of `max_verve`. No-op when
    /// disabled or `gain_per_action == 0`.
    pub fn act(&mut self) {
        if !self.enabled || self.gain_per_action == 0.0 {
            return;
        }
        let was_below = self.verve_level < self.max_verve;
        self.verve_level = (self.verve_level + self.gain_per_action).min(self.max_verve);
        if was_below && self.verve_level >= self.max_verve {
            self.just_peaked = true;
        }
    }

    /// Advance one frame: clear `just_peaked`, apply passive decay. No-op
    /// when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;

        if !self.enabled {
            return;
        }

        if self.verve_level > 0.0 && self.decay_rate > 0.0 {
            self.verve_level = (self.verve_level - self.decay_rate * dt).max(0.0);
        }
    }

    /// `true` when `verve_level` is at maximum and the component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.verve_level >= self.max_verve && self.enabled
    }

    /// Action momentum as a fraction [0.0 = idle, 1.0 = full vitality].
    pub fn verve_fraction(&self) -> f32 {
        (self.verve_level / self.max_verve).clamp(0.0, 1.0)
    }

    /// Applies the verve bonus to `base`. Returns
    /// `base * (1.0 + action_bonus * verve_fraction())` when enabled; `base`
    /// otherwise. Pure query — does NOT consume verve.
    pub fn effective_bonus(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.action_bonus * self.verve_fraction())
    }
}

impl Default for Verve {
    fn default() -> Self {
        Self::new(10.0, 1.0, 2.0, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero() {
        let v = Verve::new(10.0, 1.0, 2.0, 0.3);
        assert_eq!(v.verve_level, 0.0);
        assert!(!v.is_peaked());
        assert!(!v.just_peaked);
    }

    #[test]
    fn act_increases_level() {
        let mut v = Verve::new(10.0, 2.0, 1.0, 0.3);
        v.act();
        assert!((v.verve_level - 2.0).abs() < 1e-5);
    }

    #[test]
    fn act_caps_at_max_verve() {
        let mut v = Verve::new(5.0, 10.0, 1.0, 0.3);
        v.act();
        assert!((v.verve_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn act_fires_just_peaked_on_first_reach() {
        let mut v = Verve::new(2.0, 2.0, 0.0, 0.3);
        v.act(); // 0 + 2 = 2 = max
        assert!(v.just_peaked);
        assert!(v.is_peaked());
    }

    #[test]
    fn act_no_just_peaked_below_max() {
        let mut v = Verve::new(10.0, 2.0, 0.0, 0.3);
        v.act(); // 2 < 10
        assert!(!v.just_peaked);
    }

    #[test]
    fn act_no_just_peaked_when_already_peaked() {
        let mut v = Verve::new(2.0, 2.0, 0.0, 0.3);
        v.act(); // peaks
        v.tick(0.016); // clear flag
        v.act(); // still at max, no re-fire
        assert!(!v.just_peaked);
    }

    #[test]
    fn act_no_op_when_disabled() {
        let mut v = Verve::new(10.0, 2.0, 1.0, 0.3);
        v.enabled = false;
        v.act();
        assert_eq!(v.verve_level, 0.0);
    }

    #[test]
    fn act_no_op_when_gain_is_zero() {
        let mut v = Verve::new(10.0, 0.0, 1.0, 0.3);
        v.act();
        assert_eq!(v.verve_level, 0.0);
    }

    #[test]
    fn act_accumulates_across_calls() {
        let mut v = Verve::new(10.0, 3.0, 0.0, 0.3);
        v.act();
        v.act();
        v.act();
        assert!((v.verve_level - 9.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_level() {
        let mut v = Verve::new(10.0, 5.0, 2.0, 0.3);
        v.act(); // 5.0
        v.tick(1.0); // -2.0 = 3.0
        assert!((v.verve_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut v = Verve::new(10.0, 1.0, 10.0, 0.3);
        v.act(); // 1.0
        v.tick(1.0); // would go to -9, floor 0
        assert_eq!(v.verve_level, 0.0);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut v = Verve::new(2.0, 2.0, 0.0, 0.3);
        v.act();
        assert!(v.just_peaked);
        v.tick(0.016);
        assert!(!v.just_peaked);
    }

    #[test]
    fn tick_no_decay_when_zero() {
        let mut v = Verve::new(10.0, 2.0, 5.0, 0.3);
        v.tick(1.0); // level is 0, nothing to decay
        assert_eq!(v.verve_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut v = Verve::new(10.0, 5.0, 2.0, 0.3);
        v.act(); // 5.0
        v.enabled = false;
        v.tick(1.0); // should not decay
        assert!((v.verve_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn is_peaked_true_at_max() {
        let mut v = Verve::new(5.0, 5.0, 0.0, 0.3);
        v.act();
        assert!(v.is_peaked());
    }

    #[test]
    fn is_peaked_false_below_max() {
        let mut v = Verve::new(10.0, 2.0, 0.0, 0.3);
        v.act(); // 2 < 10
        assert!(!v.is_peaked());
    }

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut v = Verve::new(5.0, 5.0, 0.0, 0.3);
        v.act();
        v.enabled = false;
        assert!(!v.is_peaked());
    }

    #[test]
    fn verve_fraction_zero_at_start() {
        let v = Verve::new(10.0, 1.0, 2.0, 0.3);
        assert_eq!(v.verve_fraction(), 0.0);
    }

    #[test]
    fn verve_fraction_half_at_midpoint() {
        let mut v = Verve::new(10.0, 5.0, 0.0, 0.3);
        v.act(); // 5.0 = 50%
        assert!((v.verve_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn verve_fraction_one_at_max() {
        let mut v = Verve::new(10.0, 10.0, 0.0, 0.3);
        v.act();
        assert!((v.verve_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_bonus_amplified_at_full_verve() {
        let mut v = Verve::new(10.0, 10.0, 0.0, 0.5);
        v.act(); // full verve
                 // 100 * (1 + 0.5 * 1.0) = 150
        assert!((v.effective_bonus(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_bonus_partial_at_half_verve() {
        let mut v = Verve::new(10.0, 5.0, 0.0, 0.4);
        v.act(); // 50%
                 // 100 * (1 + 0.4 * 0.5) = 120
        assert!((v.effective_bonus(100.0) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn effective_bonus_base_at_zero_verve() {
        let v = Verve::new(10.0, 1.0, 0.0, 0.3);
        assert!((v.effective_bonus(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_bonus_base_when_disabled() {
        let mut v = Verve::new(10.0, 10.0, 0.0, 0.3);
        v.act();
        v.enabled = false;
        assert!((v.effective_bonus(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_bonus_is_pure_query() {
        let mut v = Verve::new(10.0, 10.0, 0.0, 0.3);
        v.act();
        let level_before = v.verve_level;
        v.effective_bonus(100.0);
        assert!((v.verve_level - level_before).abs() < 1e-5);
    }

    #[test]
    fn max_verve_clamped_to_one() {
        let v = Verve::new(0.0, 1.0, 2.0, 0.3);
        assert!((v.max_verve - 1.0).abs() < 1e-5);
    }

    #[test]
    fn gain_per_action_clamped_to_zero() {
        let v = Verve::new(10.0, -1.0, 2.0, 0.3);
        assert_eq!(v.gain_per_action, 0.0);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let v = Verve::new(10.0, 1.0, -2.0, 0.3);
        assert_eq!(v.decay_rate, 0.0);
    }

    #[test]
    fn action_bonus_clamped_to_one() {
        let v = Verve::new(10.0, 1.0, 2.0, 2.0);
        assert!((v.action_bonus - 1.0).abs() < 1e-5);
    }

    #[test]
    fn action_bonus_clamped_to_zero() {
        let v = Verve::new(10.0, 1.0, 2.0, -0.5);
        assert_eq!(v.action_bonus, 0.0);
    }

    #[test]
    fn act_and_decay_cycle() {
        let mut v = Verve::new(10.0, 5.0, 5.0, 0.3);
        v.act(); // 5.0
        v.tick(0.5); // -2.5 = 2.5
        v.act(); // +5.0 = 7.5
        assert!((v.verve_level - 7.5).abs() < 1e-4);
    }
}
