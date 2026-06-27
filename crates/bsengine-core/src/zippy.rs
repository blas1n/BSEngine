use bevy_ecs::prelude::Component;

/// Liveliness-energy tracker. `pep` builds via `invigorate(amount)` and
/// perks up passively at `perk_rate` per second in `tick(dt)` or is
/// drained immediately via `tire(amount)`.
///
/// Models energy-drink charge bars, character-enthusiasm meters,
/// sprint-readiness gauges, party-morale trackers, caffeine-effect
/// fill levels, hype-train accumulators, crowd-excitement bars,
/// or any mechanic where accumulated liveliness amplifies the next
/// action and fades when the entity is exhausted.
///
/// `invigorate(amount)` adds pep; fires `just_peppy` when first
/// reaching `max_pep`. No-op when disabled.
///
/// `tire(amount)` reduces pep immediately; fires `just_tired` when
/// reaching 0. No-op when disabled or already tired.
///
/// `tick(dt)` clears both flags, then increases pep by
/// `perk_rate * dt` (capped at `max_pep`). Fires `just_peppy`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_peppy()` returns `pep >= max_pep && enabled`.
///
/// `is_tired()` returns `pep == 0.0` (not gated by `enabled`).
///
/// `pep_fraction()` returns `(pep / max_pep).clamp(0, 1)`.
///
/// `effective_energy(scale)` returns `scale * pep_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 6.0)` — perks up at 6 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zippy {
    pub pep: f32,
    pub max_pep: f32,
    pub perk_rate: f32,
    pub just_peppy: bool,
    pub just_tired: bool,
    pub enabled: bool,
}

impl Zippy {
    pub fn new(max_pep: f32, perk_rate: f32) -> Self {
        Self {
            pep: 0.0,
            max_pep: max_pep.max(0.1),
            perk_rate: perk_rate.max(0.0),
            just_peppy: false,
            just_tired: false,
            enabled: true,
        }
    }

    /// Add pep; fires `just_peppy` when first reaching max.
    /// No-op when disabled.
    pub fn invigorate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.pep < self.max_pep;
        self.pep = (self.pep + amount).min(self.max_pep);
        if was_below && self.pep >= self.max_pep {
            self.just_peppy = true;
        }
    }

    /// Reduce pep; fires `just_tired` when reaching 0.
    /// No-op when disabled or already tired.
    pub fn tire(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pep <= 0.0 {
            return;
        }
        self.pep = (self.pep - amount).max(0.0);
        if self.pep <= 0.0 {
            self.just_tired = true;
        }
    }

    /// Clear flags, then increase pep by `perk_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_peppy = false;
        self.just_tired = false;
        if self.enabled && self.perk_rate > 0.0 && self.pep < self.max_pep {
            let was_below = self.pep < self.max_pep;
            self.pep = (self.pep + self.perk_rate * dt).min(self.max_pep);
            if was_below && self.pep >= self.max_pep {
                self.just_peppy = true;
            }
        }
    }

    /// `true` when pep is at maximum and component is enabled.
    pub fn is_peppy(&self) -> bool {
        self.pep >= self.max_pep && self.enabled
    }

    /// `true` when pep is 0 (not gated by `enabled`).
    pub fn is_tired(&self) -> bool {
        self.pep == 0.0
    }

    /// Fraction of maximum pep [0.0, 1.0].
    pub fn pep_fraction(&self) -> f32 {
        (self.pep / self.max_pep).clamp(0.0, 1.0)
    }

    /// Returns `scale * pep_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_energy(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.pep_fraction()
    }
}

