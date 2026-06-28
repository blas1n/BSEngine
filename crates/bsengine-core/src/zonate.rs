use bevy_ecs::prelude::Component;

/// Concentric-banding accumulation tracker named after "zonate", the
/// Merriam-Webster adjective describing structures arranged in or marked
/// by concentric zones — alternating layers of colour, texture, density,
/// or chemistry that record the history of a growing system from its
/// outermost edge back to its first nucleation point. The phenomenon
/// appears across every scale in the natural world: the banded agate
/// whose iron-oxide rings were deposited in a silica gel over millions
/// of years; the zonate lichen whose damp grey crustose thallus spreads
/// outward in rings of younger and older tissue from a dead centre; the
/// zonate colony pattern of a Staphylococcus culture growing on a blood-
/// agar plate; the cross-section of a stalactite whose calcite zones
/// record every wet and dry season since the cave formed; the tree ring
/// that marks each year of growth with a boundary between the last
/// summer's dense latewood and the next spring's pale earlywood. Each
/// zone is a completed chapter — identifiable, measurable, and
/// permanent once laid down. `zonation` builds via `layer(amount)` and
/// accumulates passively at `band_rate` per second in `tick(dt)` or is
/// cleared via `disperse(amount)`.
///
/// Models mineral-banding fill levels, lichen-ring growth saturation
/// bars, stalactite-layer accumulation trackers, agate-band deposition
/// gauges, tree-ring archival fill levels, microbial-colony concentric-
/// zone trackers, crystal-zone saturation bars, sediment-varve
/// accumulation meters, corrosion-layer banding fill levels, or any
/// mechanic where repeated cycles of growth gradually build up a visible
/// record of time, chemistry, and season in perfectly nested rings until
/// the final band seals the outer edge — and any disturbance that
/// dissolves the structure erases every chapter simultaneously.
///
/// `layer(amount)` adds zonation; fires `just_banded` when first
/// reaching `max_zonation`. No-op when disabled.
///
/// `disperse(amount)` reduces zonation immediately; fires
/// `just_dispersed` when reaching 0. No-op when disabled or already
/// dispersed.
///
/// `tick(dt)` clears both flags, then increases zonation by
/// `band_rate * dt` (capped at `max_zonation`). Fires `just_banded`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_banded()` returns `zonation >= max_zonation && enabled`.
///
/// `is_dispersed()` returns `zonation == 0.0` (not gated by `enabled`).
///
/// `zonation_fraction()` returns `(zonation / max_zonation).clamp(0, 1)`.
///
/// `effective_banding(scale)` returns `scale * zonation_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — bands at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zonate {
    pub zonation: f32,
    pub max_zonation: f32,
    pub band_rate: f32,
    pub just_banded: bool,
    pub just_dispersed: bool,
    pub enabled: bool,
}

impl Zonate {
    pub fn new(max_zonation: f32, band_rate: f32) -> Self {
        Self {
            zonation: 0.0,
            max_zonation: max_zonation.max(0.1),
            band_rate: band_rate.max(0.0),
            just_banded: false,
            just_dispersed: false,
            enabled: true,
        }
    }

    /// Add zonation; fires `just_banded` when first reaching max.
    /// No-op when disabled.
    pub fn layer(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.zonation < self.max_zonation;
        self.zonation = (self.zonation + amount).min(self.max_zonation);
        if was_below && self.zonation >= self.max_zonation {
            self.just_banded = true;
        }
    }

    /// Reduce zonation; fires `just_dispersed` when reaching 0.
    /// No-op when disabled or already dispersed.
    pub fn disperse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.zonation <= 0.0 {
            return;
        }
        self.zonation = (self.zonation - amount).max(0.0);
        if self.zonation <= 0.0 {
            self.just_dispersed = true;
        }
    }

    /// Clear flags, then increase zonation by `band_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_banded = false;
        self.just_dispersed = false;
        if self.enabled && self.band_rate > 0.0 && self.zonation < self.max_zonation {
            let was_below = self.zonation < self.max_zonation;
            self.zonation = (self.zonation + self.band_rate * dt).min(self.max_zonation);
            if was_below && self.zonation >= self.max_zonation {
                self.just_banded = true;
            }
        }
    }

    /// `true` when zonation is at maximum and component is enabled.
    pub fn is_banded(&self) -> bool {
        self.zonation >= self.max_zonation && self.enabled
    }

    /// `true` when zonation is 0 (not gated by `enabled`).
    pub fn is_dispersed(&self) -> bool {
        self.zonation == 0.0
    }

    /// Fraction of maximum zonation [0.0, 1.0].
    pub fn zonation_fraction(&self) -> f32 {
        (self.zonation / self.max_zonation).clamp(0.0, 1.0)
    }

    /// Returns `scale * zonation_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_banding(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.zonation_fraction()
    }
}

