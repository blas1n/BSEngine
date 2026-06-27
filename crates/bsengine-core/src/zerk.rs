use bevy_ecs::prelude::Component;

/// Mechanical-lubrication tracker. `lubrication` builds via `grease(amount)`
/// and is restored passively at `flow_rate` per second in `tick(dt)` or
/// depleted immediately via `corrode(amount)`.
///
/// Models vehicle/weapon maintenance meters, mechanical-joint health bars,
/// clockwork-creature upkeep gauges, robot-oil-level indicators, gear-wear
/// trackers, or any mechanic where a mechanism needs periodic servicing to
/// stay in peak condition and degrades without it.
///
/// `grease(amount)` adds lubrication; fires `just_serviced` when first
/// reaching `max_lubrication`. No-op when disabled.
///
/// `corrode(amount)` reduces lubrication immediately; fires `just_seized`
/// when reaching 0. No-op when disabled or already seized.
///
/// `tick(dt)` clears both flags, then restores lubrication by
/// `flow_rate * dt` (capped at `max_lubrication`). Fires `just_serviced`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_serviced()` returns `lubrication >= max_lubrication && enabled`.
///
/// `is_seized()` returns `lubrication == 0.0` (not gated by `enabled`).
///
/// `lubrication_fraction()` returns `(lubrication / max_lubrication).clamp(0, 1)`.
///
/// `effective_efficiency(scale)` returns `scale * lubrication_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 3.0)` — flows at 3 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zerk {
    pub lubrication: f32,
    pub max_lubrication: f32,
    pub flow_rate: f32,
    pub just_serviced: bool,
    pub just_seized: bool,
    pub enabled: bool,
}

impl Zerk {
    pub fn new(max_lubrication: f32, flow_rate: f32) -> Self {
        Self {
            lubrication: 0.0,
            max_lubrication: max_lubrication.max(0.1),
            flow_rate: flow_rate.max(0.0),
            just_serviced: false,
            just_seized: false,
            enabled: true,
        }
    }

    /// Add lubrication; fires `just_serviced` when first reaching max.
    /// No-op when disabled.
    pub fn grease(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.lubrication < self.max_lubrication;
        self.lubrication = (self.lubrication + amount).min(self.max_lubrication);
        if was_below && self.lubrication >= self.max_lubrication {
            self.just_serviced = true;
        }
    }

    /// Reduce lubrication; fires `just_seized` when reaching 0.
    /// No-op when disabled or already seized.
    pub fn corrode(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.lubrication <= 0.0 {
            return;
        }
        self.lubrication = (self.lubrication - amount).max(0.0);
        if self.lubrication <= 0.0 {
            self.just_seized = true;
        }
    }

    /// Clear flags, then restore lubrication by `flow_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_serviced = false;
        self.just_seized = false;
        if self.enabled && self.flow_rate > 0.0 && self.lubrication < self.max_lubrication {
            let was_below = self.lubrication < self.max_lubrication;
            self.lubrication = (self.lubrication + self.flow_rate * dt).min(self.max_lubrication);
            if was_below && self.lubrication >= self.max_lubrication {
                self.just_serviced = true;
            }
        }
    }

    /// `true` when lubrication is at maximum and component is enabled.
    pub fn is_serviced(&self) -> bool {
        self.lubrication >= self.max_lubrication && self.enabled
    }

    /// `true` when lubrication is 0 (not gated by `enabled`).
    pub fn is_seized(&self) -> bool {
        self.lubrication == 0.0
    }

    /// Fraction of maximum lubrication [0.0, 1.0].
    pub fn lubrication_fraction(&self) -> f32 {
        (self.lubrication / self.max_lubrication).clamp(0.0, 1.0)
    }

    /// Returns `scale * lubrication_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_efficiency(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.lubrication_fraction()
    }
}

