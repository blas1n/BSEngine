use bevy_ecs::prelude::Component;

/// Voltage-regulation tracker. `voltage` builds via `surge(amount)` and
/// charges passively at `charge_rate` per second in `tick(dt)` or
/// bleeds off immediately via `discharge(amount)`.
///
/// Models battery-charge gauges, electrical-power bars, energy-cell
/// meters, mana-surge trackers, shock-weapon charge levels, electric-
/// creature energy pools, or any mechanic where stored voltage builds
/// toward a critical breakdown point and can be instantly discharged.
///
/// `surge(amount)` adds voltage; fires `just_critical` when first
/// reaching `max_voltage`. No-op when disabled.
///
/// `discharge(amount)` reduces voltage immediately; fires `just_depleted`
/// when reaching 0. No-op when disabled or already depleted.
///
/// `tick(dt)` clears both flags, then charges voltage by
/// `charge_rate * dt` (capped at `max_voltage`). Fires `just_critical`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_critical()` returns `voltage >= max_voltage && enabled`.
///
/// `is_depleted()` returns `voltage == 0.0` (not gated by `enabled`).
///
/// `voltage_fraction()` returns `(voltage / max_voltage).clamp(0, 1)`.
///
/// `effective_power(scale)` returns `scale * voltage_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 9.0)` — charges at 9 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zener {
    pub voltage: f32,
    pub max_voltage: f32,
    pub charge_rate: f32,
    pub just_critical: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Zener {
    pub fn new(max_voltage: f32, charge_rate: f32) -> Self {
        Self {
            voltage: 0.0,
            max_voltage: max_voltage.max(0.1),
            charge_rate: charge_rate.max(0.0),
            just_critical: false,
            just_depleted: false,
            enabled: true,
        }
    }

    /// Add voltage; fires `just_critical` when first reaching max.
    /// No-op when disabled.
    pub fn surge(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.voltage < self.max_voltage;
        self.voltage = (self.voltage + amount).min(self.max_voltage);
        if was_below && self.voltage >= self.max_voltage {
            self.just_critical = true;
        }
    }

    /// Reduce voltage; fires `just_depleted` when reaching 0.
    /// No-op when disabled or already depleted.
    pub fn discharge(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.voltage <= 0.0 {
            return;
        }
        self.voltage = (self.voltage - amount).max(0.0);
        if self.voltage <= 0.0 {
            self.just_depleted = true;
        }
    }

    /// Clear flags, then charge voltage by `charge_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_critical = false;
        self.just_depleted = false;
        if self.enabled && self.charge_rate > 0.0 && self.voltage < self.max_voltage {
            let was_below = self.voltage < self.max_voltage;
            self.voltage = (self.voltage + self.charge_rate * dt).min(self.max_voltage);
            if was_below && self.voltage >= self.max_voltage {
                self.just_critical = true;
            }
        }
    }

    /// `true` when voltage is at maximum and component is enabled.
    pub fn is_critical(&self) -> bool {
        self.voltage >= self.max_voltage && self.enabled
    }

    /// `true` when voltage is 0 (not gated by `enabled`).
    pub fn is_depleted(&self) -> bool {
        self.voltage == 0.0
    }

    /// Fraction of maximum voltage [0.0, 1.0].
    pub fn voltage_fraction(&self) -> f32 {
        (self.voltage / self.max_voltage).clamp(0.0, 1.0)
    }

    /// Returns `scale * voltage_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_power(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.voltage_fraction()
    }
}

