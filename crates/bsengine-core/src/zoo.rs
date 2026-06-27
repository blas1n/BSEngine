use bevy_ecs::prelude::Component;

/// Enclosure-population tracker. `population` builds via `house(amount)`
/// and grows passively at `breed_rate` per second in `tick(dt)` or
/// diminishes immediately via `release(amount)`.
///
/// Models zoological-enclosure density meters, wildlife-park population
/// fill levels, contained-species abundance accumulators, exhibit-
/// capacity gauges, creature-count escalation trackers, fauna-roster
/// saturation indicators, animal-pen occupancy bars, vivarium-
/// population progress trackers, or any mechanic where steadily
/// housing more creatures within a bounded enclosure increases the
/// richness of the exhibit until every habitat is at full carrying
/// capacity and visitors crowd the viewing glass in wonder.
///
/// `house(amount)` adds population; fires `just_full` when first
/// reaching `max_population`. No-op when disabled.
///
/// `release(amount)` reduces population immediately; fires `just_empty`
/// when reaching 0. No-op when disabled or already empty.
///
/// `tick(dt)` clears both flags, then increases population by
/// `breed_rate * dt` (capped at `max_population`). Fires `just_full`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_full()` returns `population >= max_population && enabled`.
///
/// `is_empty()` returns `population == 0.0` (not gated by `enabled`).
///
/// `population_fraction()` returns `(population / max_population).clamp(0, 1)`.
///
/// `effective_exhibit(scale)` returns `scale * population_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — breeds at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoo {
    pub population: f32,
    pub max_population: f32,
    pub breed_rate: f32,
    pub just_full: bool,
    pub just_empty: bool,
    pub enabled: bool,
}

impl Zoo {
    pub fn new(max_population: f32, breed_rate: f32) -> Self {
        Self {
            population: 0.0,
            max_population: max_population.max(0.1),
            breed_rate: breed_rate.max(0.0),
            just_full: false,
            just_empty: false,
            enabled: true,
        }
    }

    /// Add population; fires `just_full` when first reaching max.
    /// No-op when disabled.
    pub fn house(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.population < self.max_population;
        self.population = (self.population + amount).min(self.max_population);
        if was_below && self.population >= self.max_population {
            self.just_full = true;
        }
    }

    /// Reduce population; fires `just_empty` when reaching 0.
    /// No-op when disabled or already empty.
    pub fn release(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.population <= 0.0 {
            return;
        }
        self.population = (self.population - amount).max(0.0);
        if self.population <= 0.0 {
            self.just_empty = true;
        }
    }

    /// Clear flags, then increase population by `breed_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_full = false;
        self.just_empty = false;
        if self.enabled && self.breed_rate > 0.0 && self.population < self.max_population {
            let was_below = self.population < self.max_population;
            self.population = (self.population + self.breed_rate * dt).min(self.max_population);
            if was_below && self.population >= self.max_population {
                self.just_full = true;
            }
        }
    }

    /// `true` when population is at maximum and component is enabled.
    pub fn is_full(&self) -> bool {
        self.population >= self.max_population && self.enabled
    }

    /// `true` when population is 0 (not gated by `enabled`).
    pub fn is_empty(&self) -> bool {
        self.population == 0.0
    }

    /// Fraction of maximum population [0.0, 1.0].
    pub fn population_fraction(&self) -> f32 {
        (self.population / self.max_population).clamp(0.0, 1.0)
    }

    /// Returns `scale * population_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_exhibit(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.population_fraction()
    }
}

impl Default for Zoo {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoo {
        Zoo::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let z = z();
        assert_eq!(z.population, 0.0);
        assert!(z.is_empty());
        assert!(!z.is_full());
    }

