use bevy_ecs::prelude::Component;

/// Carnivore-diet accumulation tracker named after zoophagous, the
/// adjective describing any organism that subsists by eating animals.
/// The word derives from Greek zōon (animal) plus phagein (to eat),
/// and it covers an ecological spectrum from specialist to generalist
/// carnivores: the polar bear that hunts ringed seals on sea ice,
/// the dragonfly larva that ambushes tadpoles in a garden pond, the
/// pitcher plant that digests insects dissolved in its digestive
/// fluid, and the enormous blue whale that strains krill through
/// baleen plates are all, in different ways, zoophagous. The term
/// stands in contrast to phytophagous (plant-eating) and saprophagous
/// (detritus-eating), and its breadth makes it useful for discussing
/// feeding guilds in food webs — a zoophagous guild in a tropical
/// forest includes raptors, snakes, centipedes, web-building spiders,
/// and carnivorous beetles, each occupying a different microhabitat
/// but all drawing their energy from animal tissue. Zoophagous
/// behaviour evolves repeatedly because animal prey is typically
/// energy-dense relative to plant material: the protein and lipid
/// content of prey supports rapid growth and reproduction, but
/// catching and subduing mobile, defended, or concealed prey demands
/// corresponding specialisation in sensory organs, locomotion,
/// toxins, or manipulative appendages. `feeding` builds via
/// `consume(amount)` and accumulates passively at `prey_rate` per
/// second in `tick(dt)` or depletes via `starve(amount)`.
///
/// Models carnivore-diet fill levels, predator-satiation bars,
/// prey-energy accumulation trackers, hunting-success gauges,
/// carnivory-saturation fill levels, pack-hunt-reward accumulators,
/// ambush-predator-patience meters, zoophagous-adaptation fill
/// levels, dietary-specialisation saturation bars, or any mechanic
/// where a creature must hunt and consume other animals to maintain
/// its vigour, metabolism, or combat performance — and where
/// prolonged failure to feed erodes that performance back to the
/// sluggish baseline of a famished predator.
///
/// `consume(amount)` adds feeding; fires `just_satiated` when first
/// reaching `max_feeding`. No-op when disabled.
///
/// `starve(amount)` reduces feeding immediately; fires `just_famished`
/// when reaching 0. No-op when disabled or already famished.
///
/// `tick(dt)` clears both flags, then increases feeding by
/// `prey_rate * dt` (capped at `max_feeding`). Fires `just_satiated`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_satiated()` returns `feeding >= max_feeding && enabled`.
///
/// `is_famished()` returns `feeding == 0.0` (not gated by `enabled`).
///
/// `feeding_fraction()` returns
/// `(feeding / max_feeding).clamp(0, 1)`.
///
/// `effective_predation(scale)` returns `scale * feeding_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — hunts at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoophagous {
    pub feeding: f32,
    pub max_feeding: f32,
    pub prey_rate: f32,
    pub just_satiated: bool,
    pub just_famished: bool,
    pub enabled: bool,
}

impl Zoophagous {
    pub fn new(max_feeding: f32, prey_rate: f32) -> Self {
        Self {
            feeding: 0.0,
            max_feeding: max_feeding.max(0.1),
            prey_rate: prey_rate.max(0.0),
            just_satiated: false,
            just_famished: false,
            enabled: true,
        }
    }

    /// Add feeding; fires `just_satiated` when first reaching max.
    /// No-op when disabled.
    pub fn consume(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.feeding < self.max_feeding;
        self.feeding = (self.feeding + amount).min(self.max_feeding);
        if was_below && self.feeding >= self.max_feeding {
            self.just_satiated = true;
        }
    }

    /// Reduce feeding; fires `just_famished` when reaching 0.
    /// No-op when disabled or already famished.
    pub fn starve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.feeding <= 0.0 {
            return;
        }
        self.feeding = (self.feeding - amount).max(0.0);
        if self.feeding <= 0.0 {
            self.just_famished = true;
        }
    }

    /// Clear flags, then increase feeding by `prey_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_satiated = false;
        self.just_famished = false;
        if self.enabled && self.prey_rate > 0.0 && self.feeding < self.max_feeding {
            let was_below = self.feeding < self.max_feeding;
            self.feeding = (self.feeding + self.prey_rate * dt).min(self.max_feeding);
            if was_below && self.feeding >= self.max_feeding {
                self.just_satiated = true;
            }
        }
    }

    /// `true` when feeding is at maximum and component is enabled.
    pub fn is_satiated(&self) -> bool {
        self.feeding >= self.max_feeding && self.enabled
    }

    /// `true` when feeding is 0 (not gated by `enabled`).
    pub fn is_famished(&self) -> bool {
        self.feeding == 0.0
    }

    /// Fraction of maximum feeding [0.0, 1.0].
    pub fn feeding_fraction(&self) -> f32 {
        (self.feeding / self.max_feeding).clamp(0.0, 1.0)
    }

    /// Returns `scale * feeding_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_predation(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.feeding_fraction()
    }
}

