use bevy_ecs::prelude::Component;

/// Animal-to-human disease transmission tracker named after zoonosis,
/// the process by which a pathogen — virus, bacterium, parasite, or
/// prion — crosses the species barrier from a vertebrate reservoir host
/// into the human population. Classic zoonoses include rabies (bat and
/// dog reservoirs), bubonic plague (rodent-flea chain), influenza
/// (waterfowl, pigs, then humans), Ebola (bats or great apes), Lyme
/// disease (deer-tick-mouse cycle), and SARS-CoV-2, where the leap from
/// an animal reservoir triggered a pandemic. `contagion` builds via
/// `infect(amount)` and accumulates passively at `transmit_rate` per
/// second in `tick(dt)` or is reduced via `quarantine(amount)`.
///
/// Models epidemic-spread saturation bars, reservoir-to-host
/// transmission fill levels, outbreak-containment gauges, pathogen-
/// load accumulation trackers, spillover-event proximity meters,
/// wildlife-disease-risk fill levels, zoonotic-hotspot saturation
/// bars, cross-species infection-pressure indicators, public-health
/// alert-level fill gauges, or any mechanic where a pathogen quietly
/// amplifies inside an animal reservoir and slowly leaks across the
/// species boundary until the pressure of accumulated exposure finally
/// ignites a sustained human-to-human transmission chain that
/// containment teams scramble to smother before it spreads further.
///
/// `infect(amount)` adds contagion; fires `just_spread` when first
/// reaching `max_contagion`. No-op when disabled.
///
/// `quarantine(amount)` reduces contagion immediately; fires
/// `just_contained` when reaching 0. No-op when disabled or already
/// contained.
///
/// `tick(dt)` clears both flags, then increases contagion by
/// `transmit_rate * dt` (capped at `max_contagion`). Fires
/// `just_spread` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_spread()` returns `contagion >= max_contagion && enabled`.
///
/// `is_contained()` returns `contagion == 0.0` (not gated by `enabled`).
///
/// `contagion_fraction()` returns `(contagion / max_contagion).clamp(0, 1)`.
///
/// `effective_virulence(scale)` returns `scale * contagion_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — transmits at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoonosis {
    pub contagion: f32,
    pub max_contagion: f32,
    pub transmit_rate: f32,
    pub just_spread: bool,
    pub just_contained: bool,
    pub enabled: bool,
}

impl Zoonosis {
    pub fn new(max_contagion: f32, transmit_rate: f32) -> Self {
        Self {
            contagion: 0.0,
            max_contagion: max_contagion.max(0.1),
            transmit_rate: transmit_rate.max(0.0),
            just_spread: false,
            just_contained: false,
            enabled: true,
        }
    }

    /// Add contagion; fires `just_spread` when first reaching max.
    /// No-op when disabled.
    pub fn infect(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.contagion < self.max_contagion;
        self.contagion = (self.contagion + amount).min(self.max_contagion);
        if was_below && self.contagion >= self.max_contagion {
            self.just_spread = true;
        }
    }

    /// Reduce contagion; fires `just_contained` when reaching 0.
    /// No-op when disabled or already contained.
    pub fn quarantine(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.contagion <= 0.0 {
            return;
        }
        self.contagion = (self.contagion - amount).max(0.0);
        if self.contagion <= 0.0 {
            self.just_contained = true;
        }
    }

    /// Clear flags, then increase contagion by `transmit_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_spread = false;
        self.just_contained = false;
        if self.enabled && self.transmit_rate > 0.0 && self.contagion < self.max_contagion {
            let was_below = self.contagion < self.max_contagion;
            self.contagion = (self.contagion + self.transmit_rate * dt).min(self.max_contagion);
            if was_below && self.contagion >= self.max_contagion {
                self.just_spread = true;
            }
        }
    }

    /// `true` when contagion is at maximum and component is enabled.
    pub fn is_spread(&self) -> bool {
        self.contagion >= self.max_contagion && self.enabled
    }

    /// `true` when contagion is 0 (not gated by `enabled`).
    pub fn is_contained(&self) -> bool {
        self.contagion == 0.0
    }

    /// Fraction of maximum contagion [0.0, 1.0].
    pub fn contagion_fraction(&self) -> f32 {
        (self.contagion / self.max_contagion).clamp(0.0, 1.0)
    }

    /// Returns `scale * contagion_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_virulence(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.contagion_fraction()
    }
}

impl Default for Zoonosis {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoonosis {
        Zoonosis::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_contained() {
        let z = z();
        assert_eq!(z.contagion, 0.0);
        assert!(z.is_contained());
        assert!(!z.is_spread());
    }

