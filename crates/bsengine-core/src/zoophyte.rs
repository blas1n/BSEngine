use bevy_ecs::prelude::Component;

/// Sessile-invertebrate vitality tracker named after zoophyte, the
/// historical term for plant-like animals — corals, sea anemones,
/// sponges, hydroids, and bryozoans — that anchor themselves to
/// hard substrate and extend delicate feeding structures into the
/// current, blurring the eighteenth-century boundary between the
/// plant and animal kingdoms. Early naturalists classified them
/// with plants because they branched, rooted, and flowered; later
/// taxonomy sorted them firmly into the animal column, but the
/// poetic name endured. `vitality` builds via `bloom(amount)` and
/// increases passively at `polyp_rate` per second in `tick(dt)` or
/// is reduced via `bleach(amount)`.
///
/// Models coral-reef health fill levels, sessile-colony vitality
/// bars, reef-structure integrity gauges, hydroid-colony density
/// trackers, sea-anemone tentacle-extension saturation meters,
/// bryozoan-mat vitality bars, sponge-tissue health indicators,
/// benthic-community health fill levels, biotic-crust vitality
/// gauges, or any mechanic where a slow-growing sessile colony
/// extends its polyps one careful millimetre per year to build a
/// reef structure that took centuries to assemble — right up until
/// a thermal anomaly bleaches the zooxanthellae, collapses the
/// symbiosis, and reduces the whole ancient structure to bare
/// white rubble in a single summer.
///
/// `bloom(amount)` adds vitality; fires `just_flourished` when
/// first reaching `max_vitality`. No-op when disabled.
///
/// `bleach(amount)` reduces vitality immediately; fires
/// `just_withered` when reaching 0. No-op when disabled or already
/// withered.
///
/// `tick(dt)` clears both flags, then increases vitality by
/// `polyp_rate * dt` (capped at `max_vitality`). Fires
/// `just_flourished` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_flourished()` returns `vitality >= max_vitality && enabled`.
///
/// `is_withered()` returns `vitality == 0.0` (not gated by `enabled`).
///
/// `vitality_fraction()` returns `(vitality / max_vitality).clamp(0, 1)`.
///
/// `effective_colony(scale)` returns `scale * vitality_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — grows at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoophyte {
    pub vitality: f32,
    pub max_vitality: f32,
    pub polyp_rate: f32,
    pub just_flourished: bool,
    pub just_withered: bool,
    pub enabled: bool,
}

impl Zoophyte {
    pub fn new(max_vitality: f32, polyp_rate: f32) -> Self {
        Self {
            vitality: 0.0,
            max_vitality: max_vitality.max(0.1),
            polyp_rate: polyp_rate.max(0.0),
            just_flourished: false,
            just_withered: false,
            enabled: true,
        }
    }

    /// Add vitality; fires `just_flourished` when first reaching max.
    /// No-op when disabled.
    pub fn bloom(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.vitality < self.max_vitality;
        self.vitality = (self.vitality + amount).min(self.max_vitality);
        if was_below && self.vitality >= self.max_vitality {
            self.just_flourished = true;
        }
    }

    /// Reduce vitality; fires `just_withered` when reaching 0.
    /// No-op when disabled or already withered.
    pub fn bleach(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.vitality <= 0.0 {
            return;
        }
        self.vitality = (self.vitality - amount).max(0.0);
        if self.vitality <= 0.0 {
            self.just_withered = true;
        }
    }

    /// Clear flags, then increase vitality by `polyp_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_flourished = false;
        self.just_withered = false;
        if self.enabled && self.polyp_rate > 0.0 && self.vitality < self.max_vitality {
            let was_below = self.vitality < self.max_vitality;
            self.vitality = (self.vitality + self.polyp_rate * dt).min(self.max_vitality);
            if was_below && self.vitality >= self.max_vitality {
                self.just_flourished = true;
            }
        }
    }

    /// `true` when vitality is at maximum and component is enabled.
    pub fn is_flourished(&self) -> bool {
        self.vitality >= self.max_vitality && self.enabled
    }

    /// `true` when vitality is 0 (not gated by `enabled`).
    pub fn is_withered(&self) -> bool {
        self.vitality == 0.0
    }

    /// Fraction of maximum vitality [0.0, 1.0].
    pub fn vitality_fraction(&self) -> f32 {
        (self.vitality / self.max_vitality).clamp(0.0, 1.0)
    }

    /// Returns `scale * vitality_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_colony(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.vitality_fraction()
    }
}

