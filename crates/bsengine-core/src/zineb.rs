use bevy_ecs::prelude::Component;

/// Fungicide-concentration accumulation tracker named after zineb, the
/// organosulfur fungicide whose chemical name is zinc
/// ethylenebisdithiocarbamate — a coordination compound in which a
/// zinc ion is chelated by the dithiocarbamate groups of
/// ethylenediamine to form a polymeric structure that disrupts fungal
/// enzyme systems, particularly those requiring sulfhydryl groups, by
/// releasing carbon disulfide and ethylenediamine thiuram disulfide as
/// metabolites upon contact with the fungal cell. Developed in the
/// early 1940s alongside the related fungicides maneb and nabam, zineb
/// became one of the most widely used protective fungicides in
/// twentieth-century agriculture — sprayed onto potatoes to prevent
/// late blight (Phytophthora infestans), onto grapes to control
/// downy mildew (Plasmopara viticola), onto tomatoes, onions, carrots,
/// and ornamental roses against a broad spectrum of Oomycetes and
/// Ascomycetes. As a protective rather than curative fungicide, zineb
/// works best when applied prophylactically — before the pathogen
/// arrives — because it forms a chemical shield across the leaf surface
/// that stops germinating spores before they can penetrate the cuticle.
/// Once infection is established, it cannot eradicate the fungus
/// already inside the tissue; the key is accumulation before the
/// disease front arrives and maintenance of residue levels through
/// periodic reapplication as weathering and new leaf growth dilute
/// the protective layer. `residue` builds via `apply(amount)` and
/// accumulates passively at `spray_rate` per second in `tick(dt)` or
/// is reduced via `weather(amount)`.
///
/// Models fungicide-residue fill levels, foliar-protection saturation
/// bars, prophylactic-spray accumulators, blight-resistance gauges,
/// crop-protection fill levels, pathogen-exclusion saturation
/// indicators, antifungal-coverage accumulation bars, plant-disease
/// prevention meters, organosulfur-compound fill levels, or any
/// mechanic where a plant, zone, or system slowly builds up a
/// protective chemical barrier — layer by layer, spray by spray —
/// until the surface concentration is high enough that any arriving
/// fungal spore dissolves its enzyme complement on contact and dies
/// before it can breach the cuticle.
///
/// `apply(amount)` adds residue; fires `just_protected` when first
/// reaching `max_residue`. No-op when disabled.
///
/// `weather(amount)` reduces residue immediately; fires `just_exposed`
/// when reaching 0. No-op when disabled or already exposed.
///
/// `tick(dt)` clears both flags, then increases residue by
/// `spray_rate * dt` (capped at `max_residue`). Fires `just_protected`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_protected()` returns `residue >= max_residue && enabled`.
///
/// `is_exposed()` returns `residue == 0.0` (not gated by `enabled`).
///
/// `residue_fraction()` returns
/// `(residue / max_residue).clamp(0, 1)`.
///
/// `effective_coverage(scale)` returns `scale * residue_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — sprays at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zineb {
    pub residue: f32,
    pub max_residue: f32,
    pub spray_rate: f32,
    pub just_protected: bool,
    pub just_exposed: bool,
    pub enabled: bool,
}

impl Zineb {
    pub fn new(max_residue: f32, spray_rate: f32) -> Self {
        Self {
            residue: 0.0,
            max_residue: max_residue.max(0.1),
            spray_rate: spray_rate.max(0.0),
            just_protected: false,
            just_exposed: false,
            enabled: true,
        }
    }

    /// Add residue; fires `just_protected` when first reaching max.
    /// No-op when disabled.
    pub fn apply(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.residue < self.max_residue;
        self.residue = (self.residue + amount).min(self.max_residue);
        if was_below && self.residue >= self.max_residue {
            self.just_protected = true;
        }
    }

    /// Reduce residue; fires `just_exposed` when reaching 0.
    /// No-op when disabled or already exposed.
    pub fn weather(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.residue <= 0.0 {
            return;
        }
        self.residue = (self.residue - amount).max(0.0);
        if self.residue <= 0.0 {
            self.just_exposed = true;
        }
    }

    /// Clear flags, then increase residue by `spray_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_protected = false;
        self.just_exposed = false;
        if self.enabled && self.spray_rate > 0.0 && self.residue < self.max_residue {
            let was_below = self.residue < self.max_residue;
            self.residue = (self.residue + self.spray_rate * dt).min(self.max_residue);
            if was_below && self.residue >= self.max_residue {
                self.just_protected = true;
            }
        }
    }

    /// `true` when residue is at maximum and component is enabled.
    pub fn is_protected(&self) -> bool {
        self.residue >= self.max_residue && self.enabled
    }

    /// `true` when residue is 0 (not gated by `enabled`).
    pub fn is_exposed(&self) -> bool {
        self.residue == 0.0
    }

    /// Fraction of maximum residue [0.0, 1.0].
    pub fn residue_fraction(&self) -> f32 {
        (self.residue / self.max_residue).clamp(0.0, 1.0)
    }

    /// Returns `scale * residue_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_coverage(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.residue_fraction()
    }
}

