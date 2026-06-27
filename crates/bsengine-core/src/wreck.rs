use bevy_ecs::prelude::Component;

/// Accumulated structural damage that degrades an entity's operational
/// effectiveness. Suitable for vehicles, ships, buildings, machines, or any
/// entity whose performance should degrade under sustained punishment.
///
/// `damage(amount)` increases `wreck_level` (capped at `max_wreck`). Fires
/// `just_wrecked` on the first reach of `max_wreck`. No-op when disabled or
/// `amount <= 0`.
///
/// `repair(amount)` decreases `wreck_level` (floored 0). Fires
/// `just_restored` when `wreck_level` drops below `max_wreck` from the
/// wrecked state. No-op when disabled or `amount <= 0`.
///
/// `tick(dt)` clears one-frame flags then applies passive structural
/// degradation: increases `wreck_level` by `decay_rate * dt` (capped at
/// `max_wreck`), firing `just_wrecked` on first reach. No-op when disabled.
///
/// `is_wrecked()` returns `wreck_level >= max_wreck && enabled`.
///
/// `wreck_fraction()` returns `(wreck_level / max_wreck).clamp(0.0, 1.0)`.
///
/// `effective_output(base)` returns
/// `(base * (1.0 - performance_penalty * wreck_fraction())).max(0.0)` when
/// enabled; returns `base` unchanged otherwise.
///
/// Distinct from `Fatigue`/`Exhaustion` (biological depletion of stamina),
/// `Durability` (item wear track), `Fracture` (brittle single-break),
/// `Corrupt` (data/magic corruption), and `Health` (vital points): Wreck
/// models **accumulated structural deterioration** of mechanical or
/// architectural entities, where partial damage continuously reduces
/// operational throughput until repaired.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wreck {
    /// Current structural damage level [0.0, max_wreck].
    pub wreck_level: f32,
    /// Maximum damage before the entity is fully wrecked. Clamped >= 1.0.
    pub max_wreck: f32,
    /// Passive degradation per second (environmental wear). Clamped >= 0.0.
    pub decay_rate: f32,
    /// Output reduction at full wreck [0.0, 1.0]. Clamped.
    pub performance_penalty: f32,
    pub just_wrecked: bool,
    pub just_restored: bool,
    pub enabled: bool,
}

impl Wreck {
    pub fn new(max_wreck: f32, decay_rate: f32, performance_penalty: f32) -> Self {
        Self {
            wreck_level: 0.0,
            max_wreck: max_wreck.max(1.0),
            decay_rate: decay_rate.max(0.0),
            performance_penalty: performance_penalty.clamp(0.0, 1.0),
            just_wrecked: false,
            just_restored: false,
            enabled: true,
        }
    }

    /// Apply structural damage. Fires `just_wrecked` on first reach of
    /// `max_wreck`. No-op when disabled or `amount <= 0`.
    pub fn damage(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below_max = self.wreck_level < self.max_wreck;
        self.wreck_level = (self.wreck_level + amount).min(self.max_wreck);
        if was_below_max && self.wreck_level >= self.max_wreck {
            self.just_wrecked = true;
        }
    }

    /// Repair structural damage. Fires `just_restored` when `wreck_level`
    /// drops below `max_wreck` from a fully-wrecked state. No-op when
    /// disabled or `amount <= 0`.
    pub fn repair(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_wrecked = self.wreck_level >= self.max_wreck;
        self.wreck_level = (self.wreck_level - amount).max(0.0);
        if was_wrecked && self.wreck_level < self.max_wreck {
            self.just_restored = true;
        }
    }

    /// Advance one frame: clear flags, then apply passive wear (`decay_rate *
    /// dt`). No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_wrecked = false;
        self.just_restored = false;

        if !self.enabled {
            return;
        }
        if self.decay_rate > 0.0 {
            let was_below_max = self.wreck_level < self.max_wreck;
            self.wreck_level = (self.wreck_level + self.decay_rate * dt).min(self.max_wreck);
            if was_below_max && self.wreck_level >= self.max_wreck {
                self.just_wrecked = true;
            }
        }
    }

    /// `true` when structural damage is at maximum and the component is
    /// enabled.
    pub fn is_wrecked(&self) -> bool {
        self.wreck_level >= self.max_wreck && self.enabled
    }

    /// Structural damage as a fraction of maximum [0.0, 1.0].
    pub fn wreck_fraction(&self) -> f32 {
        (self.wreck_level / self.max_wreck).clamp(0.0, 1.0)
    }

    /// Scale `base` by remaining operational capacity. Returns
    /// `(base * (1 - penalty * fraction)).max(0)` when enabled; `base`
    /// otherwise.
    pub fn effective_output(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.performance_penalty * self.wreck_fraction())).max(0.0)
    }
}

