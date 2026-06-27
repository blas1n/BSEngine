use bevy_ecs::prelude::Component;

/// Distance-gated bonus damage accumulator for piercing charge attacks. The
/// entity accumulates rush distance via `charge(dist)` during a dash or sprint;
/// when it strikes a target via `impact()`, the bonus damage scales linearly
/// with how far it charged relative to `full_charge_dist`. A longer charge
/// yields a larger hit.
///
/// `charge(dist)` adds to `current_charge` (capped at `full_charge_dist`).
/// No-op when disabled or `dist ≤ 0`.
///
/// `impact()` converts the accumulated charge into a bonus damage value
/// (`max_bonus * charge_fraction()`), fires `just_hit`, resets
/// `current_charge` to 0, and returns the bonus. Returns 0.0 and is a no-op
/// when disabled.
///
/// `reset()` clears `current_charge` without triggering an impact — use when
/// the charge is interrupted.
///
/// `tick()` clears `just_hit` each frame.
///
/// `charge_fraction()` returns `(current_charge / full_charge_dist).clamp(0, 1)`.
///
/// Distinct from `Lance` (timed charge window with a duration cooldown),
/// `Trample` (AoE crush that damages all entities in path), and `Momentum`
/// (speed/mass inertia tracker for physics): Tusk is a **distance-gated bonus
/// damage accumulator** — the farther the entity rushes before impact, the
/// larger the bonus hit, with no timer or cooldown involved.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Tusk {
    /// Accumulated rush distance in the current charge.
    pub current_charge: f32,
    /// Distance at which the maximum bonus is reached. Clamped > 0.
    pub full_charge_dist: f32,
    /// Maximum additive damage bonus at full charge. Clamped ≥ 0.
    pub max_bonus: f32,
    pub just_hit: bool,
    pub enabled: bool,
}

impl Tusk {
    pub fn new(max_bonus: f32, full_charge_dist: f32) -> Self {
        Self {
            current_charge: 0.0,
            full_charge_dist: full_charge_dist.max(0.001),
            max_bonus: max_bonus.max(0.0),
            just_hit: false,
            enabled: true,
        }
    }

    /// Accumulate rush distance toward the charge cap. No-op when disabled
    /// or `dist ≤ 0`.
    pub fn charge(&mut self, dist: f32) {
        if !self.enabled || dist <= 0.0 {
            return;
        }
        self.current_charge = (self.current_charge + dist).min(self.full_charge_dist);
    }

    /// Convert accumulated charge into bonus damage, fire `just_hit`, and
    /// reset `current_charge`. Returns `max_bonus * charge_fraction()`.
    /// Returns 0.0 and is a no-op when disabled.
    pub fn impact(&mut self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let bonus = self.max_bonus * self.charge_fraction();
        self.current_charge = 0.0;
        self.just_hit = true;
        bonus
    }

    /// Clear accumulated charge without firing an impact (interrupted rush).
    pub fn reset(&mut self) {
        self.current_charge = 0.0;
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_hit = false;
    }

    /// Charge fill fraction [0.0 = none, 1.0 = full charge].
    pub fn charge_fraction(&self) -> f32 {
        (self.current_charge / self.full_charge_dist).clamp(0.0, 1.0)
    }
}

impl Default for Tusk {
    fn default() -> Self {
        Self::new(50.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_uncharged() {
        let t = Tusk::new(50.0, 10.0);
        assert_eq!(t.current_charge, 0.0);
        assert!((t.charge_fraction()).abs() < 1e-5);
    }

    #[test]
    fn charge_accumulates() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(3.0);
        t.charge(2.0);
        assert!((t.current_charge - 5.0).abs() < 1e-5);
    }

    #[test]
    fn charge_caps_at_full_dist() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(15.0);
        assert!((t.current_charge - 10.0).abs() < 1e-5);
    }

    #[test]
    fn charge_no_op_when_disabled() {
        let mut t = Tusk::new(50.0, 10.0);
        t.enabled = false;
        t.charge(5.0);
        assert_eq!(t.current_charge, 0.0);
    }

    #[test]
    fn charge_no_op_at_zero_dist() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(0.0);
        assert_eq!(t.current_charge, 0.0);
    }

    #[test]
    fn impact_returns_proportional_bonus() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(5.0); // half charge
        let bonus = t.impact();
        // 50 * 0.5 = 25
        assert!((bonus - 25.0).abs() < 1e-3);
    }

    #[test]
    fn impact_full_charge_returns_max_bonus() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(10.0);
        let bonus = t.impact();
        assert!((bonus - 50.0).abs() < 1e-3);
    }

    #[test]
    fn impact_zero_charge_returns_zero() {
        let mut t = Tusk::new(50.0, 10.0);
        let bonus = t.impact();
        assert_eq!(bonus, 0.0);
    }

    #[test]
    fn impact_resets_current_charge() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(8.0);
        t.impact();
        assert_eq!(t.current_charge, 0.0);
    }

    #[test]
    fn impact_fires_just_hit() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(5.0);
        t.impact();
        assert!(t.just_hit);
    }

    #[test]
    fn impact_no_op_when_disabled() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(5.0);
        t.enabled = false;
        let bonus = t.impact();
        assert_eq!(bonus, 0.0);
        assert!(!t.just_hit);
        assert!((t.current_charge - 5.0).abs() < 1e-5);
    }

    #[test]
    fn reset_clears_charge_without_impact() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(8.0);
        t.reset();
        assert_eq!(t.current_charge, 0.0);
        assert!(!t.just_hit);
    }

    #[test]
    fn tick_clears_just_hit() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(5.0);
        t.impact();
        t.tick();
        assert!(!t.just_hit);
    }

    #[test]
    fn charge_fraction_at_zero() {
        let t = Tusk::new(50.0, 10.0);
        assert_eq!(t.charge_fraction(), 0.0);
    }

    #[test]
    fn charge_fraction_at_half() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(5.0);
        assert!((t.charge_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn charge_fraction_at_full() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(10.0);
        assert!((t.charge_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn charge_fraction_clamped_at_one() {
        let mut t = Tusk::new(50.0, 10.0);
        t.current_charge = 20.0; // manually over-set
        assert!((t.charge_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn can_charge_and_impact_multiple_times() {
        let mut t = Tusk::new(50.0, 10.0);
        t.charge(10.0);
        let b1 = t.impact();
        t.tick();
        t.charge(5.0);
        let b2 = t.impact();
        assert!((b1 - 50.0).abs() < 1e-3);
        assert!((b2 - 25.0).abs() < 1e-3);
    }

    #[test]
    fn max_bonus_clamped_at_zero() {
        let t = Tusk::new(-10.0, 5.0);
        assert_eq!(t.max_bonus, 0.0);
    }

    #[test]
    fn full_charge_dist_clamped_above_zero() {
        let t = Tusk::new(50.0, 0.0);
        assert!(t.full_charge_dist > 0.0);
    }

    #[test]
    fn zero_max_bonus_always_returns_zero() {
        let mut t = Tusk::new(0.0, 10.0);
        t.charge(10.0);
        let bonus = t.impact();
        assert_eq!(bonus, 0.0);
    }

    #[test]
    fn impact_at_quarter_charge() {
        let mut t = Tusk::new(100.0, 10.0);
        t.charge(2.5);
        let bonus = t.impact();
        // 100 * 0.25 = 25
        assert!((bonus - 25.0).abs() < 1e-3);
    }
}
