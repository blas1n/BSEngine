use bevy_ecs::prelude::Component;

/// Bacterial-biofilm-colony tracker named after the zooglea — a mass
/// of bacteria embedded in a gelatinous matrix, forming quivering
/// jelly-like colonies on organic substrates. `biofilm` builds via
/// `culture(amount)` and grows passively at `culture_rate` per
/// second in `tick(dt)` or is disrupted via `disrupt(amount)`.
///
/// Models bacterial-biofilm saturation bars, gelatinous-colony
/// density gauges, wastewater-treatment floc fill levels, activated-
/// sludge density meters, biofilm-reactor culture-accumulation
/// trackers, bacterial-mat encrustation indicators, quorum-sensing
/// threshold-proximity bars, slime-layer build-up gauges,
/// microbial-aggregate cohesion meters, or any mechanic where a
/// population of single-celled organisms stops swimming, extrudes
/// a protective polysaccharide coat, adheres to a surface, and
/// quietly builds a quivering gelatinous colony that doubles in
/// density every few hours — until an antibiotic flush or
/// hydraulic shock strips it back to bare substrate and the whole
/// slow process of re-colonization has to begin again.
///
/// `culture(amount)` adds biofilm; fires `just_encrusted` when
/// first reaching `max_biofilm`. No-op when disabled.
///
/// `disrupt(amount)` reduces biofilm immediately; fires
/// `just_dispersed` when reaching 0. No-op when disabled or
/// already dispersed.
///
/// `tick(dt)` clears both flags, then increases biofilm by
/// `culture_rate * dt` (capped at `max_biofilm`). Fires
/// `just_encrusted` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_encrusted()` returns `biofilm >= max_biofilm && enabled`.
///
/// `is_dispersed()` returns `biofilm == 0.0` (not gated by `enabled`).
///
/// `biofilm_fraction()` returns `(biofilm / max_biofilm).clamp(0, 1)`.
///
/// `effective_viscosity(scale)` returns `scale * biofilm_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — cultures at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooglea {
    pub biofilm: f32,
    pub max_biofilm: f32,
    pub culture_rate: f32,
    pub just_encrusted: bool,
    pub just_dispersed: bool,
    pub enabled: bool,
}

impl Zooglea {
    pub fn new(max_biofilm: f32, culture_rate: f32) -> Self {
        Self {
            biofilm: 0.0,
            max_biofilm: max_biofilm.max(0.1),
            culture_rate: culture_rate.max(0.0),
            just_encrusted: false,
            just_dispersed: false,
            enabled: true,
        }
    }

    /// Add biofilm; fires `just_encrusted` when first reaching max.
    /// No-op when disabled.
    pub fn culture(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.biofilm < self.max_biofilm;
        self.biofilm = (self.biofilm + amount).min(self.max_biofilm);
        if was_below && self.biofilm >= self.max_biofilm {
            self.just_encrusted = true;
        }
    }

    /// Reduce biofilm; fires `just_dispersed` when reaching 0.
    /// No-op when disabled or already dispersed.
    pub fn disrupt(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.biofilm <= 0.0 {
            return;
        }
        self.biofilm = (self.biofilm - amount).max(0.0);
        if self.biofilm <= 0.0 {
            self.just_dispersed = true;
        }
    }

    /// Clear flags, then increase biofilm by `culture_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_encrusted = false;
        self.just_dispersed = false;
        if self.enabled && self.culture_rate > 0.0 && self.biofilm < self.max_biofilm {
            let was_below = self.biofilm < self.max_biofilm;
            self.biofilm = (self.biofilm + self.culture_rate * dt).min(self.max_biofilm);
            if was_below && self.biofilm >= self.max_biofilm {
                self.just_encrusted = true;
            }
        }
    }

    /// `true` when biofilm is at maximum and component is enabled.
    pub fn is_encrusted(&self) -> bool {
        self.biofilm >= self.max_biofilm && self.enabled
    }

    /// `true` when biofilm is 0 (not gated by `enabled`).
    pub fn is_dispersed(&self) -> bool {
        self.biofilm == 0.0
    }

    /// Fraction of maximum biofilm [0.0, 1.0].
    pub fn biofilm_fraction(&self) -> f32 {
        (self.biofilm / self.max_biofilm).clamp(0.0, 1.0)
    }

    /// Returns `scale * biofilm_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_viscosity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.biofilm_fraction()
    }
}

impl Default for Zooglea {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zooglea {
        Zooglea::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dispersed() {
        let z = z();
        assert_eq!(z.biofilm, 0.0);
        assert!(z.is_dispersed());
        assert!(!z.is_encrusted());
    }

