use bevy_ecs::prelude::Component;

/// Battle-cry energy that builds while the entity gathers breath and
/// discharges in a single shout. Systems read `just_shouted` and
/// `charge_fraction()` to determine when a cry occurs and how potent it is.
///
/// `charge()` begins active charge-up (`charging = true`). No-op when
/// already charging or disabled.
///
/// `cease()` stops charge-up (`charging = false`). No-op when not charging.
///
/// `shout()` releases the cry: fires `just_shouted` if `charge_level > 0`,
/// then resets `charge_level` to 0. No-op when disabled.
///
/// `tick(dt)` clears `just_shouted`, then — when `charging` — increases
/// `charge_level` by `charge_rate * dt` (capped at `max_charge`). No-op when
/// disabled.
///
/// `is_charged()` returns `charge_level >= max_charge && enabled`.
///
/// `charge_fraction()` returns `(charge_level / max_charge).clamp(0.0, 1.0)`.
///
/// `effective_attack(base)` returns
/// `base * (1.0 + power_bonus * charge_fraction())` when enabled; returns
/// `base` unchanged otherwise. Call before `shout()` to read peak intensity.
///
/// Distinct from `Rally` (persistent morale-regen aura affecting allies),
/// `Taunt` (targeted aggro lock on a specific enemy), `Roar`/`Intimidate`
/// (fear debuff applied directly to enemies), and `Surge` (self-speed/damage
/// burst): Yell models an **energy-charged battle cry** with explicit build-up
/// and discharge — the intensity of the cry scales with how long the entity
/// has been charging, giving systems a continuous read on readiness.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yell {
    /// Current charge energy [0.0, max_charge].
    pub charge_level: f32,
    /// Energy needed for a full shout. Clamped >= 1.0.
    pub max_charge: f32,
    /// Charge accumulation per second while charging. Clamped >= 0.0.
    pub charge_rate: f32,
    /// Maximum attack bonus multiplier at full charge [0.0, max]. Clamped >= 0.0.
    pub power_bonus: f32,
    /// Whether the entity is actively building charge.
    pub charging: bool,
    pub just_shouted: bool,
    pub enabled: bool,
}

impl Yell {
    pub fn new(max_charge: f32, charge_rate: f32, power_bonus: f32) -> Self {
        Self {
            charge_level: 0.0,
            max_charge: max_charge.max(1.0),
            charge_rate: charge_rate.max(0.0),
            power_bonus: power_bonus.max(0.0),
            charging: false,
            just_shouted: false,
            enabled: true,
        }
    }

    /// Begin charging. No-op when already charging or disabled.
    pub fn charge(&mut self) {
        if !self.enabled || self.charging {
            return;
        }
        self.charging = true;
    }

    /// Stop charging. No-op when not charging.
    pub fn cease(&mut self) {
        if !self.charging {
            return;
        }
        self.charging = false;
    }

    /// Release the battle cry. Fires `just_shouted` if `charge_level > 0`,
    /// then resets `charge_level` to 0. No-op when disabled.
    pub fn shout(&mut self) {
        if !self.enabled {
            return;
        }
        if self.charge_level > 0.0 {
            self.just_shouted = true;
            self.charge_level = 0.0;
        }
    }

    /// Advance one frame: clear flags, then build charge if active. No-op when
    /// disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_shouted = false;

        if !self.enabled {
            return;
        }
        if self.charging {
            self.charge_level = (self.charge_level + self.charge_rate * dt).min(self.max_charge);
        }
    }

    /// `true` when charge is at maximum and the component is enabled.
    pub fn is_charged(&self) -> bool {
        self.charge_level >= self.max_charge && self.enabled
    }

    /// Charge energy as a fraction of maximum [0.0, 1.0].
    pub fn charge_fraction(&self) -> f32 {
        (self.charge_level / self.max_charge).clamp(0.0, 1.0)
    }

    /// Scale attack `base` by current charge intensity. Returns
    /// `base * (1 + power_bonus * fraction)` when enabled; `base` otherwise.
    /// Query before `shout()` to capture peak power.
    pub fn effective_attack(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.power_bonus * self.charge_fraction())
    }
}

