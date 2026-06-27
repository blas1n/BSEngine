use bevy_ecs::prelude::Component;

/// Shape-shift intensity tracker. `morph` builds via `shift(amount)` and
/// transforms passively at `flux_rate` per second in `tick(dt)` or
/// reverts immediately via `revert(amount)`.
///
/// Models animal-form transformation meters, shapeshifter-energy
/// bars, druidic-wildshape charge trackers, beast-form saturation
/// fill levels, chimeric-mutation intensity gauges, werewolf-shift
/// power accumulators, selkie-seal-form build-up indicators,
/// protean-creature adaptation progress bars, or any mechanic where
/// a creature slowly saturates its morphic field until every bone
/// reshapes into a new and terrible silhouette — only to revert
/// the moment the will falters.
///
/// `shift(amount)` adds morph; fires `just_shifted` when first
/// reaching `max_morph`. No-op when disabled.
///
/// `revert(amount)` reduces morph immediately; fires `just_reverted`
/// when reaching 0. No-op when disabled or already reverted.
///
/// `tick(dt)` clears both flags, then increases morph by
/// `flux_rate * dt` (capped at `max_morph`). Fires `just_shifted`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_shifted()` returns `morph >= max_morph && enabled`.
///
/// `is_reverted()` returns `morph == 0.0` (not gated by `enabled`).
///
/// `morph_fraction()` returns `(morph / max_morph).clamp(0, 1)`.
///
/// `effective_flux(scale)` returns `scale * morph_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — transforms at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoomorph {
    pub morph: f32,
    pub max_morph: f32,
    pub flux_rate: f32,
    pub just_shifted: bool,
    pub just_reverted: bool,
    pub enabled: bool,
}

impl Zoomorph {
    pub fn new(max_morph: f32, flux_rate: f32) -> Self {
        Self {
            morph: 0.0,
            max_morph: max_morph.max(0.1),
            flux_rate: flux_rate.max(0.0),
            just_shifted: false,
            just_reverted: false,
            enabled: true,
        }
    }

    /// Add morph; fires `just_shifted` when first reaching max.
    /// No-op when disabled.
    pub fn shift(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.morph < self.max_morph;
        self.morph = (self.morph + amount).min(self.max_morph);
        if was_below && self.morph >= self.max_morph {
            self.just_shifted = true;
        }
    }

    /// Reduce morph; fires `just_reverted` when reaching 0.
    /// No-op when disabled or already reverted.
    pub fn revert(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.morph <= 0.0 {
            return;
        }
        self.morph = (self.morph - amount).max(0.0);
        if self.morph <= 0.0 {
            self.just_reverted = true;
        }
    }

    /// Clear flags, then increase morph by `flux_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_shifted = false;
        self.just_reverted = false;
        if self.enabled && self.flux_rate > 0.0 && self.morph < self.max_morph {
            let was_below = self.morph < self.max_morph;
            self.morph = (self.morph + self.flux_rate * dt).min(self.max_morph);
            if was_below && self.morph >= self.max_morph {
                self.just_shifted = true;
            }
        }
    }

    /// `true` when morph is at maximum and component is enabled.
    pub fn is_shifted(&self) -> bool {
        self.morph >= self.max_morph && self.enabled
    }

    /// `true` when morph is 0 (not gated by `enabled`).
    pub fn is_reverted(&self) -> bool {
        self.morph == 0.0
    }

    /// Fraction of maximum morph [0.0, 1.0].
    pub fn morph_fraction(&self) -> f32 {
        (self.morph / self.max_morph).clamp(0.0, 1.0)
    }

    /// Returns `scale * morph_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_flux(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.morph_fraction()
    }
}

impl Default for Zoomorph {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoomorph {
        Zoomorph::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_reverted() {
        let z = z();
        assert_eq!(z.morph, 0.0);
        assert!(z.is_reverted());
        assert!(!z.is_shifted());
    }

