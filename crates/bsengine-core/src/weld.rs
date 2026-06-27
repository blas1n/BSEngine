use bevy_ecs::prelude::Component;

/// Bond strength that builds while the entity is actively joining and
/// fractures passively once joining stops. Systems read `just_bonded` and
/// `weld_fraction()` to detect when a bond is fully formed and to scale
/// force transfer proportionally to current bond strength.
///
/// `weld()` begins active joining (`welding = true`). No-op when already
/// welding or disabled.
///
/// `cool()` stops active joining (`welding = false`). No-op when not welding.
///
/// `tick(dt)` clears one-frame flags, then:
/// - when `welding`: increases `strength` by `bond_rate * dt` (capped at
///   `max_strength`), firing `just_bonded` on first reach of `max_strength`;
/// - when `!welding`: decreases `strength` by `fracture_rate * dt` (floored
///   0), firing `just_fractured` on first drop to 0 from positive.
/// No-op when disabled.
///
/// `is_bonded()` returns `strength >= max_strength && enabled`.
///
/// `weld_fraction()` returns `(strength / max_strength).clamp(0.0, 1.0)`.
///
/// `effective_cohesion(base)` returns `base * weld_fraction()` when enabled;
/// returns `base` unchanged otherwise.
///
/// Distinct from `Web` (projectile adhesive stacking per hit), `Grapple`
/// (hook-and-pull force toward a target), `Latch` (binary attached/detached
/// toggle with no continuous build-up), and `Tether` (fixed maximum distance
/// constraint): Weld models **a continuously building adhesive bond** that
/// strengthens while actively joined and fractures gradually once the joining
/// force is removed, with proportional cohesion output at any strength level.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weld {
    /// Current bond strength [0.0, max_strength].
    pub strength: f32,
    /// Strength needed for a fully formed bond. Clamped >= 1.0.
    pub max_strength: f32,
    /// Bond gain per second while actively welding. Clamped >= 0.0.
    pub bond_rate: f32,
    /// Bond loss per second when not welding. Clamped >= 0.0.
    pub fracture_rate: f32,
    /// Whether the entity is actively forming a bond.
    pub welding: bool,
    pub just_bonded: bool,
    pub just_fractured: bool,
    pub enabled: bool,
}

impl Weld {
    pub fn new(max_strength: f32, bond_rate: f32, fracture_rate: f32) -> Self {
        Self {
            strength: 0.0,
            max_strength: max_strength.max(1.0),
            bond_rate: bond_rate.max(0.0),
            fracture_rate: fracture_rate.max(0.0),
            welding: false,
            just_bonded: false,
            just_fractured: false,
            enabled: true,
        }
    }

    /// Begin active joining. No-op when already welding or disabled.
    pub fn weld(&mut self) {
        if !self.enabled || self.welding {
            return;
        }
        self.welding = true;
    }

    /// Stop active joining. No-op when not welding.
    pub fn cool(&mut self) {
        if !self.welding {
            return;
        }
        self.welding = false;
    }

    /// Advance one frame: clear flags, then build or fracture bond. No-op when
    /// disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_bonded = false;
        self.just_fractured = false;

        if !self.enabled {
            return;
        }
        if self.welding {
            let was_below_max = self.strength < self.max_strength;
            self.strength = (self.strength + self.bond_rate * dt).min(self.max_strength);
            if was_below_max && self.strength >= self.max_strength {
                self.just_bonded = true;
            }
        } else if self.strength > 0.0 {
            let was_positive = self.strength > 0.0;
            self.strength = (self.strength - self.fracture_rate * dt).max(0.0);
            if was_positive && self.strength == 0.0 {
                self.just_fractured = true;
            }
        }
    }

    /// `true` when bond is fully formed and the component is enabled.
    pub fn is_bonded(&self) -> bool {
        self.strength >= self.max_strength && self.enabled
    }

    /// Bond strength as a fraction of maximum [0.0, 1.0].
    pub fn weld_fraction(&self) -> f32 {
        (self.strength / self.max_strength).clamp(0.0, 1.0)
    }

    /// Scale cohesion `base` by current bond strength. Returns
    /// `base * weld_fraction()` when enabled; `base` otherwise.
    pub fn effective_cohesion(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * self.weld_fraction()
    }
}

