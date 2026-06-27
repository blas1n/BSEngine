use bevy_ecs::prelude::Component;

/// Mineral/micronutrient tracker. `mineral` builds via `supplement(amount)`
/// and depletes via `deplete(amount)` or passive `metabolic_rate` per second
/// in `tick(dt)`.
///
/// Models survival-game micronutrient levels, environmental-resistance
/// buffs, trace-element resources, or any slowly-consumed stat that must
/// be actively replenished to stay at optimum.
///
/// `supplement(amount)` adds to `mineral`; fires `just_optimal` when first
/// reaching `max_mineral`. No-op when disabled.
///
/// `deplete(amount)` subtracts from `mineral`; fires `just_deficient` when
/// reaching 0. No-op when disabled or already deficient.
///
/// `tick(dt)` clears both flags, then metabolizes: `mineral -= metabolic_rate
/// * dt` (floored at 0). Fires `just_deficient` when reaching 0 via
/// metabolism. No-op metabolism when disabled or rate is 0.
///
/// `is_optimal()` returns `mineral >= max_mineral && enabled`.
///
/// `is_deficient()` returns `mineral == 0.0` (not gated by `enabled`).
///
/// `mineral_fraction()` returns `(mineral / max_mineral).clamp(0, 1)`.
///
/// `effective_resilience(base)` returns `base * mineral_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — metabolizes at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zinc {
    pub mineral: f32,
    pub max_mineral: f32,
    pub metabolic_rate: f32,
    pub just_optimal: bool,
    pub just_deficient: bool,
    pub enabled: bool,
}

impl Zinc {
    pub fn new(max_mineral: f32, metabolic_rate: f32) -> Self {
        Self {
            mineral: 0.0,
            max_mineral: max_mineral.max(0.1),
            metabolic_rate: metabolic_rate.max(0.0),
            just_optimal: false,
            just_deficient: false,
            enabled: true,
        }
    }

    /// Add to mineral; fires `just_optimal` when first reaching max.
    /// No-op when disabled.
    pub fn supplement(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.mineral < self.max_mineral;
        self.mineral = (self.mineral + amount).min(self.max_mineral);
        if was_below && self.mineral >= self.max_mineral {
            self.just_optimal = true;
        }
    }

    /// Subtract from mineral; fires `just_deficient` when reaching 0.
    /// No-op when disabled or already deficient.
    pub fn deplete(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.mineral <= 0.0 {
            return;
        }
        self.mineral = (self.mineral - amount).max(0.0);
        if self.mineral <= 0.0 {
            self.just_deficient = true;
        }
    }

    /// Clear flags, then metabolize mineral by `metabolic_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_optimal = false;
        self.just_deficient = false;
        if self.enabled && self.metabolic_rate > 0.0 && self.mineral > 0.0 {
            self.mineral = (self.mineral - self.metabolic_rate * dt).max(0.0);
            if self.mineral <= 0.0 {
                self.just_deficient = true;
            }
        }
    }

    /// `true` when mineral is at maximum and component is enabled.
    pub fn is_optimal(&self) -> bool {
        self.mineral >= self.max_mineral && self.enabled
    }

    /// `true` when mineral is 0 (not gated by `enabled`).
    pub fn is_deficient(&self) -> bool {
        self.mineral == 0.0
    }

    /// Fraction of maximum mineral [0.0, 1.0].
    pub fn mineral_fraction(&self) -> f32 {
        (self.mineral / self.max_mineral).clamp(0.0, 1.0)
    }

    /// Returns `base * mineral_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_resilience(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.mineral_fraction()
    }
}

impl Default for Zinc {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Zinc {
        Zinc::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_deficient() {
        let y = y();
        assert_eq!(y.mineral, 0.0);
        assert!(y.is_deficient());
        assert!(!y.is_optimal());
    }