impl Default for Yell {
    fn default() -> Self {
        Self::new(10.0, 4.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yell {
        Yell::new(10.0, 5.0, 0.5)
    }

    #[test]
    fn new_starts_silent_and_uncharged() {
        let y = Yell::new(10.0, 5.0, 0.5);
        assert_eq!(y.charge_level, 0.0);
        assert!(!y.charging);
        assert!(!y.just_shouted);
        assert!(!y.is_charged());
    }

    #[test]
    fn charge_sets_charging() {
        let mut y = y();
        y.charge();
        assert!(y.charging);
    }

    #[test]
    fn charge_no_op_when_already_charging() {
        let mut y = y();
        y.charge();
        y.charge(); // second call
        assert!(y.charging); // still fine
    }

    #[test]
    fn charge_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.charge();
        assert!(!y.charging);
    }

    #[test]
    fn cease_clears_charging() {
        let mut y = y();
        y.charge();
        y.cease();
        assert!(!y.charging);
    }

    #[test]
    fn cease_no_op_when_not_charging() {
        let mut y = y();
        y.cease(); // no error
        assert!(!y.charging);
    }

    #[test]
    fn tick_increases_charge_when_charging() {
        let mut y = y(); // charge_rate = 5.0
        y.charge();
        y.tick(1.0); // 5.0 * 1.0 = 5.0
        assert!((y.charge_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max_charge() {
        let mut y = y();
        y.charge();
        y.tick(10.0); // 5.0 * 10 → capped at 10
        assert!((y.charge_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_change_when_not_charging() {
        let mut y = y();
        y.tick(5.0); // not charging
        assert_eq!(y.charge_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut y = Yell::new(10.0, 5.0, 0.5);
        y.enabled = false;
        y.charging = true; // force field (not via charge() which would no-op)
        y.tick(5.0);
        assert_eq!(y.charge_level, 0.0);
    }

    #[test]
    fn tick_clears_just_shouted() {
        let mut y = y();
        y.charge();
        y.tick(2.0);
        y.shout(); // just_shouted fires
        y.tick(0.016); // cleared
        assert!(!y.just_shouted);
    }

    #[test]
    fn shout_fires_just_shouted_when_charged() {
        let mut y = y();
        y.charge();
        y.tick(1.0); // some charge
        y.shout();
        assert!(y.just_shouted);
    }

    #[test]
    fn shout_resets_charge_level() {
        let mut y = y();
        y.charge();
        y.tick(1.0); // 5.0
        y.shout();
        assert_eq!(y.charge_level, 0.0);
    }

    #[test]
    fn shout_no_op_when_charge_zero() {
        let mut y = y();
        y.shout(); // no charge built
        assert!(!y.just_shouted);
        assert_eq!(y.charge_level, 0.0);
    }

    #[test]
    fn shout_no_op_when_disabled() {
        let mut y = y();
        y.charge();
        y.tick(1.0);
        y.enabled = false;
        y.shout();
        assert!(!y.just_shouted);
        assert!((y.charge_level - 5.0).abs() < 1e-4); // unchanged
    }

    #[test]
    fn is_charged_true_at_max() {
        let mut y = y();
        y.charge();
        y.tick(10.0);
        assert!(y.is_charged());
    }

    #[test]
    fn is_charged_false_below_max() {
        let mut y = y();
        y.charge();
        y.tick(1.0); // 5.0 < 10.0
        assert!(!y.is_charged());
    }

    #[test]
    fn is_charged_false_when_disabled() {
        let mut y = y();
        y.charge();
        y.tick(10.0);
        y.enabled = false;
        assert!(!y.is_charged());
    }

    #[test]
    fn charge_fraction_zero_when_empty() {
        let y = y();
        assert_eq!(y.charge_fraction(), 0.0);
    }

    #[test]
    fn charge_fraction_half_at_midpoint() {
        let mut y = y();
        y.charge();
        y.tick(1.0); // 5.0 / 10.0 = 0.5
        assert!((y.charge_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn charge_fraction_one_at_max() {
        let mut y = y();
        y.charge();
        y.tick(10.0);
        assert!((y.charge_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_attack_base_when_uncharged() {
        let y = Yell::new(10.0, 5.0, 0.5);
        assert!((y.effective_attack(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_attack_boosted_at_half_charge() {
        let mut y = Yell::new(10.0, 5.0, 0.5);
        y.charge();
        y.tick(1.0); // 5.0 → fraction 0.5
                     // 100 * (1 + 0.5*0.5) = 100 * 1.25 = 125
        assert!((y.effective_attack(100.0) - 125.0).abs() < 1e-3);
    }

    #[test]
    fn effective_attack_fully_boosted_at_max_charge() {
        let mut y = Yell::new(10.0, 5.0, 0.5);
        y.charge();
        y.tick(10.0); // max
                      // 100 * (1 + 0.5*1.0) = 150
        assert!((y.effective_attack(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_attack_passthrough_when_disabled() {
        let mut y = Yell::new(10.0, 5.0, 0.5);
        y.charge();
        y.tick(10.0);
        y.enabled = false;
        assert!((y.effective_attack(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_charge_clamped_to_one() {
        let y = Yell::new(0.0, 5.0, 0.5);
        assert!((y.max_charge - 1.0).abs() < 1e-5);
    }

    #[test]
    fn charge_rate_clamped_to_zero() {
        let y = Yell::new(10.0, -5.0, 0.5);
        assert_eq!(y.charge_rate, 0.0);
    }

    #[test]
    fn power_bonus_clamped_to_zero() {
        let y = Yell::new(10.0, 5.0, -1.0);
        assert_eq!(y.power_bonus, 0.0);
    }

    #[test]
    fn charge_cease_charge_again() {
        let mut y = y();
        y.charge();
        y.tick(1.0); // 5.0
        y.cease();
        y.tick(1.0); // not charging, stays at 5.0
        assert!((y.charge_level - 5.0).abs() < 1e-4);
        y.charge();
        y.tick(1.0); // 5.0 + 5.0 = 10.0
        assert!(y.is_charged());
    }

    #[test]
    fn shout_then_recharge_and_shout_again() {
        let mut y = y();
        y.charge();
        y.tick(10.0); // full
        y.shout(); // just_shouted, charge reset
        assert!(y.just_shouted);
        assert_eq!(y.charge_level, 0.0);
        y.tick(0.016); // clear
        y.tick(2.0); // 5.0 * 2.0 = 10.0 → max again (still charging)
        y.shout();
        assert!(y.just_shouted);
    }

    #[test]
    fn partial_charge_shout_still_fires() {
        let mut y = y();
        y.charge();
        y.tick(0.5); // 2.5, partial charge
        y.shout(); // fires at partial power
        assert!(y.just_shouted);
        assert_eq!(y.charge_level, 0.0);
    }

    #[test]
    fn effective_attack_before_shout_captures_full_power() {
        let mut y = Yell::new(10.0, 5.0, 1.0);
        y.charge();
        y.tick(10.0); // full — bonus = 100%
        let power = y.effective_attack(100.0); // 200.0
        y.shout(); // spend charge
        assert!((power - 200.0).abs() < 1e-3);
        // after shout, charge is 0 so effective_attack would return 100.0
        assert!((y.effective_attack(100.0) - 100.0).abs() < 1e-3);
    }
}
