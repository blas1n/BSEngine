use bevy_ecs::prelude::Component;

/// Pelt-quality tracker. `pelt` builds via `harvest(amount)` and
/// thickens passively at `tract_rate` per second in `tick(dt)` or
/// is depleted immediately via `deplete(amount)`.
///
/// Models South-American-skunk pelt-quality bars, fur-farm
/// accumulation trackers, trapping-yield fill levels, hide-curing
/// progress gauges, softening-treatment saturation indicators,
/// tanning-cure strength meters, felting-fiber density trackers,
/// wool-clip harvest intensity bars, or any mechanic where slow
/// careful husbandry builds a prime pelt worth everything until a
/// harsh season strips it down to nothing.
///
/// `harvest(amount)` adds pelt; fires `just_prime` when first
/// reaching `max_pelt`. No-op when disabled.
///
/// `deplete(amount)` reduces pelt immediately; fires `just_bare`
/// when reaching 0. No-op when disabled or already bare.
///
/// `tick(dt)` clears both flags, then increases pelt by
/// `tract_rate * dt` (capped at `max_pelt`). Fires `just_prime`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_prime()` returns `pelt >= max_pelt && enabled`.
///
/// `is_bare()` returns `pelt == 0.0` (not gated by `enabled`).
///
/// `pelt_fraction()` returns `(pelt / max_pelt).clamp(0, 1)`.
///
/// `effective_softness(scale)` returns `scale * pelt_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — thickens at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zorino {
    pub pelt: f32,
    pub max_pelt: f32,
    pub tract_rate: f32,
    pub just_prime: bool,
    pub just_bare: bool,
    pub enabled: bool,
}

impl Zorino {
    pub fn new(max_pelt: f32, tract_rate: f32) -> Self {
        Self {
            pelt: 0.0,
            max_pelt: max_pelt.max(0.1),
            tract_rate: tract_rate.max(0.0),
            just_prime: false,
            just_bare: false,
            enabled: true,
        }
    }

    /// Add pelt; fires `just_prime` when first reaching max.
    /// No-op when disabled.
    pub fn harvest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.pelt < self.max_pelt;
        self.pelt = (self.pelt + amount).min(self.max_pelt);
        if was_below && self.pelt >= self.max_pelt {
            self.just_prime = true;
        }
    }

    /// Reduce pelt; fires `just_bare` when reaching 0.
    /// No-op when disabled or already bare.
    pub fn deplete(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pelt <= 0.0 {
            return;
        }
        self.pelt = (self.pelt - amount).max(0.0);
        if self.pelt <= 0.0 {
            self.just_bare = true;
        }
    }

    /// Clear flags, then increase pelt by `tract_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_prime = false;
        self.just_bare = false;
        if self.enabled && self.tract_rate > 0.0 && self.pelt < self.max_pelt {
            let was_below = self.pelt < self.max_pelt;
            self.pelt = (self.pelt + self.tract_rate * dt).min(self.max_pelt);
            if was_below && self.pelt >= self.max_pelt {
                self.just_prime = true;
            }
        }
    }

    /// `true` when pelt is at maximum and component is enabled.
    pub fn is_prime(&self) -> bool {
        self.pelt >= self.max_pelt && self.enabled
    }

    /// `true` when pelt is 0 (not gated by `enabled`).
    pub fn is_bare(&self) -> bool {
        self.pelt == 0.0
    }

    /// Fraction of maximum pelt [0.0, 1.0].
    pub fn pelt_fraction(&self) -> f32 {
        (self.pelt / self.max_pelt).clamp(0.0, 1.0)
    }

    /// Returns `scale * pelt_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_softness(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.pelt_fraction()
    }
}

impl Default for Zorino {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zorino {
        Zorino::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_bare() {
        let z = z();
        assert_eq!(z.pelt, 0.0);
        assert!(z.is_bare());
        assert!(!z.is_prime());
    }