    #[test]
    fn new_clamps_max_morph() {
        let z = Zoomorph::new(-5.0, 4.0);
        assert!((z.max_morph - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_flux_rate() {
        let z = Zoomorph::new(100.0, -3.0);
        assert_eq!(z.flux_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoomorph::default();
        assert!((z.max_morph - 100.0).abs() < 1e-5);
        assert!((z.flux_rate - 4.0).abs() < 1e-5);
    }

    // --- shift ---

    #[test]
    fn shift_adds_morph() {
        let mut z = z();
        z.shift(40.0);
        assert!((z.morph - 40.0).abs() < 1e-3);
    }

    #[test]
    fn shift_clamps_at_max() {
        let mut z = z();
        z.shift(200.0);
        assert!((z.morph - 100.0).abs() < 1e-3);
    }

    #[test]
    fn shift_fires_just_shifted_at_max() {
        let mut z = z();
        z.shift(100.0);
        assert!(z.just_shifted);
        assert!(z.is_shifted());
    }

    #[test]
    fn shift_no_just_shifted_when_already_at_max() {
        let mut z = z();
        z.morph = 100.0;
        z.shift(10.0);
        assert!(!z.just_shifted);
    }

    #[test]
    fn shift_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.shift(50.0);
        assert_eq!(z.morph, 0.0);
    }

    #[test]
    fn shift_no_op_when_amount_zero() {
        let mut z = z();
        z.shift(0.0);
        assert_eq!(z.morph, 0.0);
    }

    // --- revert ---

    #[test]
    fn revert_reduces_morph() {
        let mut z = z();
        z.morph = 60.0;
        z.revert(20.0);
        assert!((z.morph - 40.0).abs() < 1e-3);
    }

    #[test]
    fn revert_clamps_at_zero() {
        let mut z = z();
        z.morph = 30.0;
        z.revert(200.0);
        assert_eq!(z.morph, 0.0);
    }

    #[test]
    fn revert_fires_just_reverted_at_zero() {
        let mut z = z();
        z.morph = 30.0;
        z.revert(30.0);
        assert!(z.just_reverted);
    }

    #[test]
    fn revert_no_op_when_already_reverted() {
        let mut z = z();
        z.revert(10.0);
        assert!(!z.just_reverted);
    }

    #[test]
    fn revert_no_op_when_disabled() {
        let mut z = z();
        z.morph = 50.0;
        z.enabled = false;
        z.revert(50.0);
        assert!((z.morph - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_transforms_morph() {
        let mut z = z(); // rate=4
        z.tick(2.0); // 0 + 4*2 = 8
        assert!((z.morph - 8.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_shifted_on_transform_to_max() {
        let mut z = Zoomorph::new(100.0, 200.0);
        z.morph = 95.0;
        z.tick(1.0);
        assert!(z.just_shifted);
        assert!(z.is_shifted());
    }

    #[test]
    fn tick_no_transform_when_already_shifted() {
        let mut z = z();
        z.morph = 100.0;
        z.tick(1.0);
        assert!(!z.just_shifted);
    }

    #[test]
    fn tick_no_transform_when_rate_zero() {
        let mut z = Zoomorph::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.morph, 0.0);
    }

    #[test]
    fn tick_no_transform_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.morph, 0.0);
    }

    #[test]
    fn tick_clears_just_shifted() {
        let mut z = Zoomorph::new(100.0, 200.0);
        z.morph = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_shifted);
    }

    #[test]
    fn tick_clears_just_reverted() {
        let mut z = z();
        z.morph = 10.0;
        z.revert(10.0);
        z.tick(0.016);
        assert!(!z.just_reverted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=4
        z.tick(3.0); // 4*3 = 12
        assert!((z.morph - 12.0).abs() < 1e-3);
    }

    // --- is_shifted / is_reverted ---

    #[test]
    fn is_shifted_false_when_disabled() {
        let mut z = z();
        z.morph = 100.0;
        z.enabled = false;
        assert!(!z.is_shifted());
    }

    #[test]
    fn is_reverted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_reverted());
    }

    // --- morph_fraction / effective_flux ---

    #[test]
    fn morph_fraction_zero_when_reverted() {
        assert_eq!(z().morph_fraction(), 0.0);
    }

    #[test]
    fn morph_fraction_half_at_midpoint() {
        let mut z = z();
        z.morph = 50.0;
        assert!((z.morph_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_flux_zero_when_reverted() {
        assert_eq!(z().effective_flux(100.0), 0.0);
    }

    #[test]
    fn effective_flux_scales_with_morph() {
        let mut z = z();
        z.morph = 75.0;
        assert!((z.effective_flux(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_flux_zero_when_disabled() {
        let mut z = z();
        z.morph = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_flux(100.0), 0.0);
    }
}
