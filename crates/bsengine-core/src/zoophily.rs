use bevy_ecs::prelude::Component;

/// Animal-pollination accumulation tracker named after zoophily, the
/// condition of being adapted for pollination by animals — or more
/// broadly the entire mutualistic economy in which flowering plants
/// invest in attracting animals and those animals, in seeking the
/// reward, inadvertently carry pollen from flower to flower. Zoophily
/// in its strict sense encompasses every non-wind pollinator: bees and
/// bumblebees are the archetypal zoophilous agents, but the guild also
/// includes moths, butterflies, beetles, flies, ants, hummingbirds,
/// sunbirds, honeyeaters, bats, and even some non-flying mammals such
/// as lemurs and possums. Each pollinator guild selects for a different
/// floral syndrome: bee flowers are typically blue or yellow, with
/// landing platforms and ultraviolet nectar guides invisible to humans;
/// moth flowers are white and heavily scented at night; bat flowers are
/// large, sturdy, nocturnal, and rich in protein-rich pollen; bird-
/// pollinated flowers are often red and odourless — birds lack the
/// olfactory receptor density that matters to insects. The degree of
/// zoophily in a plant community can be quantified as the fraction of
/// species that require an animal vector for successful fertilisation,
/// and this fraction rises steeply from boreal forests, where wind
/// dominates, toward tropical rainforests, where the dense canopy
/// suppresses wind and the extraordinary diversity of animals makes
/// zoophily overwhelmingly the dominant strategy. `pollination` builds
/// via `visit(amount)` and accumulates passively at `visit_rate` per
/// second in `tick(dt)` or declines via `wane(amount)`.
///
/// Models animal-pollination fill levels, pollinator-visit saturation
/// bars, zoophilous-flower-reward accumulation trackers, nectar-
/// production-cycle gauges, mutualism-density fill levels, floral-
/// syndrome adaptation saturation indicators, fruit-set probability
/// accumulation bars, seed-dispersal-readiness meters, ovule-
/// fertilisation fill levels, or any mechanic where a plant, character,
/// or faction slowly accumulates animal-mediated reproductive success
/// until it reaches peak fecundity — and where cold, drought, habitat
/// loss, or pesticide collapse causes pollinator visits to dwindle and
/// the fill level to retreat toward zero.
///
/// `visit(amount)` adds pollination; fires `just_fertilized` when first
/// reaching `max_pollination`. No-op when disabled.
///
/// `wane(amount)` reduces pollination immediately; fires `just_barren`
/// when reaching 0. No-op when disabled or already barren.
///
/// `tick(dt)` clears both flags, then increases pollination by
/// `visit_rate * dt` (capped at `max_pollination`). Fires
/// `just_fertilized` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_fertilized()` returns `pollination >= max_pollination && enabled`.
///
/// `is_barren()` returns `pollination == 0.0` (not gated by `enabled`).
///
/// `pollination_fraction()` returns
/// `(pollination / max_pollination).clamp(0, 1)`.
///
/// `effective_fecundity(scale)` returns `scale * pollination_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — visits at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoophily {
    pub pollination: f32,
    pub max_pollination: f32,
    pub visit_rate: f32,
    pub just_fertilized: bool,
    pub just_barren: bool,
    pub enabled: bool,
}

impl Zoophily {
    pub fn new(max_pollination: f32, visit_rate: f32) -> Self {
        Self {
            pollination: 0.0,
            max_pollination: max_pollination.max(0.1),
            visit_rate: visit_rate.max(0.0),
            just_fertilized: false,
            just_barren: false,
            enabled: true,
        }
    }

    /// Add pollination; fires `just_fertilized` when first reaching max.
    /// No-op when disabled.
    pub fn visit(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.pollination < self.max_pollination;
        self.pollination = (self.pollination + amount).min(self.max_pollination);
        if was_below && self.pollination >= self.max_pollination {
            self.just_fertilized = true;
        }
    }

    /// Reduce pollination; fires `just_barren` when reaching 0.
    /// No-op when disabled or already barren.
    pub fn wane(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pollination <= 0.0 {
            return;
        }
        self.pollination = (self.pollination - amount).max(0.0);
        if self.pollination <= 0.0 {
            self.just_barren = true;
        }
    }

    /// Clear flags, then increase pollination by `visit_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_fertilized = false;
        self.just_barren = false;
        if self.enabled && self.visit_rate > 0.0 && self.pollination < self.max_pollination {
            let was_below = self.pollination < self.max_pollination;
            self.pollination = (self.pollination + self.visit_rate * dt).min(self.max_pollination);
            if was_below && self.pollination >= self.max_pollination {
                self.just_fertilized = true;
            }
        }
    }

    /// `true` when pollination is at maximum and component is enabled.
    pub fn is_fertilized(&self) -> bool {
        self.pollination >= self.max_pollination && self.enabled
    }

    /// `true` when pollination is 0 (not gated by `enabled`).
    pub fn is_barren(&self) -> bool {
        self.pollination == 0.0
    }

    /// Fraction of maximum pollination [0.0, 1.0].
    pub fn pollination_fraction(&self) -> f32 {
        (self.pollination / self.max_pollination).clamp(0.0, 1.0)
    }

    /// Returns `scale * pollination_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_fecundity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.pollination_fraction()
    }
}

