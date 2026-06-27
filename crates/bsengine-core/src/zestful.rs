use bevy_ecs::prelude::Component;

/// Enthusiasm-energy tracker. `energy` builds via `invigorate(amount)` and
/// accumulates passively at `invigorate_rate` per second in `tick(dt)` or is
/// sapped immediately via `sap(amount)`.
///
/// Models party-spirit fill levels, excitement accumulation bars, crowd-hype
/// intensity gauges, morale-enthusiasm saturation trackers, festive-energy
/// build-up indicators, pep-rally momentum meters, theatrical-energy fill
/// levels, athletic-spirit escalation bars, adventure-eagerness trackers,
/// or any mechanic where a relentlessly upbeat attitude toward whatever
/// is happening charges the air with a kind of crackling potential that
/// makes everything seem achievable right up until the moment that
/// something difficult actually has to happen and the enthusiasm quietly
/// drains away to reveal the listless reality underneath.
///
/// `invigorate(amount)` adds energy; fires `just_zestful` when first
/// reaching `max_energy`. No-op when disabled.
///
/// `sap(amount)` reduces energy immediately; fires `just_listless`
/// when reaching 0. No-op when disabled or already listless.
///
/// `tick(dt)` clears both flags, then increases energy by
/// `invigorate_rate * dt` (capped at `max_energy`). Fires `just_zestful`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_zestful()` returns `energy >= max_energy && enabled`.
///
/// `is_listless()` returns `energy == 0.0` (not gated by `enabled`).
///
/// `energy_fraction()` returns `(energy / max_energy).clamp(0, 1)`.
///
/// `effective_enthusiasm(scale)` returns `scale * energy_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 3.0)` — invigorates at 3 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zestful {
    pub energy: f32,
    pub max_energy: f32,
    pub invigorate_rate: f32,
    pub just_zestful: bool,
    pub just_listless: bool,
    pub enabled: bool,
}

impl Zestful {
    pub fn new(max_energy: f32, invigorate_rate: f32) -> Self {
        Self {
            energy: 0.0,
            max_energy: max_energy.max(0.1),
            invigorate_rate: invigorate_rate.max(0.0),
            just_zestful: false,
            just_listless: false,
            enabled: true,
        }
    }

    /// Add energy; fires `just_zestful` when first reaching max.
    /// No-op when disabled.
    pub fn invigorate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.energy < self.max_energy;
        self.energy = (self.energy + amount).min(self.max_energy);
        if was_below && self.energy >= self.max_energy {
            self.just_zestful = true;
        }
    }

    /// Reduce energy; fires `just_listless` when reaching 0.
    /// No-op when disabled or already listless.
    pub fn sap(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.energy <= 0.0 {
            return;
        }
        self.energy = (self.energy - amount).max(0.0);
        if self.energy <= 0.0 {
            self.just_listless = true;
        }
    }

    /// Clear flags, then increase energy by `invigorate_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_zestful = false;
        self.just_listless = false;
        if self.enabled && self.invigorate_rate > 0.0 && self.energy < self.max_energy {
            let was_below = self.energy < self.max_energy;
            self.energy = (self.energy + self.invigorate_rate * dt).min(self.max_energy);
            if was_below && self.energy >= self.max_energy {
                self.just_zestful = true;
            }
        }
    }

    /// `true` when energy is at maximum and component is enabled.
    pub fn is_zestful(&self) -> bool {
        self.energy >= self.max_energy && self.enabled
    }

    /// `true` when energy is 0 (not gated by `enabled`).
    pub fn is_listless(&self) -> bool {
        self.energy == 0.0
    }

    /// Fraction of maximum energy [0.0, 1.0].
    pub fn energy_fraction(&self) -> f32 {
        (self.energy / self.max_energy).clamp(0.0, 1.0)
    }

    /// Returns `scale * energy_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_enthusiasm(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.energy_fraction()
    }
}

