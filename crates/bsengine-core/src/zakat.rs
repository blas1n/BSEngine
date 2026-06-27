use bevy_ecs::prelude::Component;

/// Obligatory-tithe saturation tracker. `tithe` builds via `give(amount)`
/// and accumulates passively at `accrue_rate` per second in `tick(dt)` or
/// is disbursed via `disburse(amount)`.
///
/// Models Islamic alms-tax obligation fill levels, charitable-giving quota
/// saturation trackers, obligatory-tithing build-up meters, wealth-
/// redistribution duty gauges, social-welfare contribution fill levels,
/// religious-tax obligation indicators, community-chest contribution bars,
/// pillar-of-faith duty saturation trackers, or any mechanic where a
/// steadily accumulating wealth-fraction is owed to the community and must
/// be periodically discharged to reset the obligation — with failure to
/// disburse resulting in compounding moral debt.
///
/// `give(amount)` adds tithe; fires `just_fulfilled` when first
/// reaching `max_tithe`. No-op when disabled.
///
/// `disburse(amount)` reduces tithe immediately; fires `just_discharged`
/// when reaching 0. No-op when disabled or already discharged.
///
/// `tick(dt)` clears both flags, then increases tithe by
/// `accrue_rate * dt` (capped at `max_tithe`). Fires `just_fulfilled`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_fulfilled()` returns `tithe >= max_tithe && enabled`.
///
/// `is_discharged()` returns `tithe == 0.0` (not gated by `enabled`).
///
/// `tithe_fraction()` returns `(tithe / max_tithe).clamp(0, 1)`.
///
/// `effective_generosity(scale)` returns `scale * tithe_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — accrues at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zakat {
    pub tithe: f32,
    pub max_tithe: f32,
    pub accrue_rate: f32,
    pub just_fulfilled: bool,
    pub just_discharged: bool,
    pub enabled: bool,
}

impl Zakat {
    pub fn new(max_tithe: f32, accrue_rate: f32) -> Self {
        Self {
            tithe: 0.0,
            max_tithe: max_tithe.max(0.1),
            accrue_rate: accrue_rate.max(0.0),
            just_fulfilled: false,
            just_discharged: false,
            enabled: true,
        }
    }

    /// Add tithe; fires `just_fulfilled` when first reaching max.
    /// No-op when disabled.
    pub fn give(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.tithe < self.max_tithe;
        self.tithe = (self.tithe + amount).min(self.max_tithe);
        if was_below && self.tithe >= self.max_tithe {
            self.just_fulfilled = true;
        }
    }

    /// Reduce tithe; fires `just_discharged` when reaching 0.
    /// No-op when disabled or already discharged.
    pub fn disburse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.tithe <= 0.0 {
            return;
        }
        self.tithe = (self.tithe - amount).max(0.0);
        if self.tithe <= 0.0 {
            self.just_discharged = true;
        }
    }

    /// Clear flags, then increase tithe by `accrue_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_fulfilled = false;
        self.just_discharged = false;
        if self.enabled && self.accrue_rate > 0.0 && self.tithe < self.max_tithe {
            let was_below = self.tithe < self.max_tithe;
            self.tithe = (self.tithe + self.accrue_rate * dt).min(self.max_tithe);
            if was_below && self.tithe >= self.max_tithe {
                self.just_fulfilled = true;
            }
        }
    }

    /// `true` when tithe is at maximum and component is enabled.
    pub fn is_fulfilled(&self) -> bool {
        self.tithe >= self.max_tithe && self.enabled
    }

    /// `true` when tithe is 0 (not gated by `enabled`).
    pub fn is_discharged(&self) -> bool {
        self.tithe == 0.0
    }

    /// Fraction of maximum tithe [0.0, 1.0].
    pub fn tithe_fraction(&self) -> f32 {
        (self.tithe / self.max_tithe).clamp(0.0, 1.0)
    }

    /// Returns `scale * tithe_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_generosity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.tithe_fraction()
    }
}

impl Default for Zakat {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zakat {
        Zakat::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_discharged() {
        let z = z();
        assert_eq!(z.tithe, 0.0);
        assert!(z.is_discharged());
        assert!(!z.is_fulfilled());
    }