impl Default for Zoophagous {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoophagous {
        Zoophagous::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_famished() {
        let z = z();
        assert_eq!(z.feeding, 0.0);
        assert!(z.is_famished());
        assert!(!z.is_satiated());
    }

    #[test]
    fn new_clamps_max_feeding() {
        let z = Zoophagous::new(-5.0, 1.5);
        assert!((z.max_feeding - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_prey_rate() {
        let z = Zoophagous::new(100.0, -1.5);
        assert_eq!(z.prey_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoophagous::default();
        assert!((z.max_feeding - 100.0).abs() < 1e-5);
        assert!((z.prey_rate - 1.5).abs() < 1e-5);
    }

    // --- consume ---

    #[test]
    fn consume_adds_feeding() {
        let mut z = z();
        z.consume(40.0);
        assert!((z.feeding - 40.0).abs() < 1e-3);
    }

    #[test]
    fn consume_clamps_at_max() {
        let mut z = z();
        z.consume(200.0);
        assert!((z.feeding - 100.0).abs() < 1e-3);
    }

    #[test]
    fn consume_fires_just_satiated_at_max() {
        let mut z = z();
        z.consume(100.0);
        assert!(z.just_satiated);
        assert!(z.is_satiated());
    }

    #[test]
    fn consume_no_just_satiated_when_already_at_max() {
        let mut z = z();
        z.feeding = 100.0;
        z.consume(10.0);
        assert!(!z.just_satiated);
    }

    #[test]
    fn consume_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.consume(50.0);
        assert_eq!(z.feeding, 0.0);
    }

    #[test]
    fn consume_no_op_when_amount_zero() {
        let mut z = z();
        z.consume(0.0);
        assert_eq!(z.feeding, 0.0);
    }

    // --- starve ---

    #[test]
    fn starve_reduces_feeding() {
        let mut z = z();
        z.feeding = 60.0;
        z.starve(20.0);
        assert!((z.feeding - 40.0).abs() < 1e-3);
    }

    #[test]
    fn starve_clamps_at_zero() {
        let mut z = z();
        z.feeding = 30.0;
        z.starve(200.0);
        assert_eq!(z.feeding, 0.0);
    }

    #[test]
    fn starve_fires_just_famished_at_zero() {
        let mut z = z();
        z.feeding = 30.0;
        z.starve(30.0);
        assert!(z.just_famished);
    }

    #[test]
    fn starve_no_op_when_already_famished() {
        let mut z = z();
        z.starve(10.0);
        assert!(!z.just_famished);
    }

    #[test]
    fn starve_no_op_when_disabled() {
        let mut z = z();
        z.feeding = 50.0;
        z.enabled = false;
        z.starve(50.0);
        assert!((z.feeding - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_accumulates_feeding() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.feeding - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_satiated_on_consume_to_max() {
        let mut z = Zoophagous::new(100.0, 200.0);
        z.feeding = 95.0;
        z.tick(1.0);
        assert!(z.just_satiated);
        assert!(z.is_satiated());
    }

    #[test]
    fn tick_no_consume_when_already_satiated() {
        let mut z = z();
        z.feeding = 100.0;
        z.tick(1.0);
        assert!(!z.just_satiated);
    }

    #[test]
    fn tick_no_consume_when_rate_zero() {
        let mut z = Zoophagous::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.feeding, 0.0);
    }

    #[test]
    fn tick_no_consume_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.feeding, 0.0);
    }

    #[test]
    fn tick_clears_just_satiated() {
        let mut z = Zoophagous::new(100.0, 200.0);
        z.feeding = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_satiated);
    }

    #[test]
    fn tick_clears_just_famished() {
        let mut z = z();
        z.feeding = 10.0;
        z.starve(10.0);
        z.tick(0.016);
        assert!(!z.just_famished);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.feeding - 9.0).abs() < 1e-3);
    }

    // --- is_satiated / is_famished ---

    #[test]
    fn is_satiated_false_when_disabled() {
        let mut z = z();
        z.feeding = 100.0;
        z.enabled = false;
        assert!(!z.is_satiated());
    }

    #[test]
    fn is_famished_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_famished());
    }

    // --- feeding_fraction / effective_predation ---

    #[test]
    fn feeding_fraction_zero_when_famished() {
        assert_eq!(z().feeding_fraction(), 0.0);
    }

    #[test]
    fn feeding_fraction_half_at_midpoint() {
        let mut z = z();
        z.feeding = 50.0;
        assert!((z.feeding_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_predation_zero_when_famished() {
        assert_eq!(z().effective_predation(100.0), 0.0);
    }

    #[test]
    fn effective_predation_scales_with_feeding() {
        let mut z = z();
        z.feeding = 75.0;
        assert!((z.effective_predation(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_predation_zero_when_disabled() {
        let mut z = z();
        z.feeding = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_predation(100.0), 0.0);
    }
}
