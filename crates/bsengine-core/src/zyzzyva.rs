use bevy_ecs::prelude::Component;

/// Pest-infestation-pressure tracker named after the South American
/// palm weevil genus (Zyzzyva) — famous for being the last entry in
/// many English dictionaries. `pressure` builds via
/// `infest(amount)` and accumulates passively at `spread_rate` per
/// second in `tick(dt)` or is suppressed immediately via
/// `fumigate(amount)`.
///
/// Models tropical-pest infestation fill levels, agricultural-
/// blight pressure bars, invasive-species spread gauges, palm-
/// frond boring intensity trackers, beetle-population density
/// meters, phytosanitary-risk saturation indicators, quarantine-
/// threshold pressure bars, crop-damage accumulation trackers,
/// biological-pest-control depletion gauges, or any mechanic where
/// a population of tiny, armored, magnificently named beetles
/// tunnels methodically through every stalk and frond until the
/// plantation is riddled with galleries, the palms are toppled,
/// and the only word that comes to mind — the very last one in
/// the dictionary — is the name of the insect responsible.
///
/// `infest(amount)` adds pressure; fires `just_swarming` when
/// first reaching `max_pressure`. No-op when disabled.
///
/// `fumigate(amount)` reduces pressure immediately; fires
/// `just_eradicated` when reaching 0. No-op when disabled or
/// already eradicated.
///
/// `tick(dt)` clears both flags, then increases pressure by
/// `spread_rate * dt` (capped at `max_pressure`). Fires
/// `just_swarming` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_swarming()` returns `pressure >= max_pressure && enabled`.
///
/// `is_eradicated()` returns `pressure == 0.0` (not gated by `enabled`).
///
/// `pressure_fraction()` returns `(pressure / max_pressure).clamp(0, 1)`.
///
/// `effective_damage(scale)` returns `scale * pressure_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — spreads at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zyzzyva {
    pub pressure: f32,
    pub max_pressure: f32,
    pub spread_rate: f32,
    pub just_swarming: bool,
    pub just_eradicated: bool,
    pub enabled: bool,
}

impl Zyzzyva {
    pub fn new(max_pressure: f32, spread_rate: f32) -> Self {
        Self {
            pressure: 0.0,
            max_pressure: max_pressure.max(0.1),
            spread_rate: spread_rate.max(0.0),
            just_swarming: false,
            just_eradicated: false,
            enabled: true,
        }
    }

    /// Add pressure; fires `just_swarming` when first reaching max.
    /// No-op when disabled.
    pub fn infest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.pressure < self.max_pressure;
        self.pressure = (self.pressure + amount).min(self.max_pressure);
        if was_below && self.pressure >= self.max_pressure {
            self.just_swarming = true;
        }
    }

    /// Reduce pressure; fires `just_eradicated` when reaching 0.
    /// No-op when disabled or already eradicated.
    pub fn fumigate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pressure <= 0.0 {
            return;
        }
        self.pressure = (self.pressure - amount).max(0.0);
        if self.pressure <= 0.0 {
            self.just_eradicated = true;
        }
    }

    /// Clear flags, then increase pressure by `spread_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_swarming = false;
        self.just_eradicated = false;
        if self.enabled && self.spread_rate > 0.0 && self.pressure < self.max_pressure {
            let was_below = self.pressure < self.max_pressure;
            self.pressure = (self.pressure + self.spread_rate * dt).min(self.max_pressure);
            if was_below && self.pressure >= self.max_pressure {
                self.just_swarming = true;
            }
        }
    }

    /// `true` when pressure is at maximum and component is enabled.
    pub fn is_swarming(&self) -> bool {
        self.pressure >= self.max_pressure && self.enabled
    }

    /// `true` when pressure is 0 (not gated by `enabled`).
    pub fn is_eradicated(&self) -> bool {
        self.pressure == 0.0
    }

    /// Fraction of maximum pressure [0.0, 1.0].
    pub fn pressure_fraction(&self) -> f32 {
        (self.pressure / self.max_pressure).clamp(0.0, 1.0)
    }

    /// Returns `scale * pressure_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_damage(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.pressure_fraction()
    }
}

impl Default for Zyzzyva {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zyzzyva {
        Zyzzyva::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_eradicated() {
        let z = z();
        assert_eq!(z.pressure, 0.0);
        assert!(z.is_eradicated());
        assert!(!z.is_swarming());
    }