impl Default for Zoophyte {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoophyte {
        Zoophyte::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_withered() {
        let z = z();
        assert_eq!(z.vitality, 0.0);
        assert!(z.is_withered());
        assert!(!z.is_flourished());
    }

    #[test]
    fn new_clamps_max_vitality() {
        let z = Zoophyte::new(-5.0, 1.5);
        assert!((z.max_vitality - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_polyp_rate() {
        let z = Zoophyte::new(100.0, -1.5);
        assert_eq!(z.polyp_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoophyte::default();
        assert!((z.max_vitality - 100.0).abs() < 1e-5);
        assert!((z.polyp_rate - 1.5).abs() < 1e-5);
    }

    // --- bloom ---

    #[test]
    fn bloom_adds_vitality() {
        let mut z = z();
        z.bloom(40.0);
        assert!((z.vitality - 40.0).abs() < 1e-3);
    }

    #[test]
    fn bloom_clamps_at_max() {
        let mut z = z();
        z.bloom(200.0);
        assert!((z.vitality - 100.0).abs() < 1e-3);
    }

    #[test]
    fn bloom_fires_just_flourished_at_max() {
        let mut z = z();
        z.bloom(100.0);
        assert!(z.just_flourished);
        assert!(z.is_flourished());
    }

    #[test]
    fn bloom_no_just_flourished_when_already_at_max() {
        let mut z = z();
        z.vitality = 100.0;
        z.bloom(10.0);
        assert!(!z.just_flourished);
    }

    #[test]
    fn bloom_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.bloom(50.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn bloom_no_op_when_amount_zero() {
        let mut z = z();
        z.bloom(0.0);
        assert_eq!(z.vitality, 0.0);
    }

    // --- bleach ---

    #[test]
    fn bleach_reduces_vitality() {
        let mut z = z();
        z.vitality = 60.0;
        z.bleach(20.0);
        assert!((z.vitality - 40.0).abs() < 1e-3);
    }

    #[test]
    fn bleach_clamps_at_zero() {
        let mut z = z();
        z.vitality = 30.0;
        z.bleach(200.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn bleach_fires_just_withered_at_zero() {
        let mut z = z();
        z.vitality = 30.0;
        z.bleach(30.0);
        assert!(z.just_withered);
    }

    #[test]
    fn bleach_no_op_when_already_withered() {
        let mut z = z();
        z.bleach(10.0);
        assert!(!z.just_withered);
    }

    #[test]
    fn bleach_no_op_when_disabled() {
        let mut z = z();
        z.vitality = 50.0;
        z.enabled = false;
        z.bleach(50.0);
        assert!((z.vitality - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_grows_vitality() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.vitality - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_flourished_on_grow_to_max() {
        let mut z = Zoophyte::new(100.0, 200.0);
        z.vitality = 95.0;
        z.tick(1.0);
        assert!(z.just_flourished);
        assert!(z.is_flourished());
    }

    #[test]
    fn tick_no_grow_when_already_flourished() {
        let mut z = z();
        z.vitality = 100.0;
        z.tick(1.0);
        assert!(!z.just_flourished);
    }

    #[test]
    fn tick_no_grow_when_rate_zero() {
        let mut z = Zoophyte::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn tick_no_grow_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn tick_clears_just_flourished() {
        let mut z = Zoophyte::new(100.0, 200.0);
        z.vitality = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_flourished);
    }

    #[test]
    fn tick_clears_just_withered() {
        let mut z = z();
        z.vitality = 10.0;
        z.bleach(10.0);
        z.tick(0.016);
        assert!(!z.just_withered);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.vitality - 9.0).abs() < 1e-3);
    }

    // --- is_flourished / is_withered ---

    #[test]
    fn is_flourished_false_when_disabled() {
        let mut z = z();
        z.vitality = 100.0;
        z.enabled = false;
        assert!(!z.is_flourished());
    }

    #[test]
    fn is_withered_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_withered());
    }

    // --- vitality_fraction / effective_colony ---

    #[test]
    fn vitality_fraction_zero_when_withered() {
        assert_eq!(z().vitality_fraction(), 0.0);
    }

    #[test]
    fn vitality_fraction_half_at_midpoint() {
        let mut z = z();
        z.vitality = 50.0;
        assert!((z.vitality_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_colony_zero_when_withered() {
        assert_eq!(z().effective_colony(100.0), 0.0);
    }

    #[test]
    fn effective_colony_scales_with_vitality() {
        let mut z = z();
        z.vitality = 75.0;
        assert!((z.effective_colony(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_colony_zero_when_disabled() {
        let mut z = z();
        z.vitality = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_colony(100.0), 0.0);
    }
}