impl Default for Zoophily {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoophily {
        Zoophily::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_barren() {
        let z = z();
        assert_eq!(z.pollination, 0.0);
        assert!(z.is_barren());
        assert!(!z.is_fertilized());
    }

    #[test]
    fn new_clamps_max_pollination() {
        let z = Zoophily::new(-5.0, 1.5);
        assert!((z.max_pollination - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_visit_rate() {
        let z = Zoophily::new(100.0, -1.5);
        assert_eq!(z.visit_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoophily::default();
        assert!((z.max_pollination - 100.0).abs() < 1e-5);
        assert!((z.visit_rate - 1.5).abs() < 1e-5);
    }

    // --- visit ---

    #[test]
    fn visit_adds_pollination() {
        let mut z = z();
        z.visit(40.0);
        assert!((z.pollination - 40.0).abs() < 1e-3);
    }

    #[test]
    fn visit_clamps_at_max() {
        let mut z = z();
        z.visit(200.0);
        assert!((z.pollination - 100.0).abs() < 1e-3);
    }

    #[test]
    fn visit_fires_just_fertilized_at_max() {
        let mut z = z();
        z.visit(100.0);
        assert!(z.just_fertilized);
        assert!(z.is_fertilized());
    }

    #[test]
    fn visit_no_just_fertilized_when_already_at_max() {
        let mut z = z();
        z.pollination = 100.0;
        z.visit(10.0);
        assert!(!z.just_fertilized);
    }

    #[test]
    fn visit_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.visit(50.0);
        assert_eq!(z.pollination, 0.0);
    }

    #[test]
    fn visit_no_op_when_amount_zero() {
        let mut z = z();
        z.visit(0.0);
        assert_eq!(z.pollination, 0.0);
    }

    // --- wane ---

    #[test]
    fn wane_reduces_pollination() {
        let mut z = z();
        z.pollination = 60.0;
        z.wane(20.0);
        assert!((z.pollination - 40.0).abs() < 1e-3);
    }

    #[test]
    fn wane_clamps_at_zero() {
        let mut z = z();
        z.pollination = 30.0;
        z.wane(200.0);
        assert_eq!(z.pollination, 0.0);
    }

    #[test]
    fn wane_fires_just_barren_at_zero() {
        let mut z = z();
        z.pollination = 30.0;
        z.wane(30.0);
        assert!(z.just_barren);
    }

    #[test]
    fn wane_no_op_when_already_barren() {
        let mut z = z();
        z.wane(10.0);
        assert!(!z.just_barren);
    }

    #[test]
    fn wane_no_op_when_disabled() {
        let mut z = z();
        z.pollination = 50.0;
        z.enabled = false;
        z.wane(50.0);
        assert!((z.pollination - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_visits_pollination() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.pollination - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_fertilized_on_visit_to_max() {
        let mut z = Zoophily::new(100.0, 200.0);
        z.pollination = 95.0;
        z.tick(1.0);
        assert!(z.just_fertilized);
        assert!(z.is_fertilized());
    }

    #[test]
    fn tick_no_visit_when_already_fertilized() {
        let mut z = z();
        z.pollination = 100.0;
        z.tick(1.0);
        assert!(!z.just_fertilized);
    }

    #[test]
    fn tick_no_visit_when_rate_zero() {
        let mut z = Zoophily::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.pollination, 0.0);
    }

    #[test]
    fn tick_no_visit_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.pollination, 0.0);
    }

    #[test]
    fn tick_clears_just_fertilized() {
        let mut z = Zoophily::new(100.0, 200.0);
        z.pollination = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_fertilized);
    }

    #[test]
    fn tick_clears_just_barren() {
        let mut z = z();
        z.pollination = 10.0;
        z.wane(10.0);
        z.tick(0.016);
        assert!(!z.just_barren);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.pollination - 9.0).abs() < 1e-3);
    }

    // --- is_fertilized / is_barren ---

    #[test]
    fn is_fertilized_false_when_disabled() {
        let mut z = z();
        z.pollination = 100.0;
        z.enabled = false;
        assert!(!z.is_fertilized());
    }

    #[test]
    fn is_barren_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_barren());
    }

    // --- pollination_fraction / effective_fecundity ---

    #[test]
    fn pollination_fraction_zero_when_barren() {
        assert_eq!(z().pollination_fraction(), 0.0);
    }

    #[test]
    fn pollination_fraction_half_at_midpoint() {
        let mut z = z();
        z.pollination = 50.0;
        assert!((z.pollination_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_fecundity_zero_when_barren() {
        assert_eq!(z().effective_fecundity(100.0), 0.0);
    }

    #[test]
    fn effective_fecundity_scales_with_pollination() {
        let mut z = z();
        z.pollination = 75.0;
        assert!((z.effective_fecundity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_fecundity_zero_when_disabled() {
        let mut z = z();
        z.pollination = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_fecundity(100.0), 0.0);
    }
}