impl Default for Zener {
    fn default() -> Self {
        Self::new(100.0, 9.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zener {
        Zener::new(100.0, 9.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_depleted() {
        let z = z();
        assert_eq!(z.voltage, 0.0);
        assert!(z.is_depleted());
        assert!(!z.is_critical());
    }

    #[test]
    fn new_clamps_max_voltage() {
        let z = Zener::new(-5.0, 9.0);
        assert!((z.max_voltage - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_charge_rate() {
        let z = Zener::new(100.0, -3.0);
        assert_eq!(z.charge_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zener::default();
        assert!((z.max_voltage - 100.0).abs() < 1e-5);
        assert!((z.charge_rate - 9.0).abs() < 1e-5);
    }

    // --- surge ---

    #[test]
    fn surge_adds_voltage() {
        let mut z = z();
        z.surge(40.0);
        assert!((z.voltage - 40.0).abs() < 1e-3);
    }

    #[test]
    fn surge_clamps_at_max() {
        let mut z = z();
        z.surge(200.0);
        assert!((z.voltage - 100.0).abs() < 1e-3);
    }

    #[test]
    fn surge_fires_just_critical_at_max() {
        let mut z = z();
        z.surge(100.0);
        assert!(z.just_critical);
        assert!(z.is_critical());
    }

    #[test]
    fn surge_no_just_critical_when_already_at_max() {
        let mut z = z();
        z.voltage = 100.0;
        z.surge(10.0);
        assert!(!z.just_critical);
    }

    #[test]
    fn surge_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.surge(50.0);
        assert_eq!(z.voltage, 0.0);
    }

    #[test]
    fn surge_no_op_when_amount_zero() {
        let mut z = z();
        z.surge(0.0);
        assert_eq!(z.voltage, 0.0);
    }

    // --- discharge ---

    #[test]
    fn discharge_reduces_voltage() {
        let mut z = z();
        z.voltage = 60.0;
        z.discharge(20.0);
        assert!((z.voltage - 40.0).abs() < 1e-3);
    }

    #[test]
    fn discharge_clamps_at_zero() {
        let mut z = z();
        z.voltage = 30.0;
        z.discharge(200.0);
        assert_eq!(z.voltage, 0.0);
    }

    #[test]
    fn discharge_fires_just_depleted_at_zero() {
        let mut z = z();
        z.voltage = 30.0;
        z.discharge(30.0);
        assert!(z.just_depleted);
    }

    #[test]
    fn discharge_no_op_when_already_depleted() {
        let mut z = z();
        z.discharge(10.0);
        assert!(!z.just_depleted);
    }

    #[test]
    fn discharge_no_op_when_disabled() {
        let mut z = z();
        z.voltage = 50.0;
        z.enabled = false;
        z.discharge(50.0);
        assert!((z.voltage - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_charges_voltage() {
        let mut z = z(); // charge=9
        z.tick(1.0); // 0 + 9 = 9
        assert!((z.voltage - 9.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_critical_on_charge_to_max() {
        let mut z = Zener::new(100.0, 200.0);
        z.voltage = 95.0;
        z.tick(1.0);
        assert!(z.just_critical);
        assert!(z.is_critical());
    }

    #[test]
    fn tick_no_charge_when_already_at_max() {
        let mut z = z();
        z.voltage = 100.0;
        z.tick(1.0);
        assert!(!z.just_critical);
    }

    #[test]
    fn tick_no_charge_when_rate_zero() {
        let mut z = Zener::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.voltage, 0.0);
    }

    #[test]
    fn tick_no_charge_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.voltage, 0.0);
    }

    #[test]
    fn tick_clears_just_critical() {
        let mut z = Zener::new(100.0, 200.0);
        z.voltage = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_critical);
    }

    #[test]
    fn tick_clears_just_depleted() {
        let mut z = z();
        z.voltage = 10.0;
        z.discharge(10.0);
        z.tick(0.016);
        assert!(!z.just_depleted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // charge=9
        z.tick(4.0); // 9*4 = 36
        assert!((z.voltage - 36.0).abs() < 1e-3);
    }

    // --- is_critical / is_depleted ---

    #[test]
    fn is_critical_false_when_disabled() {
        let mut z = z();
        z.voltage = 100.0;
        z.enabled = false;
        assert!(!z.is_critical());
    }

    #[test]
    fn is_depleted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_depleted());
    }

    // --- voltage_fraction / effective_power ---

    #[test]
    fn voltage_fraction_zero_when_depleted() {
        assert_eq!(z().voltage_fraction(), 0.0);
    }

    #[test]
    fn voltage_fraction_half_at_midpoint() {
        let mut z = z();
        z.voltage = 50.0;
        assert!((z.voltage_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_power_zero_when_depleted() {
        assert_eq!(z().effective_power(100.0), 0.0);
    }

    #[test]
    fn effective_power_scales_with_voltage() {
        let mut z = z();
        z.voltage = 90.0;
        assert!((z.effective_power(100.0) - 90.0).abs() < 1e-3);
    }

    #[test]
    fn effective_power_zero_when_disabled() {
        let mut z = z();
        z.voltage = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_power(100.0), 0.0);
    }
}
