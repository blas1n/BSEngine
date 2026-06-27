use bevy_ecs::prelude::Component;

/// Bryozoan-colony tracker. `colony` builds via `bud(amount)` and
/// encrusts passively at `encrust_rate` per second in `tick(dt)` or
/// is dissolved immediately via `dissolve(amount)`.
///
/// Models bryozoan-colony expansion bars, reef-encrustation fill
/// levels, coral-crust growth trackers, sessile-colony saturation
/// gauges, lophophore-chamber density indicators, colonial-growth
/// rate meters, calcareous-mat coverage accumulators, filter-feeding
/// efficiency bars, biofouling intensity trackers, or any mechanic
/// where thousands of tiny polyps build their individual calcite
/// chambers one by one until every hard surface is smothered in a
/// lacework of miniature arches — only for a chemical spike to
/// dissolve the whole colony back to bare rock in moments.
///
/// `bud(amount)` adds colony; fires `just_established` when first
/// reaching `max_colony`. No-op when disabled.
///
/// `dissolve(amount)` reduces colony immediately; fires `just_dispersed`
/// when reaching 0. No-op when disabled or already dispersed.
///
/// `tick(dt)` clears both flags, then increases colony by
/// `encrust_rate * dt` (capped at `max_colony`). Fires
/// `just_established` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_established()` returns `colony >= max_colony && enabled`.
///
/// `is_dispersed()` returns `colony == 0.0` (not gated by `enabled`).
///
/// `colony_fraction()` returns `(colony / max_colony).clamp(0, 1)`.
///
/// `effective_filtration(scale)` returns `scale * colony_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — encrusts at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoecium {
    pub colony: f32,
    pub max_colony: f32,
    pub encrust_rate: f32,
    pub just_established: bool,
    pub just_dispersed: bool,
    pub enabled: bool,
}

impl Zoecium {
    pub fn new(max_colony: f32, encrust_rate: f32) -> Self {
        Self {
            colony: 0.0,
            max_colony: max_colony.max(0.1),
            encrust_rate: encrust_rate.max(0.0),
            just_established: false,
            just_dispersed: false,
            enabled: true,
        }
    }

    /// Add colony; fires `just_established` when first reaching max.
    /// No-op when disabled.
    pub fn bud(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.colony < self.max_colony;
        self.colony = (self.colony + amount).min(self.max_colony);
        if was_below && self.colony >= self.max_colony {
            self.just_established = true;
        }
    }

    /// Reduce colony; fires `just_dispersed` when reaching 0.
    /// No-op when disabled or already dispersed.
    pub fn dissolve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.colony <= 0.0 {
            return;
        }
        self.colony = (self.colony - amount).max(0.0);
        if self.colony <= 0.0 {
            self.just_dispersed = true;
        }
    }

    /// Clear flags, then increase colony by `encrust_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_established = false;
        self.just_dispersed = false;
        if self.enabled && self.encrust_rate > 0.0 && self.colony < self.max_colony {
            let was_below = self.colony < self.max_colony;
            self.colony = (self.colony + self.encrust_rate * dt).min(self.max_colony);
            if was_below && self.colony >= self.max_colony {
                self.just_established = true;
            }
        }
    }

    /// `true` when colony is at maximum and component is enabled.
    pub fn is_established(&self) -> bool {
        self.colony >= self.max_colony && self.enabled
    }

    /// `true` when colony is 0 (not gated by `enabled`).
    pub fn is_dispersed(&self) -> bool {
        self.colony == 0.0
    }

    /// Fraction of maximum colony [0.0, 1.0].
    pub fn colony_fraction(&self) -> f32 {
        (self.colony / self.max_colony).clamp(0.0, 1.0)
    }

    /// Returns `scale * colony_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_filtration(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.colony_fraction()
    }
}

impl Default for Zoecium {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoecium {
        Zoecium::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_dispersed() {
        let z = z();
        assert_eq!(z.colony, 0.0);
        assert!(z.is_dispersed());
        assert!(!z.is_established());
    }

