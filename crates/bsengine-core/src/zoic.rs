use bevy_ecs::prelude::Component;

/// Fauna-vitality tracker. `vitality` builds via `animate(amount)` and
/// deepens passively at `fauna_rate` per second in `tick(dt)` or
/// diminishes immediately via `diminish(amount)`.
///
/// Models animal-life richness meters, ecosystem-biodiversity fill
/// levels, wildlife-abundance accumulators, zoological-vitality
/// gauges, biome-fauna density trackers, wilderness-fauna health
/// indicators, fauna-population saturation bars, creature-richness
/// progress trackers, or any mechanic where a habitat teems with
/// escalating animal life — from a single scurrying mouse to an
/// explosion of megafauna thundering across the savanna at maximum
/// zoic vitality.
///
/// `animate(amount)` adds vitality; fires `just_teeming` when first
/// reaching `max_vitality`. No-op when disabled.
///
/// `diminish(amount)` reduces vitality immediately; fires `just_barren`
/// when reaching 0. No-op when disabled or already barren.
///
/// `tick(dt)` clears both flags, then increases vitality by
/// `fauna_rate * dt` (capped at `max_vitality`). Fires `just_teeming`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_teeming()` returns `vitality >= max_vitality && enabled`.
///
/// `is_barren()` returns `vitality == 0.0` (not gated by `enabled`).
///
/// `vitality_fraction()` returns `(vitality / max_vitality).clamp(0, 1)`.
///
/// `effective_abundance(scale)` returns `scale * vitality_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — fauna replenishes at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoic {
    pub vitality: f32,
    pub max_vitality: f32,
    pub fauna_rate: f32,
    pub just_teeming: bool,
    pub just_barren: bool,
    pub enabled: bool,
}

impl Zoic {
    pub fn new(max_vitality: f32, fauna_rate: f32) -> Self {
        Self {
            vitality: 0.0,
            max_vitality: max_vitality.max(0.1),
            fauna_rate: fauna_rate.max(0.0),
            just_teeming: false,
            just_barren: false,
            enabled: true,
        }
    }

    /// Add vitality; fires `just_teeming` when first reaching max.
    /// No-op when disabled.
    pub fn animate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.vitality < self.max_vitality;
        self.vitality = (self.vitality + amount).min(self.max_vitality);
        if was_below && self.vitality >= self.max_vitality {
            self.just_teeming = true;
        }
    }

    /// Reduce vitality; fires `just_barren` when reaching 0.
    /// No-op when disabled or already barren.
    pub fn diminish(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.vitality <= 0.0 {
            return;
        }
        self.vitality = (self.vitality - amount).max(0.0);
        if self.vitality <= 0.0 {
            self.just_barren = true;
        }
    }

    /// Clear flags, then increase vitality by `fauna_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_teeming = false;
        self.just_barren = false;
        if self.enabled && self.fauna_rate > 0.0 && self.vitality < self.max_vitality {
            let was_below = self.vitality < self.max_vitality;
            self.vitality = (self.vitality + self.fauna_rate * dt).min(self.max_vitality);
            if was_below && self.vitality >= self.max_vitality {
                self.just_teeming = true;
            }
        }
    }

    /// `true` when vitality is at maximum and component is enabled.
    pub fn is_teeming(&self) -> bool {
        self.vitality >= self.max_vitality && self.enabled
    }

    /// `true` when vitality is 0 (not gated by `enabled`).
    pub fn is_barren(&self) -> bool {
        self.vitality == 0.0
    }

    /// Fraction of maximum vitality [0.0, 1.0].
    pub fn vitality_fraction(&self) -> f32 {
        (self.vitality / self.max_vitality).clamp(0.0, 1.0)
    }

    /// Returns `scale * vitality_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_abundance(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.vitality_fraction()
    }
}

impl Default for Zoic {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoic {
        Zoic::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_barren() {
        let z = z();
        assert_eq!(z.vitality, 0.0);
        assert!(z.is_barren());
        assert!(!z.is_teeming());
    }