    #[test]
    fn new_clamps_max_tithe() {
        let z = Zakat::new(-5.0, 1.0);
        assert!((z.max_tithe - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_accrue_rate() {
        let z = Zakat::new(100.0, -1.0);
        assert_eq!(z.accrue_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zakat::default();
        assert!((z.max_tithe - 100.0).abs() < 1e-5);
        assert!((z.accrue_rate - 1.0).abs() < 1e-5);
    }

    // --- give ---

    #[test]
    fn give_adds_tithe() {
        let mut z = z();
        z.give(40.0);
        assert!((z.tithe - 40.0).abs() < 1e-3);
    }

    #[test]
    fn give_clamps_at_max() {
        let mut z = z();
        z.give(200.0);
        assert!((z.tithe - 100.0).abs() < 1e-3);
    }

    #[test]
    fn give_fires_just_fulfilled_at_max() {
        let mut z = z();
        z.give(100.0);
        assert!(z.just_fulfilled);
        assert!(z.is_fulfilled());
    }

    #[test]
    fn give_no_just_fulfilled_when_already_at_max() {
        let mut z = z();
        z.tithe = 100.0;
        z.give(10.0);
        assert!(!z.just_fulfilled);
    }

    #[test]
    fn give_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.give(50.0);
        assert_eq!(z.tithe, 0.0);
    }

    #[test]
    fn give_no_op_when_amount_zero() {
        let mut z = z();
        z.give(0.0);
        assert_eq!(z.tithe, 0.0);
    }

    // --- disburse ---

    #[test]
    fn disburse_reduces_tithe() {
        let mut z = z();
        z.tithe = 60.0;
        z.disburse(20.0);
        assert!((z.tithe - 40.0).abs() < 1e-3);
    }

    #[test]
    fn disburse_clamps_at_zero() {
        let mut z = z();
        z.tithe = 30.0;
        z.disburse(200.0);
        assert_eq!(z.tithe, 0.0);
    }

    #[test]
    fn disburse_fires_just_discharged_at_zero() {
        let mut z = z();
        z.tithe = 30.0;
        z.disburse(30.0);
        assert!(z.just_discharged);
    }

    #[test]
    fn disburse_no_op_when_already_discharged() {
        let mut z = z();
        z.disburse(10.0);
        assert!(!z.just_discharged);
    }

    #[test]
    fn disburse_no_op_when_disabled() {
        let mut z = z();
        z.tithe = 50.0;
        z.enabled = false;
        z.disburse(50.0);
        assert!((z.tithe - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_accrues_tithe() {
        let mut z = z(); // rate=1
        z.tick(7.0); // 0 + 1*7 = 7
        assert!((z.tithe - 7.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_fulfilled_on_accrue_to_max() {
        let mut z = Zakat::new(100.0, 200.0);
        z.tithe = 95.0;
        z.tick(1.0);
        assert!(z.just_fulfilled);
        assert!(z.is_fulfilled());
    }

    #[test]
    fn tick_no_accrue_when_already_fulfilled() {
        let mut z = z();
        z.tithe = 100.0;
        z.tick(1.0);
        assert!(!z.just_fulfilled);
    }

    #[test]
    fn tick_no_accrue_when_rate_zero() {
        let mut z = Zakat::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.tithe, 0.0);
    }

    #[test]
    fn tick_no_accrue_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.tithe, 0.0);
    }

    #[test]
    fn tick_clears_just_fulfilled() {
        let mut z = Zakat::new(100.0, 200.0);
        z.tithe = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_fulfilled);
    }

    #[test]
    fn tick_clears_just_discharged() {
        let mut z = z();
        z.tithe = 10.0;
        z.disburse(10.0);
        z.tick(0.016);
        assert!(!z.just_discharged);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(9.0); // 1*9 = 9
        assert!((z.tithe - 9.0).abs() < 1e-3);
    }

    // --- is_fulfilled / is_discharged ---

    #[test]
    fn is_fulfilled_false_when_disabled() {
        let mut z = z();
        z.tithe = 100.0;
        z.enabled = false;
        assert!(!z.is_fulfilled());
    }

    #[test]
    fn is_discharged_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_discharged());
    }

    // --- tithe_fraction / effective_generosity ---

    #[test]
    fn tithe_fraction_zero_when_discharged() {
        assert_eq!(z().tithe_fraction(), 0.0);
    }

    #[test]
    fn tithe_fraction_half_at_midpoint() {
        let mut z = z();
        z.tithe = 50.0;
        assert!((z.tithe_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_generosity_zero_when_discharged() {
        assert_eq!(z().effective_generosity(100.0), 0.0);
    }

    #[test]
    fn effective_generosity_scales_with_tithe() {
        let mut z = z();
        z.tithe = 75.0;
        assert!((z.effective_generosity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_generosity_zero_when_disabled() {
        let mut z = z();
        z.tithe = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_generosity(100.0), 0.0);
    }
}