impl Default for Zineb {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zineb {
        Zineb::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_exposed() {
        let z = z();
        assert_eq!(z.residue, 0.0);
        assert!(z.is_exposed());
        assert!(!z.is_protected());
    }

    #[test]
    fn new_clamps_max_residue() {
        let z = Zineb::new(-5.0, 1.5);
        assert!((z.max_residue - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spray_rate() {
        let z = Zineb::new(100.0, -1.5);
        assert_eq!(z.spray_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zineb::default();
        assert!((z.max_residue - 100.0).abs() < 1e-5);
        assert!((z.spray_rate - 1.5).abs() < 1e-5);
    }

    // --- apply ---

    #[test]
    fn apply_adds_residue() {
        let mut z = z();
        z.apply(40.0);
        assert!((z.residue - 40.0).abs() < 1e-3);
    }

    #[test]
    fn apply_clamps_at_max() {
        let mut z = z();
        z.apply(200.0);
        assert!((z.residue - 100.0).abs() < 1e-3);
    }

    #[test]
    fn apply_fires_just_protected_at_max() {
        let mut z = z();
        z.apply(100.0);
        assert!(z.just_protected);
        assert!(z.is_protected());
    }

    #[test]
    fn apply_no_just_protected_when_already_at_max() {
        let mut z = z();
        z.residue = 100.0;
        z.apply(10.0);
        assert!(!z.just_protected);
    }

    #[test]
    fn apply_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.apply(50.0);
        assert_eq!(z.residue, 0.0);
    }

    #[test]
    fn apply_no_op_when_amount_zero() {
        let mut z = z();
        z.apply(0.0);
        assert_eq!(z.residue, 0.0);
    }

    // --- weather ---

    #[test]
    fn weather_reduces_residue() {
        let mut z = z();
        z.residue = 60.0;
        z.weather(20.0);
        assert!((z.residue - 40.0).abs() < 1e-3);
    }

    #[test]
    fn weather_clamps_at_zero() {
        let mut z = z();
        z.residue = 30.0;
        z.weather(200.0);
        assert_eq!(z.residue, 0.0);
    }

    #[test]
    fn weather_fires_just_exposed_at_zero() {
        let mut z = z();
        z.residue = 30.0;
        z.weather(30.0);
        assert!(z.just_exposed);
    }

    #[test]
    fn weather_no_op_when_already_exposed() {
        let mut z = z();
        z.weather(10.0);
        assert!(!z.just_exposed);
    }

    #[test]
    fn weather_no_op_when_disabled() {
        let mut z = z();
        z.residue = 50.0;
        z.enabled = false;
        z.weather(50.0);
        assert!((z.residue - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_sprays_residue() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.residue - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_protected_on_spray_to_max() {
        let mut z = Zineb::new(100.0, 200.0);
        z.residue = 95.0;
        z.tick(1.0);
        assert!(z.just_protected);
        assert!(z.is_protected());
    }

    #[test]
    fn tick_no_spray_when_already_protected() {
        let mut z = z();
        z.residue = 100.0;
        z.tick(1.0);
        assert!(!z.just_protected);
    }

    #[test]
    fn tick_no_spray_when_rate_zero() {
        let mut z = Zineb::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.residue, 0.0);
    }

    #[test]
    fn tick_no_spray_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.residue, 0.0);
    }

    #[test]
    fn tick_clears_just_protected() {
        let mut z = Zineb::new(100.0, 200.0);
        z.residue = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_protected);
    }

    #[test]
    fn tick_clears_just_exposed() {
        let mut z = z();
        z.residue = 10.0;
        z.weather(10.0);
        z.tick(0.016);
        assert!(!z.just_exposed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.residue - 9.0).abs() < 1e-3);
    }

    // --- is_protected / is_exposed ---

    #[test]
    fn is_protected_false_when_disabled() {
        let mut z = z();
        z.residue = 100.0;
        z.enabled = false;
        assert!(!z.is_protected());
    }

    #[test]
    fn is_exposed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_exposed());
    }

    // --- residue_fraction / effective_coverage ---

    #[test]
    fn residue_fraction_zero_when_exposed() {
        assert_eq!(z().residue_fraction(), 0.0);
    }

    #[test]
    fn residue_fraction_half_at_midpoint() {
        let mut z = z();
        z.residue = 50.0;
        assert!((z.residue_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_coverage_zero_when_exposed() {
        assert_eq!(z().effective_coverage(100.0), 0.0);
    }

    #[test]
    fn effective_coverage_scales_with_residue() {
        let mut z = z();
        z.residue = 75.0;
        assert!((z.effective_coverage(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_coverage_zero_when_disabled() {
        let mut z = z();
        z.residue = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_coverage(100.0), 0.0);
    }
}
