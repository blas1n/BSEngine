use bevy_ecs::prelude::Component;

/// Risk-escalation tracker that amplifies damage output the longer an entity
/// stays engaged. While `in_venture`, `venture_level` climbs at `venture_rate`
/// per second up to `max_venture`; when disengaged, it decays at
/// `recovery_rate`. Outgoing damage scales linearly with the current
/// `venture_fraction()`.
///
/// `begin_venture()` sets `in_venture = true`. No-op when already venturing
/// or disabled.
///
/// `end_venture()` sets `in_venture = false`. No-op when not venturing.
///
/// `tick(dt)` clears `just_peaked` at start; when venturing: increments
/// `venture_level` at `venture_rate * dt` (capped at `max_venture`); fires
/// `just_peaked` once on the first reach of `max_venture`; when not venturing
/// and `venture_level > 0`: decrements at `recovery_rate * dt` (floored at
/// 0.0). No-op when disabled.
///
/// `is_peaked()` returns `venture_level >= max_venture && in_venture &&
/// enabled`.
///
/// `venture_fraction()` returns `(venture_level / max_venture).clamp(0.0,
/// 1.0)`.
///
/// `effective_outgoing(base)` returns
/// `base * (1.0 + damage_bonus * venture_fraction())` when enabled; `base`
/// otherwise.
///
/// Distinct from `Patience` (bonus after sustained *waiting* with no action),
/// `Feral` (kill-count scaling), `Fervor` (attack-speed build-up), and
/// `Combo` (rapid-hit streak that decays on gaps): Venture is a **continuous
/// engagement ramp** — the damage bonus scales in real time with how long the
/// entity has been in active combat, then bleeds off during disengagement.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Venture {
    /// Current engagement depth [0.0, max_venture].
    pub venture_level: f32,
    /// Maximum venture level. Clamped >= 1.0.
    pub max_venture: f32,
    /// Venture level gained per second while engaged. Clamped >= 0.0.
    pub venture_rate: f32,
    /// Venture level lost per second while disengaged. Clamped >= 0.0.
    pub recovery_rate: f32,
    /// Maximum fractional damage bonus at full venture. Clamped [0.0, 1.0].
    pub damage_bonus: f32,
    pub in_venture: bool,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Venture {
    pub fn new(max_venture: f32, venture_rate: f32, recovery_rate: f32, damage_bonus: f32) -> Self {
        Self {
            venture_level: 0.0,
            max_venture: max_venture.max(1.0),
            venture_rate: venture_rate.max(0.0),
            recovery_rate: recovery_rate.max(0.0),
            damage_bonus: damage_bonus.clamp(0.0, 1.0),
            in_venture: false,
            just_peaked: false,
            enabled: true,
        }
    }

    /// Begin active engagement — venture level starts climbing. No-op when
    /// already venturing or disabled.
    pub fn begin_venture(&mut self) {
        if !self.enabled || self.in_venture {
            return;
        }
        self.in_venture = true;
    }

    /// Leave active engagement — venture level starts decaying. No-op when
    /// not venturing.
    pub fn end_venture(&mut self) {
        if !self.in_venture {
            return;
        }
        self.in_venture = false;
    }

    /// Advance venture accumulation or decay. Clears `just_peaked` first;
    /// when venturing: grows `venture_level` toward `max_venture` and fires
    /// `just_peaked` on first reach; when not venturing: decays toward 0.0.
    /// No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;

        if !self.enabled {
            return;
        }

        if self.in_venture {
            let was_below = self.venture_level < self.max_venture;
            self.venture_level =
                (self.venture_level + self.venture_rate * dt).min(self.max_venture);
            if was_below && self.venture_level >= self.max_venture {
                self.just_peaked = true;
            }
        } else if self.venture_level > 0.0 {
            self.venture_level = (self.venture_level - self.recovery_rate * dt).max(0.0);
        }
    }

    /// `true` when at maximum venture while actively engaged and enabled.
    pub fn is_peaked(&self) -> bool {
        self.venture_level >= self.max_venture && self.in_venture && self.enabled
    }

    /// Engagement progress [0.0 = disengaged, 1.0 = fully peaked].
    pub fn venture_fraction(&self) -> f32 {
        (self.venture_level / self.max_venture).clamp(0.0, 1.0)
    }

    /// Outgoing damage amplified by venture depth. Returns
    /// `base * (1.0 + damage_bonus * venture_fraction())` when enabled;
    /// `base` otherwise.
    pub fn effective_outgoing(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.damage_bonus * self.venture_fraction())
    }
}