impl Default for Zerk {
    fn default() -> Self {
        Self::new(100.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zerk {
        Zerk::new(100.0, 3.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_seized() {
        let z = z();
        assert_eq!(z.lubrication, 0.0);
        assert!(z.is_seized());
        assert!(!z.is_serviced());
    }

    #[test]
    fn new_clamps_max_lubrication() {
        let z = Zerk::new(-5.0, 3.0);
        assert!((z.max_lubrication - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_flow_rate() {
        let z = Zerk::new(100.0, -3.0);
        assert_eq!(z.flow_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zerk::default();
        assert!((z.max_lubrication - 100.0).abs() < 1e-5);
        assert!((z.flow_rate - 3.0).abs() < 1e-5);
    }

    // --- grease ---

    #[test]
    fn grease_adds_lubrication() {
        let mut z = z();
        z.grease(40.0);
        assert!((z.lubrication - 40.0).abs() < 1e-3);
    }

    #[test]
    fn grease_clamps_at_max() {
        let mut z = z();
        z.grease(200.0);
        assert!((z.lubrication - 100.0).abs() < 1e-3);
    }

    #[test]
    fn grease_fires_just_serviced_at_max() {
        let mut z = z();
        z.grease(100.0);
        assert!(z.just_serviced);
        assert!(z.is_serviced());
    }

    #[test]
    fn grease_no_just_serviced_when_already_at_max() {
        let mut z = z();
        z.lubrication = 100.0;
        z.grease(10.0);
        assert!(!z.just_serviced);
    }

    #[test]
    fn grease_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.grease(50.0);
        assert_eq!(z.lubrication, 0.0);
    }

    #[test]
    fn grease_no_op_when_amount_zero() {
        let mut z = z();
        z.grease(0.0);
        assert_eq!(z.lubrication, 0.0);
    }

    // --- corrode ---

    #[test]
    fn corrode_reduces_lubrication() {
        let mut z = z();
        z.lubrication = 60.0;
        z.corrode(20.0);
        assert!((z.lubrication - 40.0).abs() < 1e-3);
    }

    #[test]
    fn corrode_clamps_at_zero() {
        let mut z = z();
        z.lubrication = 30.0;
        z.corrode(200.0);
        assert_eq!(z.lubrication, 0.0);
    }

    #[test]
    fn corrode_fires_just_seized_at_zero() {
        let mut z = z();
        z.lubrication = 30.0;
        z.corrode(30.0);
        assert!(z.just_seized);
    }

    #[test]
    fn corrode_no_op_when_already_seized() {
        let mut z = z();
        z.corrode(10.0);
        assert!(!z.just_seized);
    }

    #[test]
    fn corrode_no_op_when_disabled() {
        let mut z = z();
        z.lubrication = 50.0;
        z.enabled = false;
        z.corrode(50.0);
        assert!((z.lubrication - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_restores_lubrication() {
        let mut z = z(); // flow=3
        z.tick(1.0); // 0 + 3 = 3
        assert!((z.lubrication - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_serviced_on_flow_to_max() {
        let mut z = Zerk::new(100.0, 200.0);
        z.lubrication = 95.0;
        z.tick(1.0);
        assert!(z.just_serviced);
        assert!(z.is_serviced());
    }

    #[test]
    fn tick_no_flow_when_already_serviced() {
        let mut z = z();
        z.lubrication = 100.0;
        z.tick(1.0);
        assert!(!z.just_serviced);
    }

    #[test]
    fn tick_no_flow_when_rate_zero() {
        let mut z = Zerk::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.lubrication, 0.0);
    }

    #[test]
    fn tick_no_flow_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.lubrication, 0.0);
    }

    #[test]
    fn tick_clears_just_serviced() {
        let mut z = Zerk::new(100.0, 200.0);
        z.lubrication = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_serviced);
    }

    #[test]
    fn tick_clears_just_seized() {
        let mut z = z();
        z.lubrication = 10.0;
        z.corrode(10.0);
        z.tick(0.016);
        assert!(!z.just_seized);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // flow=3
        z.tick(6.0); // 3*6 = 18
        assert!((z.lubrication - 18.0).abs() < 1e-3);
    }

    // --- is_serviced / is_seized ---

    #[test]
    fn is_serviced_false_when_disabled() {
        let mut z = z();
        z.lubrication = 100.0;
        z.enabled = false;
        assert!(!z.is_serviced());
    }

    #[test]
    fn is_seized_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_seized());
    }

    // --- lubrication_fraction / effective_efficiency ---

    #[test]
    fn lubrication_fraction_zero_when_seized() {
        assert_eq!(z().lubrication_fraction(), 0.0);
    }

    #[test]
    fn lubrication_fraction_half_at_midpoint() {
        let mut z = z();
        z.lubrication = 50.0;
        assert!((z.lubrication_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_efficiency_zero_when_seized() {
        assert_eq!(z().effective_efficiency(100.0), 0.0);
    }

    #[test]
    fn effective_efficiency_scales_with_lubrication() {
        let mut z = z();
        z.lubrication = 80.0;
        assert!((z.effective_efficiency(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_efficiency_zero_when_disabled() {
        let mut z = z();
        z.lubrication = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_efficiency(100.0), 0.0);
    }
}