    #[test]
    fn new_clamps_max_contagion() {
        let z = Zoonosis::new(-5.0, 1.5);
        assert!((z.max_contagion - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_transmit_rate() {
        let z = Zoonosis::new(100.0, -1.5);
        assert_eq!(z.transmit_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoonosis::default();
        assert!((z.max_contagion - 100.0).abs() < 1e-5);
        assert!((z.transmit_rate - 1.5).abs() < 1e-5);
    }

    // --- infect ---

    #[test]
    fn infect_adds_contagion() {
        let mut z = z();
        z.infect(40.0);
        assert!((z.contagion - 40.0).abs() < 1e-3);
    }

    #[test]
    fn infect_clamps_at_max() {
        let mut z = z();
        z.infect(200.0);
        assert!((z.contagion - 100.0).abs() < 1e-3);
    }

    #[test]
    fn infect_fires_just_spread_at_max() {
        let mut z = z();
        z.infect(100.0);
        assert!(z.just_spread);
        assert!(z.is_spread());
    }

    #[test]
    fn infect_no_just_spread_when_already_at_max() {
        let mut z = z();
        z.contagion = 100.0;
        z.infect(10.0);
        assert!(!z.just_spread);
    }

    #[test]
    fn infect_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.infect(50.0);
        assert_eq!(z.contagion, 0.0);
    }

    #[test]
    fn infect_no_op_when_amount_zero() {
        let mut z = z();
        z.infect(0.0);
        assert_eq!(z.contagion, 0.0);
    }

    // --- quarantine ---

    #[test]
    fn quarantine_reduces_contagion() {
        let mut z = z();
        z.contagion = 60.0;
        z.quarantine(20.0);
        assert!((z.contagion - 40.0).abs() < 1e-3);
    }

    #[test]
    fn quarantine_clamps_at_zero() {
        let mut z = z();
        z.contagion = 30.0;
        z.quarantine(200.0);
        assert_eq!(z.contagion, 0.0);
    }

    #[test]
    fn quarantine_fires_just_contained_at_zero() {
        let mut z = z();
        z.contagion = 30.0;
        z.quarantine(30.0);
        assert!(z.just_contained);
    }

    #[test]
    fn quarantine_no_op_when_already_contained() {
        let mut z = z();
        z.quarantine(10.0);
        assert!(!z.just_contained);
    }

    #[test]
    fn quarantine_no_op_when_disabled() {
        let mut z = z();
        z.contagion = 50.0;
        z.enabled = false;
        z.quarantine(50.0);
        assert!((z.contagion - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_transmits_contagion() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.contagion - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_spread_on_transmit_to_max() {
        let mut z = Zoonosis::new(100.0, 200.0);
        z.contagion = 95.0;
        z.tick(1.0);
        assert!(z.just_spread);
        assert!(z.is_spread());
    }

    #[test]
    fn tick_no_transmit_when_already_spread() {
        let mut z = z();
        z.contagion = 100.0;
        z.tick(1.0);
        assert!(!z.just_spread);
    }

    #[test]
    fn tick_no_transmit_when_rate_zero() {
        let mut z = Zoonosis::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.contagion, 0.0);
    }

    #[test]
    fn tick_no_transmit_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.contagion, 0.0);
    }

    #[test]
    fn tick_clears_just_spread() {
        let mut z = Zoonosis::new(100.0, 200.0);
        z.contagion = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_spread);
    }

    #[test]
    fn tick_clears_just_contained() {
        let mut z = z();
        z.contagion = 10.0;
        z.quarantine(10.0);
        z.tick(0.016);
        assert!(!z.just_contained);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.contagion - 9.0).abs() < 1e-3);
    }

    // --- is_spread / is_contained ---

    #[test]
    fn is_spread_false_when_disabled() {
        let mut z = z();
        z.contagion = 100.0;
        z.enabled = false;
        assert!(!z.is_spread());
    }

    #[test]
    fn is_contained_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_contained());
    }

    // --- contagion_fraction / effective_virulence ---

    #[test]
    fn contagion_fraction_zero_when_contained() {
        assert_eq!(z().contagion_fraction(), 0.0);
    }

    #[test]
    fn contagion_fraction_half_at_midpoint() {
        let mut z = z();
        z.contagion = 50.0;
        assert!((z.contagion_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_virulence_zero_when_contained() {
        assert_eq!(z().effective_virulence(100.0), 0.0);
    }

    #[test]
    fn effective_virulence_scales_with_contagion() {
        let mut z = z();
        z.contagion = 75.0;
        assert!((z.effective_virulence(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_virulence_zero_when_disabled() {
        let mut z = z();
        z.contagion = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_virulence(100.0), 0.0);
    }
}
