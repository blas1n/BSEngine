use bevy_ecs::prelude::Component;

/// Animal-mutualism accumulation tracker named after zoophilous, the
/// botanical adjective describing plants that are pollinated by animals
/// — or more broadly any organism adapted to attract, rely upon, or
/// coexist with animal partners. The term encompasses an enormous
/// range of mutualistic relationships: fig trees and their obligate
/// fig wasps; yuccas and yucca moths; the vast guild of flowering
/// plants that reward bats, birds, beetles, and bees with nectar in
/// exchange for pollen transport. Zoophilous pollination is contrasted
/// with anemophily (wind pollination): wind-pollinated plants invest
/// heavily in pollen quantity, producing clouds of lightweight grains
/// on bare catkins; zoophilous plants instead invest in advertising —
/// petals, nectar, scent, and sometimes elaborate mimicry of insects
/// or fungi. The energetic bargain of zoophily is that it trades
/// massive pollen waste for targeted delivery, dramatically increasing
/// the chance that a grain reaches a conspecific stigma. Beyond
/// pollination, zoophily extends to seed dispersal: fleshy fruits
/// are advertisements to frugivores, whose digestive passage scarifies
/// tough seed coats and deposits the seed far from the parent plant
/// wrapped in a plug of fertiliser. `affinity` builds via
/// `attract(amount)` and accumulates passively at `attract_rate`
/// per second in `tick(dt)` or diminishes via `repel(amount)`.
///
/// Models animal-mutualism fill levels, pollinator-attraction
/// saturation bars, frugivore-dispersal affinity trackers, flower-
/// visitor-reward gauges, zoophilous-adaptation fill levels,
/// nectar-production saturation indicators, coevolution-commitment
/// accumulation bars, bird-pollinator attraction meters, bat-
/// visitation fill levels, or any mechanic where an organism slowly
/// perfects its advertising to animal mutualists — colour, scent,
/// nectar yield, landing-platform geometry — until the partnership
/// is so entrenched that neither partner can reproduce without the
/// other.
///
/// `attract(amount)` adds affinity; fires `just_adapted` when first
/// reaching `max_affinity`. No-op when disabled.
///
/// `repel(amount)` reduces affinity immediately; fires `just_repelled`
/// when reaching 0. No-op when disabled or already repelled.
///
/// `tick(dt)` clears both flags, then increases affinity by
/// `attract_rate * dt` (capped at `max_affinity`). Fires `just_adapted`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_adapted()` returns `affinity >= max_affinity && enabled`.
///
/// `is_repelled()` returns `affinity == 0.0` (not gated by `enabled`).
///
/// `affinity_fraction()` returns
/// `(affinity / max_affinity).clamp(0, 1)`.
///
/// `effective_mutualism(scale)` returns `scale * affinity_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — attracts at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoophilous {
    pub affinity: f32,
    pub max_affinity: f32,
    pub attract_rate: f32,
    pub just_adapted: bool,
    pub just_repelled: bool,
    pub enabled: bool,
}

impl Zoophilous {
    pub fn new(max_affinity: f32, attract_rate: f32) -> Self {
        Self {
            affinity: 0.0,
            max_affinity: max_affinity.max(0.1),
            attract_rate: attract_rate.max(0.0),
            just_adapted: false,
            just_repelled: false,
            enabled: true,
        }
    }

    /// Add affinity; fires `just_adapted` when first reaching max.
    /// No-op when disabled.
    pub fn attract(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.affinity < self.max_affinity;
        self.affinity = (self.affinity + amount).min(self.max_affinity);
        if was_below && self.affinity >= self.max_affinity {
            self.just_adapted = true;
        }
    }

    /// Reduce affinity; fires `just_repelled` when reaching 0.
    /// No-op when disabled or already repelled.
    pub fn repel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.affinity <= 0.0 {
            return;
        }
        self.affinity = (self.affinity - amount).max(0.0);
        if self.affinity <= 0.0 {
            self.just_repelled = true;
        }
    }

    /// Clear flags, then increase affinity by `attract_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_adapted = false;
        self.just_repelled = false;
        if self.enabled && self.attract_rate > 0.0 && self.affinity < self.max_affinity {
            let was_below = self.affinity < self.max_affinity;
            self.affinity = (self.affinity + self.attract_rate * dt).min(self.max_affinity);
            if was_below && self.affinity >= self.max_affinity {
                self.just_adapted = true;
            }
        }
    }

    /// `true` when affinity is at maximum and component is enabled.
    pub fn is_adapted(&self) -> bool {
        self.affinity >= self.max_affinity && self.enabled
    }

    /// `true` when affinity is 0 (not gated by `enabled`).
    pub fn is_repelled(&self) -> bool {
        self.affinity == 0.0
    }

    /// Fraction of maximum affinity [0.0, 1.0].
    pub fn affinity_fraction(&self) -> f32 {
        (self.affinity / self.max_affinity).clamp(0.0, 1.0)
    }

    /// Returns `scale * affinity_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_mutualism(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.affinity_fraction()
    }
}