    #[test]
    fn new_clamps_max_population() {
        let z = Zoo::new(-5.0, 1.0);
        assert!((z.max_population - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_breed_rate() {
        let z = Zoo::new(100.0, -3.0);
        assert_eq!(z.breed_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoo::default();
        assert!((z.max_population - 100.0).abs() < 1e-5);
        assert!((z.breed_rate - 1.0).abs() < 1e-5);
    }

    // --- house ---

    #[test]
    fn house_adds_population() {
        let mut z = z();
        z.house(40.0);
        assert!((z.population - 40.0).abs() < 1e-3);
    }

    #[test]
    fn house_clamps_at_max() {
        let mut z = z();
        z.house(200.0);
        assert!((z.population - 100.0).abs() < 1e-3);
    }

    #[test]
    fn house_fires_just_full_at_max() {
        let mut z = z();
        z.house(100.0);
        assert!(z.just_full);
        assert!(z.is_full());
    }

    #[test]
    fn house_no_just_full_when_already_at_max() {
        let mut z = z();
        z.population = 100.0;
        z.house(10.0);
        assert!(!z.just_full);
    }

    #[test]
    fn house_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.house(50.0);
        assert_eq!(z.population, 0.0);
    }

    #[test]
    fn house_no_op_when_amount_zero() {
        let mut z = z();
        z.house(0.0);
        assert_eq!(z.population, 0.0);
    }

    // --- release ---

    #[test]
    fn release_reduces_population() {
        let mut z = z();
        z.population = 60.0;
        z.release(20.0);
        assert!((z.population - 40.0).abs() < 1e-3);
    }

    #[test]
    fn release_clamps_at_zero() {
        let mut z = z();
        z.population = 30.0;
        z.release(200.0);
        assert_eq!(z.population, 0.0);
    }

    #[test]
    fn release_fires_just_empty_at_zero() {
        let mut z = z();
        z.population = 30.0;
        z.release(30.0);
        assert!(z.just_empty);
    }

    #[test]
    fn release_no_op_when_already_empty() {
        let mut z = z();
        z.release(10.0);
        assert!(!z.just_empty);
    }

    #[test]
    fn release_no_op_when_disabled() {
        let mut z = z();
        z.population = 50.0;
        z.enabled = false;
        z.release(50.0);
        assert!((z.population - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_breeds_population() {
        let mut z = z(); // rate=1
        z.tick(5.0); // 0 + 1*5 = 5
        assert!((z.population - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_full_on_breed_to_max() {
        let mut z = Zoo::new(100.0, 200.0);
        z.population = 95.0;
        z.tick(1.0);
        assert!(z.just_full);
        assert!(z.is_full());
    }

    #[test]
    fn tick_no_breed_when_already_full() {
        let mut z = z();
        z.population = 100.0;
        z.tick(1.0);
        assert!(!z.just_full);
    }

    #[test]
    fn tick_no_breed_when_rate_zero() {
        let mut z = Zoo::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.population, 0.0);
    }

    #[test]
    fn tick_no_breed_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.population, 0.0);
    }

    #[test]
    fn tick_clears_just_full() {
        let mut z = Zoo::new(100.0, 200.0);
        z.population = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_full);
    }

    #[test]
    fn tick_clears_just_empty() {
        let mut z = z();
        z.population = 10.0;
        z.release(10.0);
        z.tick(0.016);
        assert!(!z.just_empty);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(8.0); // 1*8 = 8
        assert!((z.population - 8.0).abs() < 1e-3);
    }

    // --- is_full / is_empty ---

    #[test]
    fn is_full_false_when_disabled() {
        let mut z = z();
        z.population = 100.0;
        z.enabled = false;
        assert!(!z.is_full());
    }

    #[test]
    fn is_empty_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_empty());
    }

    // --- population_fraction / effective_exhibit ---

    #[test]
    fn population_fraction_zero_when_empty() {
        assert_eq!(z().population_fraction(), 0.0);
    }

    #[test]
    fn population_fraction_half_at_midpoint() {
        let mut z = z();
        z.population = 50.0;
        assert!((z.population_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_exhibit_zero_when_empty() {
        assert_eq!(z().effective_exhibit(100.0), 0.0);
    }

    #[test]
    fn effective_exhibit_scales_with_population() {
        let mut z = z();
        z.population = 75.0;
        assert!((z.effective_exhibit(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_exhibit_zero_when_disabled() {
        let mut z = z();
        z.population = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_exhibit(100.0), 0.0);
    }
}