impl Default for Wreck {
    fn default() -> Self {
        Self::new(10.0, 0.1, 0.7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wreck {
        Wreck::new(10.0, 0.0, 0.5)
    }

    #[test]
    fn new_starts_undamaged() {
        let w = Wreck::new(10.0, 0.0, 0.5);
        assert_eq!(w.wreck_level, 0.0);
        assert!(!w.is_wrecked());
        assert!(!w.just_wrecked);
    }

    #[test]
    fn damage_increases_wreck_level() {
        let mut w = w();
        w.damage(4.0);
        assert!((w.wreck_level - 4.0).abs() < 1e-5);
    }

    #[test]
    fn damage_caps_at_max_wreck() {
        let mut w = w();
        w.damage(20.0);
        assert!((w.wreck_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn damage_fires_just_wrecked_on_first_reach() {
        let mut w = w();
        w.damage(10.0);
        assert!(w.just_wrecked);
        assert!(w.is_wrecked());
    }

    #[test]
    fn damage_no_just_wrecked_when_already_wrecked() {
        let mut w = w();
        w.damage(10.0); // just_wrecked fires
        w.tick(0.016); // clear
        w.damage(1.0); // already wrecked, no re-fire
        assert!(!w.just_wrecked);
    }

    #[test]
    fn damage_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.damage(5.0);
        assert_eq!(w.wreck_level, 0.0);
    }

    #[test]
    fn damage_no_op_when_amount_zero() {
        let mut w = w();
        w.damage(0.0);
        assert_eq!(w.wreck_level, 0.0);
    }

    #[test]
    fn damage_no_op_when_amount_negative() {
        let mut w = w();
        w.damage(-5.0);
        assert_eq!(w.wreck_level, 0.0);
    }

    #[test]
    fn repair_decreases_wreck_level() {
        let mut w = w();
        w.damage(8.0);
        w.repair(3.0);
        assert!((w.wreck_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn repair_floors_at_zero() {
        let mut w = w();
        w.damage(3.0);
        w.repair(10.0);
        assert_eq!(w.wreck_level, 0.0);
    }

    #[test]
    fn repair_fires_just_restored_from_wrecked() {
        let mut w = w();
        w.damage(10.0); // fully wrecked
        w.tick(0.016); // clear just_wrecked
        w.repair(1.0); // drops below max
        assert!(w.just_restored);
        assert!(!w.is_wrecked());
    }

    #[test]
    fn repair_no_just_restored_when_not_wrecked() {
        let mut w = w();
        w.damage(5.0); // partial damage
        w.tick(0.016);
        w.repair(2.0); // still not from wrecked state
        assert!(!w.just_restored);
    }

    #[test]
    fn repair_no_op_when_disabled() {
        let mut w = w();
        w.damage(6.0);
        w.enabled = false;
        w.repair(3.0);
        assert!((w.wreck_level - 6.0).abs() < 1e-5);
    }

    #[test]
    fn repair_no_op_when_amount_zero() {
        let mut w = w();
        w.damage(5.0);
        w.repair(0.0);
        assert!((w.wreck_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn repair_no_op_when_amount_negative() {
        let mut w = w();
        w.damage(5.0);
        w.repair(-1.0);
        assert!((w.wreck_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_wrecked() {
        let mut w = w();
        w.damage(10.0);
        w.tick(0.016);
        assert!(!w.just_wrecked);
    }

    #[test]
    fn tick_clears_just_restored() {
        let mut w = w();
        w.damage(10.0);
        w.tick(0.016);
        w.repair(1.0);
        w.tick(0.016);
        assert!(!w.just_restored);
    }

    #[test]
    fn tick_applies_passive_decay() {
        let mut w = Wreck::new(10.0, 2.0, 0.5);
        w.tick(1.0); // 2.0 * 1.0 = 2.0
        assert!((w.wreck_level - 2.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_passive_decay_at_max() {
        let mut w = Wreck::new(10.0, 20.0, 0.5);
        w.tick(1.0);
        assert!((w.wreck_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_wrecked_via_decay() {
        let mut w = Wreck::new(10.0, 20.0, 0.5);
        w.tick(1.0); // decay pushes to max
        assert!(w.just_wrecked);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = Wreck::new(10.0, 5.0, 0.5);
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.wreck_level, 0.0);
    }

    #[test]
    fn tick_zero_decay_no_increase() {
        let mut w = Wreck::new(10.0, 0.0, 0.5);
        w.damage(3.0);
        w.tick(1.0);
        assert!((w.wreck_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn is_wrecked_true_at_max() {
        let mut w = w();
        w.damage(10.0);
        assert!(w.is_wrecked());
    }

    #[test]
    fn is_wrecked_false_below_max() {
        let mut w = w();
        w.damage(9.9);
        assert!(!w.is_wrecked());
    }

    #[test]
    fn is_wrecked_false_when_disabled() {
        let mut w = w();
        w.damage(10.0);
        w.enabled = false;
        assert!(!w.is_wrecked());
    }

    #[test]
    fn wreck_fraction_zero_when_undamaged() {
        let w = w();
        assert_eq!(w.wreck_fraction(), 0.0);
    }

    #[test]
    fn wreck_fraction_half_at_midpoint() {
        let mut w = w();
        w.damage(5.0);
        assert!((w.wreck_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn wreck_fraction_one_at_max() {
        let mut w = w();
        w.damage(10.0);
        assert!((w.wreck_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_output_unpenalized_when_undamaged() {
        let w = Wreck::new(10.0, 0.0, 0.5);
        assert!((w.effective_output(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_output_half_penalty_at_full_wreck_with_0_5_penalty() {
        let mut w = Wreck::new(10.0, 0.0, 0.5);
        w.damage(10.0);
        assert!((w.effective_output(100.0) - 50.0).abs() < 1e-4);
    }

    #[test]
    fn effective_output_zero_penalty_at_full_wreck_with_1_0_penalty() {
        let mut w = Wreck::new(10.0, 0.0, 1.0);
        w.damage(10.0);
        assert!((w.effective_output(100.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn effective_output_no_penalty_scaled_at_half_wreck() {
        let mut w = Wreck::new(10.0, 0.0, 0.5);
        w.damage(5.0); // fraction = 0.5
                       // 100 * (1 - 0.5*0.5) = 100 * 0.75 = 75
        assert!((w.effective_output(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_passthrough_when_disabled() {
        let mut w = Wreck::new(10.0, 0.0, 1.0);
        w.damage(10.0);
        w.enabled = false;
        assert!((w.effective_output(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_output_floored_at_zero() {
        let mut w = Wreck::new(10.0, 0.0, 1.0);
        w.damage(10.0);
        assert!(w.effective_output(100.0) >= 0.0);
    }

    #[test]
    fn max_wreck_clamped_to_one() {
        let w = Wreck::new(0.0, 0.0, 0.5);
        assert!((w.max_wreck - 1.0).abs() < 1e-5);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let w = Wreck::new(10.0, -5.0, 0.5);
        assert_eq!(w.decay_rate, 0.0);
    }

    #[test]
    fn performance_penalty_clamped() {
        let w1 = Wreck::new(10.0, 0.0, 2.0);
        let w2 = Wreck::new(10.0, 0.0, -1.0);
        assert!((w1.performance_penalty - 1.0).abs() < 1e-5);
        assert_eq!(w2.performance_penalty, 0.0);
    }

    #[test]
    fn damage_repair_cycle() {
        let mut w = w();
        w.damage(10.0); // just_wrecked
        assert!(w.just_wrecked);
        w.tick(0.016);
        w.repair(10.0); // just_restored
        assert!(w.just_restored);
        w.tick(0.016);
        w.damage(5.0); // partial damage again
        assert!(!w.just_wrecked);
        assert!((w.wreck_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn decay_then_manual_damage_accumulates() {
        let mut w = Wreck::new(10.0, 1.0, 0.5);
        w.tick(2.0); // decay: 2.0
        w.damage(3.0); // total: 5.0
        assert!((w.wreck_level - 5.0).abs() < 1e-4);
    }
}