    #[test]
    fn new_clamps_max_pressure() {
        let z = Zyzzyva::new(-5.0, 2.0);
        assert!((z.max_pressure - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spread_rate() {
        let z = Zyzzyva::new(100.0, -2.0);
        assert_eq!(z.spread_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zyzzyva::default();
        assert!((z.max_pressure - 100.0).abs() < 1e-5);
        assert!((z.spread_rate - 2.0).abs() < 1e-5);
    }

    // --- infest ---

    #[test]
    fn infest_adds_pressure() {
        let mut z = z();
        z.infest(40.0);
        assert!((z.pressure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn infest_clamps_at_max() {
        let mut z = z();
        z.infest(200.0);
        assert!((z.pressure - 100.0).abs() < 1e-3);
    }

    #[test]
    fn infest_fires_just_swarming_at_max() {
        let mut z = z();
        z.infest(100.0);
        assert!(z.just_swarming);
        assert!(z.is_swarming());
    }

    #[test]
    fn infest_no_just_swarming_when_already_at_max() {
        let mut z = z();
        z.pressure = 100.0;
        z.infest(10.0);
        assert!(!z.just_swarming);
    }

    #[test]
    fn infest_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.infest(50.0);
        assert_eq!(z.pressure, 0.0);
    }

    #[test]
    fn infest_no_op_when_amount_zero() {
        let mut z = z();
        z.infest(0.0);
        assert_eq!(z.pressure, 0.0);
    }

    // --- fumigate ---

    #[test]
    fn fumigate_reduces_pressure() {
        let mut z = z();
        z.pressure = 60.0;
        z.fumigate(20.0);
        assert!((z.pressure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fumigate_clamps_at_zero() {
        let mut z = z();
        z.pressure = 30.0;
        z.fumigate(200.0);
        assert_eq!(z.pressure, 0.0);
    }

    #[test]
    fn fumigate_fires_just_eradicated_at_zero() {
        let mut z = z();
        z.pressure = 30.0;
        z.fumigate(30.0);
        assert!(z.just_eradicated);
    }

    #[test]
    fn fumigate_no_op_when_already_eradicated() {
        let mut z = z();
        z.fumigate(10.0);
        assert!(!z.just_eradicated);
    }

    #[test]
    fn fumigate_no_op_when_disabled() {
        let mut z = z();
        z.pressure = 50.0;
        z.enabled = false;
        z.fumigate(50.0);
        assert!((z.pressure - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_spreads_pressure() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.pressure - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_swarming_on_spread_to_max() {
        let mut z = Zyzzyva::new(100.0, 200.0);
        z.pressure = 95.0;
        z.tick(1.0);
        assert!(z.just_swarming);
        assert!(z.is_swarming());
    }

    #[test]
    fn tick_no_spread_when_already_swarming() {
        let mut z = z();
        z.pressure = 100.0;
        z.tick(1.0);
        assert!(!z.just_swarming);
    }

    #[test]
    fn tick_no_spread_when_rate_zero() {
        let mut z = Zyzzyva::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.pressure, 0.0);
    }

    #[test]
    fn tick_no_spread_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.pressure, 0.0);
    }

    #[test]
    fn tick_clears_just_swarming() {
        let mut z = Zyzzyva::new(100.0, 200.0);
        z.pressure = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_swarming);
    }

    #[test]
    fn tick_clears_just_eradicated() {
        let mut z = z();
        z.pressure = 10.0;
        z.fumigate(10.0);
        z.tick(0.016);
        assert!(!z.just_eradicated);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.pressure - 10.0).abs() < 1e-3);
    }

    // --- is_swarming / is_eradicated ---

    #[test]
    fn is_swarming_false_when_disabled() {
        let mut z = z();
        z.pressure = 100.0;
        z.enabled = false;
        assert!(!z.is_swarming());
    }

    #[test]
    fn is_eradicated_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_eradicated());
    }

    // --- pressure_fraction / effective_damage ---

    #[test]
    fn pressure_fraction_zero_when_eradicated() {
        assert_eq!(z().pressure_fraction(), 0.0);
    }

    #[test]
    fn pressure_fraction_half_at_midpoint() {
        let mut z = z();
        z.pressure = 50.0;
        assert!((z.pressure_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_damage_zero_when_eradicated() {
        assert_eq!(z().effective_damage(100.0), 0.0);
    }

    #[test]
    fn effective_damage_scales_with_pressure() {
        let mut z = z();
        z.pressure = 75.0;
        assert!((z.effective_damage(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_zero_when_disabled() {
        let mut z = z();
        z.pressure = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_damage(100.0), 0.0);
    }
}
