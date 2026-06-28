use bevy_ecs::prelude::Component;

/// Bilateral-symmetry accumulation tracker named after zygomorphic,
/// the botanical adjective describing a flower — or more broadly any
/// organism or structure — that is bilaterally symmetrical: divisible
/// into two mirror-image halves along exactly one plane rather than
/// many. The word derives from Greek zygon (yoke or pair) plus
/// morphe (form), and it stands in contrast to actinomorphic (radially
/// symmetrical, like a buttercup or sea anemone). Zygomorphic flowers
/// are among the most elaborate products of co-evolution between plants
/// and their pollinators: an orchid whose labellum guides a bee into
/// precisely the right position to pick up or deposit pollen, a
/// snapdragon whose hinged lip admits a heavy bumblebee but excludes
/// lighter insects, a monkey-face orchid whose bilateral bilateral
/// symmetry mimics the face of a primate — all of these are
/// zygomorphic. The condition arises from the selective expression
/// of CYCLOIDEA-family genes, which suppress dorsal petal development
/// on one side of the floral meristem; mutations in these genes revert
/// a zygomorphic flower to actinomorphy, demonstrating that bilateral
/// symmetry is an acquired, maintained state rather than a default.
/// `symmetry` builds via `align(amount)` and accumulates passively
/// at `align_rate` per second in `tick(dt)` or retreats via
/// `distort(amount)`.
///
/// Models bilateral-symmetry fill levels, body-plan alignment
/// saturation bars, structural-balance accumulation trackers,
/// mirror-image fidelity gauges, morphological-precision fill levels,
/// skeletal-laterality saturation indicators, body-axis coherence
/// accumulation bars, bilateral-defence orientation meters,
/// choreographic-mirror-step fill levels, or any mechanic where a
/// body, formation, or design slowly achieves perfect bilateral
/// correspondence between its left and right halves — and where
/// injury, mutation, or disorder shatters that correspondence back
/// into asymmetric chaos.
///
/// `align(amount)` adds symmetry; fires `just_symmetric` when first
/// reaching `max_symmetry`. No-op when disabled.
///
/// `distort(amount)` reduces symmetry immediately; fires
/// `just_asymmetric` when reaching 0. No-op when disabled or already
/// asymmetric.
///
/// `tick(dt)` clears both flags, then increases symmetry by
/// `align_rate * dt` (capped at `max_symmetry`). Fires `just_symmetric`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_symmetric()` returns `symmetry >= max_symmetry && enabled`.
///
/// `is_asymmetric()` returns `symmetry == 0.0` (not gated by
/// `enabled`).
///
/// `symmetry_fraction()` returns
/// `(symmetry / max_symmetry).clamp(0, 1)`.
///
/// `effective_bilateral(scale)` returns `scale * symmetry_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — aligns at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zygomorphic {
    pub symmetry: f32,
    pub max_symmetry: f32,
    pub align_rate: f32,
    pub just_symmetric: bool,
    pub just_asymmetric: bool,
    pub enabled: bool,
}

impl Zygomorphic {
    pub fn new(max_symmetry: f32, align_rate: f32) -> Self {
        Self {
            symmetry: 0.0,
            max_symmetry: max_symmetry.max(0.1),
            align_rate: align_rate.max(0.0),
            just_symmetric: false,
            just_asymmetric: false,
            enabled: true,
        }
    }

    /// Add symmetry; fires `just_symmetric` when first reaching max.
    /// No-op when disabled.
    pub fn align(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.symmetry < self.max_symmetry;
        self.symmetry = (self.symmetry + amount).min(self.max_symmetry);
        if was_below && self.symmetry >= self.max_symmetry {
            self.just_symmetric = true;
        }
    }

    /// Reduce symmetry; fires `just_asymmetric` when reaching 0.
    /// No-op when disabled or already asymmetric.
    pub fn distort(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.symmetry <= 0.0 {
            return;
        }
        self.symmetry = (self.symmetry - amount).max(0.0);
        if self.symmetry <= 0.0 {
            self.just_asymmetric = true;
        }
    }

    /// Clear flags, then increase symmetry by `align_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_symmetric = false;
        self.just_asymmetric = false;
        if self.enabled && self.align_rate > 0.0 && self.symmetry < self.max_symmetry {
            let was_below = self.symmetry < self.max_symmetry;
            self.symmetry = (self.symmetry + self.align_rate * dt).min(self.max_symmetry);
            if was_below && self.symmetry >= self.max_symmetry {
                self.just_symmetric = true;
            }
        }
    }

    /// `true` when symmetry is at maximum and component is enabled.
    pub fn is_symmetric(&self) -> bool {
        self.symmetry >= self.max_symmetry && self.enabled
    }

    /// `true` when symmetry is 0 (not gated by `enabled`).
    pub fn is_asymmetric(&self) -> bool {
        self.symmetry == 0.0
    }

    /// Fraction of maximum symmetry [0.0, 1.0].
    pub fn symmetry_fraction(&self) -> f32 {
        (self.symmetry / self.max_symmetry).clamp(0.0, 1.0)
    }

    /// Returns `scale * symmetry_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_bilateral(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.symmetry_fraction()
    }
}