impl Default for Weld {
    fn default() -> Self {
        Self::new(10.0, 3.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Weld {
        Weld::new(10.0, 5.0, 2.0)
    }

    #[test]
    fn new_starts_unjoined() {
        let w = Weld::new(10.0, 5.0, 2.0);
        assert_eq!(w.strength, 0.0);
        assert!(!w.welding);
        assert!(!w.just_bonded);
        assert!(!w.just_fractured);
        assert!(!w.is_bonded());
    }

    #[test]
    fn weld_sets_welding() {
        let mut w = w();
        w.weld();
        assert!(w.welding);
    }

    #[test]
    fn weld_no_op_when_already_welding() {
        let mut w = w();
        w.weld();
        w.weld();
        assert!(w.welding);
    }

    #[test]
    fn weld_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.weld();
        assert!(!w.welding);
    }

    #[test]
    fn cool_clears_welding() {
        let mut w = w();
        w.weld();
        w.cool();
        assert!(!w.welding);
    }

    #[test]
    fn cool_no_op_when_not_welding() {
        let mut w = w();
        w.cool();
        assert!(!w.welding);
    }

    #[test]
    fn tick_builds_strength_when_welding() {
        let mut w = w(); // bond_rate = 5.0
        w.weld();
        w.tick(1.0); // 5.0 * 1.0 = 5.0
        assert!((w.strength - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max_strength() {
        let mut w = w();
        w.weld();
        w.tick(10.0); // 5.0 * 10 → capped at 10
        assert!((w.strength - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_bonded_on_first_max() {
        let mut w = w();
        w.weld();
        w.tick(10.0);
        assert!(w.just_bonded);
    }

    #[test]
    fn tick_no_just_bonded_when_already_at_max() {
        let mut w = w();
        w.weld();
        w.tick(10.0); // just_bonded fires
        w.tick(0.016); // clears; already at max, no re-fire
        assert!(!w.just_bonded);
    }

    #[test]
    fn tick_no_just_bonded_when_below_max() {
        let mut w = w();
        w.weld();
        w.tick(1.0); // 5.0, below max
        assert!(!w.just_bonded);
    }

    #[test]
    fn tick_fractures_when_not_welding() {
        let mut w = w(); // fracture_rate = 2.0
        w.weld();
        w.tick(2.0); // 10.0
        w.cool();
        w.tick(1.0); // 10.0 - 2.0 = 8.0
        assert!((w.strength - 8.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_at_zero_when_fracturing() {
        let mut w = w();
        w.weld();
        w.tick(1.0); // 5.0
        w.cool();
        w.tick(10.0); // 5.0 - 20.0 → floored 0
        assert_eq!(w.strength, 0.0);
    }

    #[test]
    fn tick_fires_just_fractured_on_first_zero() {
        let mut w = w();
        w.weld();
        w.tick(1.0); // 5.0
        w.cool();
        w.tick(10.0); // drops to 0
        assert!(w.just_fractured);
    }

    #[test]
    fn tick_no_just_fractured_when_already_at_zero() {
        let mut w = w();
        w.tick(1.0); // strength=0, !welding → no change
        assert!(!w.just_fractured);
    }

    #[test]
    fn tick_no_change_when_not_welding_and_zero() {
        let mut w = w();
        w.tick(5.0);
        assert_eq!(w.strength, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = Weld::new(10.0, 5.0, 2.0);
        w.enabled = false;
        w.welding = true; // bypass weld() guard
        w.tick(5.0);
        assert_eq!(w.strength, 0.0);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_bonded = true;
        w.just_fractured = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_bonded);
        assert!(!w.just_fractured);
    }

    #[test]
    fn is_bonded_true_at_max() {
        let mut w = w();
        w.weld();
        w.tick(10.0);
        assert!(w.is_bonded());
    }

    #[test]
    fn is_bonded_false_below_max() {
        let mut w = w();
        w.weld();
        w.tick(1.0); // 5.0 < 10.0
        assert!(!w.is_bonded());
    }

    #[test]
    fn is_bonded_false_when_disabled() {
        let mut w = w();
        w.weld();
        w.tick(10.0);
        w.enabled = false;
        assert!(!w.is_bonded());
    }

    #[test]
    fn weld_fraction_zero_when_no_strength() {
        let w = w();
        assert_eq!(w.weld_fraction(), 0.0);
    }

    #[test]
    fn weld_fraction_half_at_midpoint() {
        let mut w = w();
        w.weld();
        w.tick(1.0); // 5.0 / 10.0 = 0.5
        assert!((w.weld_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn weld_fraction_one_at_max() {
        let mut w = w();
        w.weld();
        w.tick(10.0);
        assert!((w.weld_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_cohesion_zero_when_no_bond() {
        let w = w();
        assert!((w.effective_cohesion(100.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn effective_cohesion_half_at_midpoint() {
        let mut w = w();
        w.weld();
        w.tick(1.0); // 5.0 → fraction 0.5
        assert!((w.effective_cohesion(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_cohesion_full_at_max_bond() {
        let mut w = w();
        w.weld();
        w.tick(10.0);
        assert!((w.effective_cohesion(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_cohesion_passthrough_when_disabled() {
        let mut w = w();
        w.weld();
        w.tick(10.0);
        w.enabled = false;
        assert!((w.effective_cohesion(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_strength_clamped_to_one() {
        let w = Weld::new(0.0, 5.0, 2.0);
        assert!((w.max_strength - 1.0).abs() < 1e-5);
    }

    #[test]
    fn bond_rate_clamped_to_zero() {
        let w = Weld::new(10.0, -5.0, 2.0);
        assert_eq!(w.bond_rate, 0.0);
    }

    #[test]
    fn fracture_rate_clamped_to_zero() {
        let w = Weld::new(10.0, 5.0, -2.0);
        assert_eq!(w.fracture_rate, 0.0);
    }

    #[test]
    fn weld_cool_weld_again_resumes_building() {
        let mut w = w();
        w.weld();
        w.tick(1.0); // 5.0
        w.cool();
        w.tick(0.5); // 5.0 - 2.0*0.5 = 4.0
        w.weld();
        w.tick(1.0); // 4.0 + 5.0 = 9.0
        assert!((w.strength - 9.0).abs() < 1e-4);
    }

    #[test]
    fn bond_fracture_bond_cycle() {
        let mut w = w();
        w.weld();
        w.tick(2.0); // 10.0, just_bonded
        assert!(w.just_bonded);
        w.cool();
        w.tick(1.0); // 8.0
        w.tick(0.016); // 8.0 - 2.0*0.016 = 7.968
        w.weld();
        w.tick(1.0); // 7.968 + 5.0 = 12.968 → capped 10.0, just_bonded
        assert!(w.just_bonded);
        assert!(w.is_bonded());
    }

    #[test]
    fn full_fracture_cycle() {
        let mut w = w();
        w.weld();
        w.tick(1.0); // 5.0
        w.cool();
        w.tick(2.5); // 5.0 - 5.0 = 0.0 → just_fractured
        assert!(w.just_fractured);
        assert_eq!(w.strength, 0.0);
        w.tick(0.016); // clears just_fractured
        assert!(!w.just_fractured);
    }

    #[test]
    fn partial_bond_still_transfers_cohesion() {
        let mut w = w();
        w.weld();
        w.tick(0.4); // 5.0 * 0.4 = 2.0 → fraction 0.2
                     // 0.2 * 50.0 = 10.0
        assert!((w.effective_cohesion(50.0) - 10.0).abs() < 1e-3);
    }
}