impl Default for Zonate {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zonate {
        Zonate::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dispersed() {
        let z = z();
        assert_eq!(z.zonation, 0.0);
        assert!(z.is_dispersed());
        assert!(!z.is_banded());
    }

    #[test]
    fn new_clamps_max_zonation() {
        let z = Zonate::new(-5.0, 1.5);
        assert!((z.max_zonation - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_band_rate() {
        let z = Zonate::new(100.0, -1.5);
        assert_eq!(z.band_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zonate::default();
        assert!((z.max_zonation - 100.0).abs() < 1e-5);
        assert!((z.band_rate - 1.5).abs() < 1e-5);
    }

    // --- layer ---

    #[test]
    fn layer_adds_zonation() {
        let mut z = z();
        z.layer(40.0);
        assert!((z.zonation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn layer_clamps_at_max() {
        let mut z = z();
        z.layer(200.0);
        assert!((z.zonation - 100.0).abs() < 1e-3);
    }

    #[test]
    fn layer_fires_just_banded_at_max() {
        let mut z = z();
        z.layer(100.0);
        assert!(z.just_banded);
        assert!(z.is_banded());
    }

    #[test]
    fn layer_no_just_banded_when_already_at_max() {
        let mut z = z();
        z.zonation = 100.0;
        z.layer(10.0);
        assert!(!z.just_banded);
    }

    #[test]
    fn layer_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.layer(50.0);
        assert_eq!(z.zonation, 0.0);
    }

    #[test]
    fn layer_no_op_when_amount_zero() {
        let mut z = z();
        z.layer(0.0);
        assert_eq!(z.zonation, 0.0);
    }

    // --- disperse ---

    #[test]
    fn disperse_reduces_zonation() {
        let mut z = z();
        z.zonation = 60.0;
        z.disperse(20.0);
        assert!((z.zonation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn disperse_clamps_at_zero() {
        let mut z = z();
        z.zonation = 30.0;
        z.disperse(200.0);
        assert_eq!(z.zonation, 0.0);
    }

    #[test]
    fn disperse_fires_just_dispersed_at_zero() {
        let mut z = z();
        z.zonation = 30.0;
        z.disperse(30.0);
        assert!(z.just_dispersed);
    }

    #[test]
    fn disperse_no_op_when_already_dispersed() {
        let mut z = z();
        z.disperse(10.0);
        assert!(!z.just_dispersed);
    }

    #[test]
    fn disperse_no_op_when_disabled() {
        let mut z = z();
        z.zonation = 50.0;
        z.enabled = false;
        z.disperse(50.0);
        assert!((z.zonation - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_bands_zonation() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.zonation - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_banded_on_band_to_max() {
        let mut z = Zonate::new(100.0, 200.0);
        z.zonation = 95.0;
        z.tick(1.0);
        assert!(z.just_banded);
        assert!(z.is_banded());
    }

    #[test]
    fn tick_no_band_when_already_banded() {
        let mut z = z();
        z.zonation = 100.0;
        z.tick(1.0);
        assert!(!z.just_banded);
    }

    #[test]
    fn tick_no_band_when_rate_zero() {
        let mut z = Zonate::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.zonation, 0.0);
    }

    #[test]
    fn tick_no_band_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.zonation, 0.0);
    }

    #[test]
    fn tick_clears_just_banded() {
        let mut z = Zonate::new(100.0, 200.0);
        z.zonation = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_banded);
    }

    #[test]
    fn tick_clears_just_dispersed() {
        let mut z = z();
        z.zonation = 10.0;
        z.disperse(10.0);
        z.tick(0.016);
        assert!(!z.just_dispersed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.zonation - 9.0).abs() < 1e-3);
    }

    // --- is_banded / is_dispersed ---

    #[test]
    fn is_banded_false_when_disabled() {
        let mut z = z();
        z.zonation = 100.0;
        z.enabled = false;
        assert!(!z.is_banded());
    }

    #[test]
    fn is_dispersed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_dispersed());
    }

    // --- zonation_fraction / effective_banding ---

    #[test]
    fn zonation_fraction_zero_when_dispersed() {
        assert_eq!(z().zonation_fraction(), 0.0);
    }

    #[test]
    fn zonation_fraction_half_at_midpoint() {
        let mut z = z();
        z.zonation = 50.0;
        assert!((z.zonation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_banding_zero_when_dispersed() {
        assert_eq!(z().effective_banding(100.0), 0.0);
    }

    #[test]
    fn effective_banding_scales_with_zonation() {
        let mut z = z();
        z.zonation = 75.0;
        assert!((z.effective_banding(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_banding_zero_when_disabled() {
        let mut z = z();
        z.zonation = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_banding(100.0), 0.0);
    }
}