impl Default for Venture {
    fn default() -> Self {
        Self::new(10.0, 2.0, 5.0, 0.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_not_venturing() {
        let v = Venture::new(10.0, 2.0, 5.0, 0.4);
        assert!(!v.in_venture);
        assert_eq!(v.venture_level, 0.0);
        assert!(!v.just_peaked);
    }

    #[test]
    fn begin_venture_sets_flag() {
        let mut v = Venture::new(10.0, 2.0, 5.0, 0.4);
        v.begin_venture();
        assert!(v.in_venture);
    }

    #[test]
    fn begin_venture_no_op_when_already_venturing() {
        let mut v = Venture::new(10.0, 2.0, 5.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // level = 2.0
        v.begin_venture(); // should not reset
        assert!((v.venture_level - 2.0).abs() < 1e-5);
    }

    #[test]
    fn begin_venture_no_op_when_disabled() {
        let mut v = Venture::new(10.0, 2.0, 5.0, 0.4);
        v.enabled = false;
        v.begin_venture();
        assert!(!v.in_venture);
    }

    #[test]
    fn end_venture_clears_flag() {
        let mut v = Venture::new(10.0, 2.0, 5.0, 0.4);
        v.begin_venture();
        v.end_venture();
        assert!(!v.in_venture);
    }

    #[test]
    fn end_venture_no_op_when_not_venturing() {
        let mut v = Venture::new(10.0, 2.0, 5.0, 0.4);
        v.end_venture(); // should not panic
        assert!(!v.in_venture);
    }

    #[test]
    fn tick_grows_level_while_venturing() {
        let mut v = Venture::new(10.0, 2.0, 5.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // +2.0
        assert!((v.venture_level - 2.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_max_venture() {
        let mut v = Venture::new(10.0, 20.0, 5.0, 0.4);
        v.begin_venture();
        v.tick(2.0); // 40 would exceed 10
        assert!((v.venture_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_peaked_on_reaching_max() {
        let mut v = Venture::new(5.0, 5.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // exactly 5.0
        assert!(v.just_peaked);
        assert!(v.is_peaked());
    }

    #[test]
    fn tick_no_just_peaked_when_already_at_max() {
        let mut v = Venture::new(5.0, 5.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // peaks
        v.tick(0.016); // still peaked, flag cleared
        assert!(!v.just_peaked);
    }

    #[test]
    fn tick_no_just_peaked_below_max() {
        let mut v = Venture::new(10.0, 2.0, 5.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // 2.0 < 10.0
        assert!(!v.just_peaked);
    }

    #[test]
    fn tick_decays_level_when_not_venturing() {
        let mut v = Venture::new(10.0, 10.0, 5.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // level = 10.0
        v.end_venture();
        v.tick(1.0); // -5.0 = 5.0
        assert!((v.venture_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decay_floors_at_zero() {
        let mut v = Venture::new(10.0, 2.0, 10.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // level = 2.0
        v.end_venture();
        v.tick(5.0); // -50, floored to 0
        assert_eq!(v.venture_level, 0.0);
    }

    #[test]
    fn tick_no_change_when_not_venturing_and_at_zero() {
        let mut v = Venture::new(10.0, 2.0, 5.0, 0.4);
        v.tick(1.0); // not venturing, level = 0 — no change
        assert_eq!(v.venture_level, 0.0);
    }

    #[test]
    fn tick_clears_just_peaked_next_frame() {
        let mut v = Venture::new(5.0, 5.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // just_peaked = true
        v.tick(0.016); // cleared
        assert!(!v.just_peaked);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut v = Venture::new(10.0, 2.0, 5.0, 0.4);
        v.begin_venture();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.venture_level, 0.0);
    }

    #[test]
    fn is_peaked_true_at_max_while_venturing() {
        let mut v = Venture::new(5.0, 10.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0);
        assert!(v.is_peaked());
    }

    #[test]
    fn is_peaked_false_when_not_venturing() {
        let mut v = Venture::new(5.0, 10.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0);
        v.end_venture();
        assert!(!v.is_peaked());
    }

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut v = Venture::new(5.0, 10.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0);
        v.enabled = false;
        assert!(!v.is_peaked());
    }

    #[test]
    fn is_peaked_false_below_max() {
        let mut v = Venture::new(10.0, 2.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // level = 2.0 < 10.0
        assert!(!v.is_peaked());
    }

    #[test]
    fn venture_fraction_zero_when_no_level() {
        let v = Venture::new(10.0, 2.0, 5.0, 0.4);
        assert_eq!(v.venture_fraction(), 0.0);
    }

    #[test]
    fn venture_fraction_half_at_midpoint() {
        let mut v = Venture::new(10.0, 5.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // level = 5.0 = 50%
        assert!((v.venture_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn venture_fraction_one_at_max() {
        let mut v = Venture::new(10.0, 10.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0);
        assert!((v.venture_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_amplified_at_full_venture() {
        let mut v = Venture::new(10.0, 10.0, 1.0, 0.5);
        v.begin_venture();
        v.tick(1.0); // full venture
                     // 100 * (1 + 0.5 * 1.0) = 150
        assert!((v.effective_outgoing(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_partial_at_half_venture() {
        let mut v = Venture::new(10.0, 5.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0); // 50% venture
                     // 100 * (1 + 0.4 * 0.5) = 120
        assert!((v.effective_outgoing(100.0) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_base_when_no_venture() {
        let v = Venture::new(10.0, 2.0, 5.0, 0.4);
        assert!((v.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_base_when_disabled() {
        let mut v = Venture::new(10.0, 10.0, 1.0, 0.4);
        v.begin_venture();
        v.tick(1.0);
        v.enabled = false;
        assert!((v.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn max_venture_clamped_to_one() {
        let v = Venture::new(0.0, 2.0, 5.0, 0.4);
        assert!((v.max_venture - 1.0).abs() < 1e-5);
    }

    #[test]
    fn venture_rate_clamped_to_zero() {
        let v = Venture::new(10.0, -1.0, 5.0, 0.4);
        assert_eq!(v.venture_rate, 0.0);
    }

    #[test]
    fn recovery_rate_clamped_to_zero() {
        let v = Venture::new(10.0, 2.0, -1.0, 0.4);
        assert_eq!(v.recovery_rate, 0.0);
    }

    #[test]
    fn damage_bonus_clamped_to_one() {
        let v = Venture::new(10.0, 2.0, 5.0, 2.0);
        assert!((v.damage_bonus - 1.0).abs() < 1e-5);
    }

    #[test]
    fn damage_bonus_clamped_to_zero() {
        let v = Venture::new(10.0, 2.0, 5.0, -0.5);
        assert_eq!(v.damage_bonus, 0.0);
    }

    #[test]
    fn level_persists_after_end_venture() {
        let mut v = Venture::new(10.0, 4.0, 0.0, 0.4); // zero recovery
        v.begin_venture();
        v.tick(1.0); // level = 4.0
        v.end_venture();
        assert!((v.venture_level - 4.0).abs() < 1e-5); // still 4.0 (no decay yet)
    }
}