    #[test]
    fn new_clamps_max_biofilm() {
        let z = Zooglea::new(-5.0, 1.5);
        assert!((z.max_biofilm - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_culture_rate() {
        let z = Zooglea::new(100.0, -1.5);
        assert_eq!(z.culture_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zooglea::default();
        assert!((z.max_biofilm - 100.0).abs() < 1e-5);
        assert!((z.culture_rate - 1.5).abs() < 1e-5);
    }

    // --- culture ---

    #[test]
    fn culture_adds_biofilm() {
        let mut z = z();
        z.culture(40.0);
        assert!((z.biofilm - 40.0).abs() < 1e-3);
    }

    #[test]
    fn culture_clamps_at_max() {
        let mut z = z();
        z.culture(200.0);
        assert!((z.biofilm - 100.0).abs() < 1e-3);
    }

    #[test]
    fn culture_fires_just_encrusted_at_max() {
        let mut z = z();
        z.culture(100.0);
        assert!(z.just_encrusted);
        assert!(z.is_encrusted());
    }

    #[test]
    fn culture_no_just_encrusted_when_already_at_max() {
        let mut z = z();
        z.biofilm = 100.0;
        z.culture(10.0);
        assert!(!z.just_encrusted);
    }

    #[test]
    fn culture_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.culture(50.0);
        assert_eq!(z.biofilm, 0.0);
    }

    #[test]
    fn culture_no_op_when_amount_zero() {
        let mut z = z();
        z.culture(0.0);
        assert_eq!(z.biofilm, 0.0);
    }

    // --- disrupt ---

    #[test]
    fn disrupt_reduces_biofilm() {
        let mut z = z();
        z.biofilm = 60.0;
        z.disrupt(20.0);
        assert!((z.biofilm - 40.0).abs() < 1e-3);
    }

    #[test]
    fn disrupt_clamps_at_zero() {
        let mut z = z();
        z.biofilm = 30.0;
        z.disrupt(200.0);
        assert_eq!(z.biofilm, 0.0);
    }

    #[test]
    fn disrupt_fires_just_dispersed_at_zero() {
        let mut z = z();
        z.biofilm = 30.0;
        z.disrupt(30.0);
        assert!(z.just_dispersed);
    }

    #[test]
    fn disrupt_no_op_when_already_dispersed() {
        let mut z = z();
        z.disrupt(10.0);
        assert!(!z.just_dispersed);
    }

    #[test]
    fn disrupt_no_op_when_disabled() {
        let mut z = z();
        z.biofilm = 50.0;
        z.enabled = false;
        z.disrupt(50.0);
        assert!((z.biofilm - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_cultures_biofilm() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.biofilm - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_encrusted_on_culture_to_max() {
        let mut z = Zooglea::new(100.0, 200.0);
        z.biofilm = 95.0;
        z.tick(1.0);
        assert!(z.just_encrusted);
        assert!(z.is_encrusted());
    }

    #[test]
    fn tick_no_culture_when_already_encrusted() {
        let mut z = z();
        z.biofilm = 100.0;
        z.tick(1.0);
        assert!(!z.just_encrusted);
    }

    #[test]
    fn tick_no_culture_when_rate_zero() {
        let mut z = Zooglea::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.biofilm, 0.0);
    }

    #[test]
    fn tick_no_culture_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.biofilm, 0.0);
    }

    #[test]
    fn tick_clears_just_encrusted() {
        let mut z = Zooglea::new(100.0, 200.0);
        z.biofilm = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_encrusted);
    }

    #[test]
    fn tick_clears_just_dispersed() {
        let mut z = z();
        z.biofilm = 10.0;
        z.disrupt(10.0);
        z.tick(0.016);
        assert!(!z.just_dispersed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.biofilm - 9.0).abs() < 1e-3);
    }

    // --- is_encrusted / is_dispersed ---

    #[test]
    fn is_encrusted_false_when_disabled() {
        let mut z = z();
        z.biofilm = 100.0;
        z.enabled = false;
        assert!(!z.is_encrusted());
    }

    #[test]
    fn is_dispersed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_dispersed());
    }

    // --- biofilm_fraction / effective_viscosity ---

    #[test]
    fn biofilm_fraction_zero_when_dispersed() {
        assert_eq!(z().biofilm_fraction(), 0.0);
    }

    #[test]
    fn biofilm_fraction_half_at_midpoint() {
        let mut z = z();
        z.biofilm = 50.0;
        assert!((z.biofilm_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_viscosity_zero_when_dispersed() {
        assert_eq!(z().effective_viscosity(100.0), 0.0);
    }

    #[test]
    fn effective_viscosity_scales_with_biofilm() {
        let mut z = z();
        z.biofilm = 75.0;
        assert!((z.effective_viscosity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_viscosity_zero_when_disabled() {
        let mut z = z();
        z.biofilm = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_viscosity(100.0), 0.0);
    }
}