    #[test]
    fn new_clamps_max_pelt() {
        let z = Zorino::new(-5.0, 2.0);
        assert!((z.max_pelt - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_tract_rate() {
        let z = Zorino::new(100.0, -3.0);
        assert_eq!(z.tract_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zorino::default();
        assert!((z.max_pelt - 100.0).abs() < 1e-5);
        assert!((z.tract_rate - 2.0).abs() < 1e-5);
    }

    // --- harvest ---

    #[test]
    fn harvest_adds_pelt() {
        let mut z = z();
        z.harvest(40.0);
        assert!((z.pelt - 40.0).abs() < 1e-3);
    }

    #[test]
    fn harvest_clamps_at_max() {
        let mut z = z();
        z.harvest(200.0);
        assert!((z.pelt - 100.0).abs() < 1e-3);
    }

    #[test]
    fn harvest_fires_just_prime_at_max() {
        let mut z = z();
        z.harvest(100.0);
        assert!(z.just_prime);
        assert!(z.is_prime());
    }

    #[test]
    fn harvest_no_just_prime_when_already_at_max() {
        let mut z = z();
        z.pelt = 100.0;
        z.harvest(10.0);
        assert!(!z.just_prime);
    }

    #[test]
    fn harvest_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.harvest(50.0);
        assert_eq!(z.pelt, 0.0);
    }

    #[test]
    fn harvest_no_op_when_amount_zero() {
        let mut z = z();
        z.harvest(0.0);
        assert_eq!(z.pelt, 0.0);
    }

    // --- deplete ---

    #[test]
    fn deplete_reduces_pelt() {
        let mut z = z();
        z.pelt = 60.0;
        z.deplete(20.0);
        assert!((z.pelt - 40.0).abs() < 1e-3);
    }

    #[test]
    fn deplete_clamps_at_zero() {
        let mut z = z();
        z.pelt = 30.0;
        z.deplete(200.0);
        assert_eq!(z.pelt, 0.0);
    }

    #[test]
    fn deplete_fires_just_bare_at_zero() {
        let mut z = z();
        z.pelt = 30.0;
        z.deplete(30.0);
        assert!(z.just_bare);
    }

    #[test]
    fn deplete_no_op_when_already_bare() {
        let mut z = z();
        z.deplete(10.0);
        assert!(!z.just_bare);
    }

    #[test]
    fn deplete_no_op_when_disabled() {
        let mut z = z();
        z.pelt = 50.0;
        z.enabled = false;
        z.deplete(50.0);
        assert!((z.pelt - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_thickens_pelt() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.pelt - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_prime_on_thicken_to_max() {
        let mut z = Zorino::new(100.0, 200.0);
        z.pelt = 95.0;
        z.tick(1.0);
        assert!(z.just_prime);
        assert!(z.is_prime());
    }

    #[test]
    fn tick_no_thicken_when_already_prime() {
        let mut z = z();
        z.pelt = 100.0;
        z.tick(1.0);
        assert!(!z.just_prime);
    }

    #[test]
    fn tick_no_thicken_when_rate_zero() {
        let mut z = Zorino::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.pelt, 0.0);
    }

    #[test]
    fn tick_no_thicken_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.pelt, 0.0);
    }

    #[test]
    fn tick_clears_just_prime() {
        let mut z = Zorino::new(100.0, 200.0);
        z.pelt = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_prime);
    }

    #[test]
    fn tick_clears_just_bare() {
        let mut z = z();
        z.pelt = 10.0;
        z.deplete(10.0);
        z.tick(0.016);
        assert!(!z.just_bare);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.pelt - 10.0).abs() < 1e-3);
    }

    // --- is_prime / is_bare ---

    #[test]
    fn is_prime_false_when_disabled() {
        let mut z = z();
        z.pelt = 100.0;
        z.enabled = false;
        assert!(!z.is_prime());
    }

    #[test]
    fn is_bare_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_bare());
    }

    // --- pelt_fraction / effective_softness ---

    #[test]
    fn pelt_fraction_zero_when_bare() {
        assert_eq!(z().pelt_fraction(), 0.0);
    }

    #[test]
    fn pelt_fraction_half_at_midpoint() {
        let mut z = z();
        z.pelt = 50.0;
        assert!((z.pelt_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_softness_zero_when_bare() {
        assert_eq!(z().effective_softness(100.0), 0.0);
    }

    #[test]
    fn effective_softness_scales_with_pelt() {
        let mut z = z();
        z.pelt = 75.0;
        assert!((z.effective_softness(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_softness_zero_when_disabled() {
        let mut z = z();
        z.pelt = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_softness(100.0), 0.0);
    }
}