impl Default for Zippy {
    fn default() -> Self {
        Self::new(100.0, 6.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zippy {
        Zippy::new(100.0, 6.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_tired() {
        let z = z();
        assert_eq!(z.pep, 0.0);
        assert!(z.is_tired());
        assert!(!z.is_peppy());
    }

    #[test]
    fn new_clamps_max_pep() {
        let z = Zippy::new(-5.0, 6.0);
        assert!((z.max_pep - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_perk_rate() {
        let z = Zippy::new(100.0, -3.0);
        assert_eq!(z.perk_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zippy::default();
        assert!((z.max_pep - 100.0).abs() < 1e-5);
        assert!((z.perk_rate - 6.0).abs() < 1e-5);
    }

    // --- invigorate ---

    #[test]
    fn invigorate_adds_pep() {
        let mut z = z();
        z.invigorate(40.0);
        assert!((z.pep - 40.0).abs() < 1e-3);
    }

    #[test]
    fn invigorate_clamps_at_max() {
        let mut z = z();
        z.invigorate(200.0);
        assert!((z.pep - 100.0).abs() < 1e-3);
    }

    #[test]
    fn invigorate_fires_just_peppy_at_max() {
        let mut z = z();
        z.invigorate(100.0);
        assert!(z.just_peppy);
        assert!(z.is_peppy());
    }

    #[test]
    fn invigorate_no_just_peppy_when_already_at_max() {
        let mut z = z();
        z.pep = 100.0;
        z.invigorate(10.0);
        assert!(!z.just_peppy);
    }

    #[test]
    fn invigorate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.invigorate(50.0);
        assert_eq!(z.pep, 0.0);
    }

    #[test]
    fn invigorate_no_op_when_amount_zero() {
        let mut z = z();
        z.invigorate(0.0);
        assert_eq!(z.pep, 0.0);
    }

    // --- tire ---

    #[test]
    fn tire_reduces_pep() {
        let mut z = z();
        z.pep = 60.0;
        z.tire(20.0);
        assert!((z.pep - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tire_clamps_at_zero() {
        let mut z = z();
        z.pep = 30.0;
        z.tire(200.0);
        assert_eq!(z.pep, 0.0);
    }

    #[test]
    fn tire_fires_just_tired_at_zero() {
        let mut z = z();
        z.pep = 30.0;
        z.tire(30.0);
        assert!(z.just_tired);
    }

    #[test]
    fn tire_no_op_when_already_tired() {
        let mut z = z();
        z.tire(10.0);
        assert!(!z.just_tired);
    }

    #[test]
    fn tire_no_op_when_disabled() {
        let mut z = z();
        z.pep = 50.0;
        z.enabled = false;
        z.tire(50.0);
        assert!((z.pep - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_perks_up_pep() {
        let mut z = z(); // rate=6
        z.tick(1.0); // 0 + 6 = 6
        assert!((z.pep - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_peppy_on_perk_to_max() {
        let mut z = Zippy::new(100.0, 200.0);
        z.pep = 95.0;
        z.tick(1.0);
        assert!(z.just_peppy);
        assert!(z.is_peppy());
    }

    #[test]
    fn tick_no_perk_when_already_peppy() {
        let mut z = z();
        z.pep = 100.0;
        z.tick(1.0);
        assert!(!z.just_peppy);
    }

    #[test]
    fn tick_no_perk_when_rate_zero() {
        let mut z = Zippy::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.pep, 0.0);
    }

    #[test]
    fn tick_no_perk_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.pep, 0.0);
    }

    #[test]
    fn tick_clears_just_peppy() {
        let mut z = Zippy::new(100.0, 200.0);
        z.pep = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_peppy);
    }

    #[test]
    fn tick_clears_just_tired() {
        let mut z = z();
        z.pep = 10.0;
        z.tire(10.0);
        z.tick(0.016);
        assert!(!z.just_tired);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=6
        z.tick(3.0); // 6*3 = 18
        assert!((z.pep - 18.0).abs() < 1e-3);
    }

    // --- is_peppy / is_tired ---

    #[test]
    fn is_peppy_false_when_disabled() {
        let mut z = z();
        z.pep = 100.0;
        z.enabled = false;
        assert!(!z.is_peppy());
    }

    #[test]
    fn is_tired_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_tired());
    }

    // --- pep_fraction / effective_energy ---

    #[test]
    fn pep_fraction_zero_when_tired() {
        assert_eq!(z().pep_fraction(), 0.0);
    }

    #[test]
    fn pep_fraction_half_at_midpoint() {
        let mut z = z();
        z.pep = 50.0;
        assert!((z.pep_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_energy_zero_when_tired() {
        assert_eq!(z().effective_energy(100.0), 0.0);
    }

    #[test]
    fn effective_energy_scales_with_pep() {
        let mut z = z();
        z.pep = 80.0;
        assert!((z.effective_energy(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_energy_zero_when_disabled() {
        let mut z = z();
        z.pep = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_energy(100.0), 0.0);
    }
}