    #[test]
    fn new_clamps_max_colony() {
        let z = Zoecium::new(-5.0, 2.0);
        assert!((z.max_colony - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_encrust_rate() {
        let z = Zoecium::new(100.0, -3.0);
        assert_eq!(z.encrust_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoecium::default();
        assert!((z.max_colony - 100.0).abs() < 1e-5);
        assert!((z.encrust_rate - 2.0).abs() < 1e-5);
    }

    // --- bud ---

    #[test]
    fn bud_adds_colony() {
        let mut z = z();
        z.bud(40.0);
        assert!((z.colony - 40.0).abs() < 1e-3);
    }

    #[test]
    fn bud_clamps_at_max() {
        let mut z = z();
        z.bud(200.0);
        assert!((z.colony - 100.0).abs() < 1e-3);
    }

    #[test]
    fn bud_fires_just_established_at_max() {
        let mut z = z();
        z.bud(100.0);
        assert!(z.just_established);
        assert!(z.is_established());
    }

    #[test]
    fn bud_no_just_established_when_already_at_max() {
        let mut z = z();
        z.colony = 100.0;
        z.bud(10.0);
        assert!(!z.just_established);
    }

    #[test]
    fn bud_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.bud(50.0);
        assert_eq!(z.colony, 0.0);
    }

    #[test]
    fn bud_no_op_when_amount_zero() {
        let mut z = z();
        z.bud(0.0);
        assert_eq!(z.colony, 0.0);
    }

    // --- dissolve ---

    #[test]
    fn dissolve_reduces_colony() {
        let mut z = z();
        z.colony = 60.0;
        z.dissolve(20.0);
        assert!((z.colony - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dissolve_clamps_at_zero() {
        let mut z = z();
        z.colony = 30.0;
        z.dissolve(200.0);
        assert_eq!(z.colony, 0.0);
    }

    #[test]
    fn dissolve_fires_just_dispersed_at_zero() {
        let mut z = z();
        z.colony = 30.0;
        z.dissolve(30.0);
        assert!(z.just_dispersed);
    }

    #[test]
    fn dissolve_no_op_when_already_dispersed() {
        let mut z = z();
        z.dissolve(10.0);
        assert!(!z.just_dispersed);
    }

    #[test]
    fn dissolve_no_op_when_disabled() {
        let mut z = z();
        z.colony = 50.0;
        z.enabled = false;
        z.dissolve(50.0);
        assert!((z.colony - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_encrusts_colony() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.colony - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_established_on_encrust_to_max() {
        let mut z = Zoecium::new(100.0, 200.0);
        z.colony = 95.0;
        z.tick(1.0);
        assert!(z.just_established);
        assert!(z.is_established());
    }

    #[test]
    fn tick_no_encrust_when_already_established() {
        let mut z = z();
        z.colony = 100.0;
        z.tick(1.0);
        assert!(!z.just_established);
    }

    #[test]
    fn tick_no_encrust_when_rate_zero() {
        let mut z = Zoecium::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.colony, 0.0);
    }

    #[test]
    fn tick_no_encrust_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.colony, 0.0);
    }

    #[test]
    fn tick_clears_just_established() {
        let mut z = Zoecium::new(100.0, 200.0);
        z.colony = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_established);
    }

    #[test]
    fn tick_clears_just_dispersed() {
        let mut z = z();
        z.colony = 10.0;
        z.dissolve(10.0);
        z.tick(0.016);
        assert!(!z.just_dispersed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.colony - 10.0).abs() < 1e-3);
    }

    // --- is_established / is_dispersed ---

    #[test]
    fn is_established_false_when_disabled() {
        let mut z = z();
        z.colony = 100.0;
        z.enabled = false;
        assert!(!z.is_established());
    }

    #[test]
    fn is_dispersed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_dispersed());
    }

    // --- colony_fraction / effective_filtration ---

    #[test]
    fn colony_fraction_zero_when_dispersed() {
        assert_eq!(z().colony_fraction(), 0.0);
    }

    #[test]
    fn colony_fraction_half_at_midpoint() {
        let mut z = z();
        z.colony = 50.0;
        assert!((z.colony_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_filtration_zero_when_dispersed() {
        assert_eq!(z().effective_filtration(100.0), 0.0);
    }

    #[test]
    fn effective_filtration_scales_with_colony() {
        let mut z = z();
        z.colony = 75.0;
        assert!((z.effective_filtration(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_filtration_zero_when_disabled() {
        let mut z = z();
        z.colony = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_filtration(100.0), 0.0);
    }
}