    #[test]
    fn new_clamps_max_mineral() {
        let y = Zinc::new(-5.0, 1.0);
        assert!((y.max_mineral - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_metabolic_rate() {
        let y = Zinc::new(100.0, -2.0);
        assert_eq!(y.metabolic_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Zinc::default();
        assert!((y.max_mineral - 100.0).abs() < 1e-5);
        assert!((y.metabolic_rate - 1.0).abs() < 1e-5);
    }

    // --- supplement ---

    #[test]
    fn supplement_adds_mineral() {
        let mut y = y();
        y.supplement(40.0);
        assert!((y.mineral - 40.0).abs() < 1e-3);
    }

    #[test]
    fn supplement_clamps_at_max() {
        let mut y = y();
        y.supplement(200.0);
        assert!((y.mineral - 100.0).abs() < 1e-3);
    }

    #[test]
    fn supplement_fires_just_optimal_at_max() {
        let mut y = y();
        y.supplement(100.0);
        assert!(y.just_optimal);
        assert!(y.is_optimal());
    }

    #[test]
    fn supplement_no_just_optimal_when_already_at_max() {
        let mut y = y();
        y.mineral = 100.0;
        y.supplement(10.0);
        assert!(!y.just_optimal);
    }

    #[test]
    fn supplement_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.supplement(50.0);
        assert_eq!(y.mineral, 0.0);
    }

    #[test]
    fn supplement_no_op_when_amount_zero() {
        let mut y = y();
        y.supplement(0.0);
        assert_eq!(y.mineral, 0.0);
    }

    // --- deplete ---

    #[test]
    fn deplete_reduces_mineral() {
        let mut y = y();
        y.mineral = 60.0;
        y.deplete(20.0);
        assert!((y.mineral - 40.0).abs() < 1e-3);
    }

    #[test]
    fn deplete_clamps_at_zero() {
        let mut y = y();
        y.mineral = 30.0;
        y.deplete(200.0);
        assert_eq!(y.mineral, 0.0);
    }

    #[test]
    fn deplete_fires_just_deficient_at_zero() {
        let mut y = y();
        y.mineral = 30.0;
        y.deplete(30.0);
        assert!(y.just_deficient);
    }

    #[test]
    fn deplete_no_op_when_already_deficient() {
        let mut y = y();
        y.deplete(10.0);
        assert!(!y.just_deficient);
    }

    #[test]
    fn deplete_no_op_when_disabled() {
        let mut y = y();
        y.mineral = 50.0;
        y.enabled = false;
        y.deplete(50.0);
        assert!((y.mineral - 50.0).abs() < 1e-3);
    }

    // --- tick / metabolism ---

    #[test]
    fn tick_metabolizes_mineral() {
        let mut y = y(); // metabolic_rate=1
        y.mineral = 60.0;
        y.tick(1.0); // 60 - 1 = 59
        assert!((y.mineral - 59.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_deficient_on_metabolism_to_zero() {
        let mut y = Zinc::new(100.0, 200.0);
        y.mineral = 5.0;
        y.tick(1.0);
        assert!(y.just_deficient);
        assert!(y.is_deficient());
    }

    #[test]
    fn tick_no_metabolism_when_already_deficient() {
        let mut y = y();
        y.tick(10.0);
        assert!(!y.just_deficient);
    }

    #[test]
    fn tick_no_metabolism_when_rate_zero() {
        let mut y = Zinc::new(100.0, 0.0);
        y.mineral = 50.0;
        y.tick(100.0);
        assert!((y.mineral - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_metabolism_when_disabled() {
        let mut y = y();
        y.mineral = 50.0;
        y.enabled = false;
        y.tick(1.0);
        assert!((y.mineral - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_optimal() {
        let mut y = y();
        y.supplement(100.0); // just_optimal fires
        y.tick(0.016);
        assert!(!y.just_optimal);
    }

    #[test]
    fn tick_clears_just_deficient() {
        let mut y = Zinc::new(100.0, 200.0);
        y.mineral = 5.0;
        y.tick(1.0); // just_deficient fires
        y.tick(0.016);
        assert!(!y.just_deficient);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = Zinc::new(100.0, 5.0);
        y.mineral = 100.0;
        y.tick(2.0); // 100 - 5*2 = 90
        assert!((y.mineral - 90.0).abs() < 1e-3);
    }

    // --- is_optimal / is_deficient ---

    #[test]
    fn is_optimal_false_when_disabled() {
        let mut y = y();
        y.mineral = 100.0;
        y.enabled = false;
        assert!(!y.is_optimal());
    }

    #[test]
    fn is_deficient_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_deficient());
    }

    // --- mineral_fraction / effective_resilience ---

    #[test]
    fn mineral_fraction_zero_when_deficient() {
        assert_eq!(y().mineral_fraction(), 0.0);
    }

    #[test]
    fn mineral_fraction_half_at_midpoint() {
        let mut y = y();
        y.mineral = 50.0;
        assert!((y.mineral_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_resilience_zero_when_deficient() {
        assert_eq!(y().effective_resilience(100.0), 0.0);
    }

    #[test]
    fn effective_resilience_scales_with_mineral() {
        let mut y = y();
        y.mineral = 80.0;
        assert!((y.effective_resilience(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_resilience_zero_when_disabled() {
        let mut y = y();
        y.mineral = 50.0;
        y.enabled = false;
        assert_eq!(y.effective_resilience(100.0), 0.0);
    }
}