    #[test]
    fn new_clamps_max_vitality() {
        let z = Zoic::new(-5.0, 2.0);
        assert!((z.max_vitality - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_fauna_rate() {
        let z = Zoic::new(100.0, -3.0);
        assert_eq!(z.fauna_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoic::default();
        assert!((z.max_vitality - 100.0).abs() < 1e-5);
        assert!((z.fauna_rate - 2.0).abs() < 1e-5);
    }

    // --- animate ---

    #[test]
    fn animate_adds_vitality() {
        let mut z = z();
        z.animate(40.0);
        assert!((z.vitality - 40.0).abs() < 1e-3);
    }

    #[test]
    fn animate_clamps_at_max() {
        let mut z = z();
        z.animate(200.0);
        assert!((z.vitality - 100.0).abs() < 1e-3);
    }

    #[test]
    fn animate_fires_just_teeming_at_max() {
        let mut z = z();
        z.animate(100.0);
        assert!(z.just_teeming);
        assert!(z.is_teeming());
    }

    #[test]
    fn animate_no_just_teeming_when_already_at_max() {
        let mut z = z();
        z.vitality = 100.0;
        z.animate(10.0);
        assert!(!z.just_teeming);
    }

    #[test]
    fn animate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.animate(50.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn animate_no_op_when_amount_zero() {
        let mut z = z();
        z.animate(0.0);
        assert_eq!(z.vitality, 0.0);
    }

    // --- diminish ---

    #[test]
    fn diminish_reduces_vitality() {
        let mut z = z();
        z.vitality = 60.0;
        z.diminish(20.0);
        assert!((z.vitality - 40.0).abs() < 1e-3);
    }

    #[test]
    fn diminish_clamps_at_zero() {
        let mut z = z();
        z.vitality = 30.0;
        z.diminish(200.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn diminish_fires_just_barren_at_zero() {
        let mut z = z();
        z.vitality = 30.0;
        z.diminish(30.0);
        assert!(z.just_barren);
    }

    #[test]
    fn diminish_no_op_when_already_barren() {
        let mut z = z();
        z.diminish(10.0);
        assert!(!z.just_barren);
    }

    #[test]
    fn diminish_no_op_when_disabled() {
        let mut z = z();
        z.vitality = 50.0;
        z.enabled = false;
        z.diminish(50.0);
        assert!((z.vitality - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_replenishes_vitality() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.vitality - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_teeming_on_replenish_to_max() {
        let mut z = Zoic::new(100.0, 200.0);
        z.vitality = 95.0;
        z.tick(1.0);
        assert!(z.just_teeming);
        assert!(z.is_teeming());
    }

    #[test]
    fn tick_no_replenish_when_already_teeming() {
        let mut z = z();
        z.vitality = 100.0;
        z.tick(1.0);
        assert!(!z.just_teeming);
    }

    #[test]
    fn tick_no_replenish_when_rate_zero() {
        let mut z = Zoic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn tick_no_replenish_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn tick_clears_just_teeming() {
        let mut z = Zoic::new(100.0, 200.0);
        z.vitality = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_teeming);
    }

    #[test]
    fn tick_clears_just_barren() {
        let mut z = z();
        z.vitality = 10.0;
        z.diminish(10.0);
        z.tick(0.016);
        assert!(!z.just_barren);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.vitality - 10.0).abs() < 1e-3);
    }

    // --- is_teeming / is_barren ---

    #[test]
    fn is_teeming_false_when_disabled() {
        let mut z = z();
        z.vitality = 100.0;
        z.enabled = false;
        assert!(!z.is_teeming());
    }

    #[test]
    fn is_barren_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_barren());
    }

    // --- vitality_fraction / effective_abundance ---

    #[test]
    fn vitality_fraction_zero_when_barren() {
        assert_eq!(z().vitality_fraction(), 0.0);
    }

    #[test]
    fn vitality_fraction_half_at_midpoint() {
        let mut z = z();
        z.vitality = 50.0;
        assert!((z.vitality_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_abundance_zero_when_barren() {
        assert_eq!(z().effective_abundance(100.0), 0.0);
    }

    #[test]
    fn effective_abundance_scales_with_vitality() {
        let mut z = z();
        z.vitality = 70.0;
        assert!((z.effective_abundance(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_abundance_zero_when_disabled() {
        let mut z = z();
        z.vitality = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_abundance(100.0), 0.0);
    }
}