impl Default for Zoophilous {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoophilous {
        Zoophilous::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_repelled() {
        let z = z();
        assert_eq!(z.affinity, 0.0);
        assert!(z.is_repelled());
        assert!(!z.is_adapted());
    }

    #[test]
    fn new_clamps_max_affinity() {
        let z = Zoophilous::new(-5.0, 1.5);
        assert!((z.max_affinity - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_attract_rate() {
        let z = Zoophilous::new(100.0, -1.5);
        assert_eq!(z.attract_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoophilous::default();
        assert!((z.max_affinity - 100.0).abs() < 1e-5);
        assert!((z.attract_rate - 1.5).abs() < 1e-5);
    }

    // --- attract ---

    #[test]
    fn attract_adds_affinity() {
        let mut z = z();
        z.attract(40.0);
        assert!((z.affinity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn attract_clamps_at_max() {
        let mut z = z();
        z.attract(200.0);
        assert!((z.affinity - 100.0).abs() < 1e-3);
    }

    #[test]
    fn attract_fires_just_adapted_at_max() {
        let mut z = z();
        z.attract(100.0);
        assert!(z.just_adapted);
        assert!(z.is_adapted());
    }

    #[test]
    fn attract_no_just_adapted_when_already_at_max() {
        let mut z = z();
        z.affinity = 100.0;
        z.attract(10.0);
        assert!(!z.just_adapted);
    }

    #[test]
    fn attract_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.attract(50.0);
        assert_eq!(z.affinity, 0.0);
    }

    #[test]
    fn attract_no_op_when_amount_zero() {
        let mut z = z();
        z.attract(0.0);
        assert_eq!(z.affinity, 0.0);
    }

    // --- repel ---

    #[test]
    fn repel_reduces_affinity() {
        let mut z = z();
        z.affinity = 60.0;
        z.repel(20.0);
        assert!((z.affinity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn repel_clamps_at_zero() {
        let mut z = z();
        z.affinity = 30.0;
        z.repel(200.0);
        assert_eq!(z.affinity, 0.0);
    }

    #[test]
    fn repel_fires_just_repelled_at_zero() {
        let mut z = z();
        z.affinity = 30.0;
        z.repel(30.0);
        assert!(z.just_repelled);
    }

    #[test]
    fn repel_no_op_when_already_repelled() {
        let mut z = z();
        z.repel(10.0);
        assert!(!z.just_repelled);
    }

    #[test]
    fn repel_no_op_when_disabled() {
        let mut z = z();
        z.affinity = 50.0;
        z.enabled = false;
        z.repel(50.0);
        assert!((z.affinity - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_attracts_affinity() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.affinity - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_adapted_on_attract_to_max() {
        let mut z = Zoophilous::new(100.0, 200.0);
        z.affinity = 95.0;
        z.tick(1.0);
        assert!(z.just_adapted);
        assert!(z.is_adapted());
    }

    #[test]
    fn tick_no_attract_when_already_adapted() {
        let mut z = z();
        z.affinity = 100.0;
        z.tick(1.0);
        assert!(!z.just_adapted);
    }

    #[test]
    fn tick_no_attract_when_rate_zero() {
        let mut z = Zoophilous::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.affinity, 0.0);
    }

    #[test]
    fn tick_no_attract_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.affinity, 0.0);
    }

    #[test]
    fn tick_clears_just_adapted() {
        let mut z = Zoophilous::new(100.0, 200.0);
        z.affinity = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_adapted);
    }

    #[test]
    fn tick_clears_just_repelled() {
        let mut z = z();
        z.affinity = 10.0;
        z.repel(10.0);
        z.tick(0.016);
        assert!(!z.just_repelled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.affinity - 9.0).abs() < 1e-3);
    }

    // --- is_adapted / is_repelled ---

    #[test]
    fn is_adapted_false_when_disabled() {
        let mut z = z();
        z.affinity = 100.0;
        z.enabled = false;
        assert!(!z.is_adapted());
    }

    #[test]
    fn is_repelled_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_repelled());
    }

    // --- affinity_fraction / effective_mutualism ---

    #[test]
    fn affinity_fraction_zero_when_repelled() {
        assert_eq!(z().affinity_fraction(), 0.0);
    }

    #[test]
    fn affinity_fraction_half_at_midpoint() {
        let mut z = z();
        z.affinity = 50.0;
        assert!((z.affinity_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_mutualism_zero_when_repelled() {
        assert_eq!(z().effective_mutualism(100.0), 0.0);
    }

    #[test]
    fn effective_mutualism_scales_with_affinity() {
        let mut z = z();
        z.affinity = 75.0;
        assert!((z.effective_mutualism(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_mutualism_zero_when_disabled() {
        let mut z = z();
        z.affinity = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_mutualism(100.0), 0.0);
    }
}