impl Default for Zestful {
    fn default() -> Self {
        Self::new(100.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zestful {
        Zestful::new(100.0, 3.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_listless() {
        let z = z();
        assert_eq!(z.energy, 0.0);
        assert!(z.is_listless());
        assert!(!z.is_zestful());
    }

    #[test]
    fn new_clamps_max_energy() {
        let z = Zestful::new(-5.0, 3.0);
        assert!((z.max_energy - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_invigorate_rate() {
        let z = Zestful::new(100.0, -3.0);
        assert_eq!(z.invigorate_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zestful::default();
        assert!((z.max_energy - 100.0).abs() < 1e-5);
        assert!((z.invigorate_rate - 3.0).abs() < 1e-5);
    }

    // --- invigorate ---

    #[test]
    fn invigorate_adds_energy() {
        let mut z = z();
        z.invigorate(40.0);
        assert!((z.energy - 40.0).abs() < 1e-3);
    }

    #[test]
    fn invigorate_clamps_at_max() {
        let mut z = z();
        z.invigorate(200.0);
        assert!((z.energy - 100.0).abs() < 1e-3);
    }

    #[test]
    fn invigorate_fires_just_zestful_at_max() {
        let mut z = z();
        z.invigorate(100.0);
        assert!(z.just_zestful);
        assert!(z.is_zestful());
    }

    #[test]
    fn invigorate_no_just_zestful_when_already_at_max() {
        let mut z = z();
        z.energy = 100.0;
        z.invigorate(10.0);
        assert!(!z.just_zestful);
    }

    #[test]
    fn invigorate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.invigorate(50.0);
        assert_eq!(z.energy, 0.0);
    }

    #[test]
    fn invigorate_no_op_when_amount_zero() {
        let mut z = z();
        z.invigorate(0.0);
        assert_eq!(z.energy, 0.0);
    }

    // --- sap ---

    #[test]
    fn sap_reduces_energy() {
        let mut z = z();
        z.energy = 60.0;
        z.sap(20.0);
        assert!((z.energy - 40.0).abs() < 1e-3);
    }

    #[test]
    fn sap_clamps_at_zero() {
        let mut z = z();
        z.energy = 30.0;
        z.sap(200.0);
        assert_eq!(z.energy, 0.0);
    }

    #[test]
    fn sap_fires_just_listless_at_zero() {
        let mut z = z();
        z.energy = 30.0;
        z.sap(30.0);
        assert!(z.just_listless);
    }

    #[test]
    fn sap_no_op_when_already_listless() {
        let mut z = z();
        z.sap(10.0);
        assert!(!z.just_listless);
    }

    #[test]
    fn sap_no_op_when_disabled() {
        let mut z = z();
        z.energy = 50.0;
        z.enabled = false;
        z.sap(50.0);
        assert!((z.energy - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_invigorates_energy() {
        let mut z = z(); // rate=3
        z.tick(2.0); // 0 + 3*2 = 6
        assert!((z.energy - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_zestful_on_invigorate_to_max() {
        let mut z = Zestful::new(100.0, 200.0);
        z.energy = 95.0;
        z.tick(1.0);
        assert!(z.just_zestful);
        assert!(z.is_zestful());
    }

    #[test]
    fn tick_no_invigorate_when_already_zestful() {
        let mut z = z();
        z.energy = 100.0;
        z.tick(1.0);
        assert!(!z.just_zestful);
    }

    #[test]
    fn tick_no_invigorate_when_rate_zero() {
        let mut z = Zestful::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.energy, 0.0);
    }

    #[test]
    fn tick_no_invigorate_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.energy, 0.0);
    }

    #[test]
    fn tick_clears_just_zestful() {
        let mut z = Zestful::new(100.0, 200.0);
        z.energy = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_zestful);
    }

    #[test]
    fn tick_clears_just_listless() {
        let mut z = z();
        z.energy = 10.0;
        z.sap(10.0);
        z.tick(0.016);
        assert!(!z.just_listless);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=3
        z.tick(4.0); // 3*4 = 12
        assert!((z.energy - 12.0).abs() < 1e-3);
    }

    // --- is_zestful / is_listless ---

    #[test]
    fn is_zestful_false_when_disabled() {
        let mut z = z();
        z.energy = 100.0;
        z.enabled = false;
        assert!(!z.is_zestful());
    }

    #[test]
    fn is_listless_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_listless());
    }

    // --- energy_fraction / effective_enthusiasm ---

    #[test]
    fn energy_fraction_zero_when_listless() {
        assert_eq!(z().energy_fraction(), 0.0);
    }

    #[test]
    fn energy_fraction_half_at_midpoint() {
        let mut z = z();
        z.energy = 50.0;
        assert!((z.energy_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_enthusiasm_zero_when_listless() {
        assert_eq!(z().effective_enthusiasm(100.0), 0.0);
    }

    #[test]
    fn effective_enthusiasm_scales_with_energy() {
        let mut z = z();
        z.energy = 75.0;
        assert!((z.effective_enthusiasm(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_enthusiasm_zero_when_disabled() {
        let mut z = z();
        z.energy = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_enthusiasm(100.0), 0.0);
    }
}