impl Default for Zygomorphic {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zygomorphic {
        Zygomorphic::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_asymmetric() {
        let z = z();
        assert_eq!(z.symmetry, 0.0);
        assert!(z.is_asymmetric());
        assert!(!z.is_symmetric());
    }

    #[test]
    fn new_clamps_max_symmetry() {
        let z = Zygomorphic::new(-5.0, 1.5);
        assert!((z.max_symmetry - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_align_rate() {
        let z = Zygomorphic::new(100.0, -1.5);
        assert_eq!(z.align_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zygomorphic::default();
        assert!((z.max_symmetry - 100.0).abs() < 1e-5);
        assert!((z.align_rate - 1.5).abs() < 1e-5);
    }

    // --- align ---

    #[test]
    fn align_adds_symmetry() {
        let mut z = z();
        z.align(40.0);
        assert!((z.symmetry - 40.0).abs() < 1e-3);
    }

    #[test]
    fn align_clamps_at_max() {
        let mut z = z();
        z.align(200.0);
        assert!((z.symmetry - 100.0).abs() < 1e-3);
    }

    #[test]
    fn align_fires_just_symmetric_at_max() {
        let mut z = z();
        z.align(100.0);
        assert!(z.just_symmetric);
        assert!(z.is_symmetric());
    }

    #[test]
    fn align_no_just_symmetric_when_already_at_max() {
        let mut z = z();
        z.symmetry = 100.0;
        z.align(10.0);
        assert!(!z.just_symmetric);
    }

    #[test]
    fn align_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.align(50.0);
        assert_eq!(z.symmetry, 0.0);
    }

    #[test]
    fn align_no_op_when_amount_zero() {
        let mut z = z();
        z.align(0.0);
        assert_eq!(z.symmetry, 0.0);
    }

    // --- distort ---

    #[test]
    fn distort_reduces_symmetry() {
        let mut z = z();
        z.symmetry = 60.0;
        z.distort(20.0);
        assert!((z.symmetry - 40.0).abs() < 1e-3);
    }

    #[test]
    fn distort_clamps_at_zero() {
        let mut z = z();
        z.symmetry = 30.0;
        z.distort(200.0);
        assert_eq!(z.symmetry, 0.0);
    }

    #[test]
    fn distort_fires_just_asymmetric_at_zero() {
        let mut z = z();
        z.symmetry = 30.0;
        z.distort(30.0);
        assert!(z.just_asymmetric);
    }

    #[test]
    fn distort_no_op_when_already_asymmetric() {
        let mut z = z();
        z.distort(10.0);
        assert!(!z.just_asymmetric);
    }

    #[test]
    fn distort_no_op_when_disabled() {
        let mut z = z();
        z.symmetry = 50.0;
        z.enabled = false;
        z.distort(50.0);
        assert!((z.symmetry - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_aligns_symmetry() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.symmetry - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_symmetric_on_align_to_max() {
        let mut z = Zygomorphic::new(100.0, 200.0);
        z.symmetry = 95.0;
        z.tick(1.0);
        assert!(z.just_symmetric);
        assert!(z.is_symmetric());
    }

    #[test]
    fn tick_no_align_when_already_symmetric() {
        let mut z = z();
        z.symmetry = 100.0;
        z.tick(1.0);
        assert!(!z.just_symmetric);
    }

    #[test]
    fn tick_no_align_when_rate_zero() {
        let mut z = Zygomorphic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.symmetry, 0.0);
    }

    #[test]
    fn tick_no_align_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.symmetry, 0.0);
    }

    #[test]
    fn tick_clears_just_symmetric() {
        let mut z = Zygomorphic::new(100.0, 200.0);
        z.symmetry = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_symmetric);
    }

    #[test]
    fn tick_clears_just_asymmetric() {
        let mut z = z();
        z.symmetry = 10.0;
        z.distort(10.0);
        z.tick(0.016);
        assert!(!z.just_asymmetric);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.symmetry - 9.0).abs() < 1e-3);
    }

    // --- is_symmetric / is_asymmetric ---

    #[test]
    fn is_symmetric_false_when_disabled() {
        let mut z = z();
        z.symmetry = 100.0;
        z.enabled = false;
        assert!(!z.is_symmetric());
    }

    #[test]
    fn is_asymmetric_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_asymmetric());
    }

    // --- symmetry_fraction / effective_bilateral ---

    #[test]
    fn symmetry_fraction_zero_when_asymmetric() {
        assert_eq!(z().symmetry_fraction(), 0.0);
    }

    #[test]
    fn symmetry_fraction_half_at_midpoint() {
        let mut z = z();
        z.symmetry = 50.0;
        assert!((z.symmetry_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_bilateral_zero_when_asymmetric() {
        assert_eq!(z().effective_bilateral(100.0), 0.0);
    }

    #[test]
    fn effective_bilateral_scales_with_symmetry() {
        let mut z = z();
        z.symmetry = 75.0;
        assert!((z.effective_bilateral(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_bilateral_zero_when_disabled() {
        let mut z = z();
        z.symmetry = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_bilateral(100.0), 0.0);
    }
}
