use bevy_ecs::prelude::Component;

/// Zone-distribution accumulation tracker named after zonation, the
/// arrangement or distribution of organisms, rock types, or other
/// phenomena in distinct bands or zones according to varying environmental
/// conditions. Intertidal zonation is the paradigmatic example: on any
/// rocky shore exposed to tidal rhythms, the splash zone above the tide
/// line, the high intertidal, mid intertidal, and low intertidal each
/// host a characteristic community of barnacles, limpets, mussels,
/// anemones, and kelp, each band beginning and ending at the elevations
/// that define its tolerance for desiccation, wave exposure, temperature,
/// and salinity. The pattern repeats on land as altitudinal zonation on
/// mountainsides — oak forest giving way to conifer forest, then krumm-
/// holz scrub, then alpine meadow, then bare rock and permanent snow —
/// and in the ocean as depth zonation, where the photic zone, twilight
/// zone, midnight zone, and abyssal zone each harbour organisms adapted
/// to the available light and crushing pressure. In geology the same
/// word describes the lateral and vertical variation of ore-mineral
/// assemblages within a deposit, or the metamorphic facies arrayed around
/// a plutonic intrusion. `distribution` builds via `stratify(amount)`
/// and accumulates passively at `layer_rate` per second in `tick(dt)`
/// or disperses via `collapse(amount)`.
///
/// Models zone-distribution fill levels, intertidal-band saturation
/// bars, altitudinal-stratum coverage trackers, ecological-belt
/// establishment gauges, metamorphic-zonation fill levels, ore-deposit
/// zonal saturation indicators, territorial-stratum accumulation bars,
/// biome-layer progression meters, depth-zone establishment fill levels,
/// or any mechanic where a system slowly differentiates into clearly
/// banded strata — each zone carved out by the interplay of tolerance
/// limits and environmental gradients — until the full stack of
/// stratified layers is established.
///
/// `stratify(amount)` adds distribution; fires `just_stratified` when
/// first reaching `max_distribution`. No-op when disabled.
///
/// `collapse(amount)` reduces distribution immediately; fires
/// `just_collapsed` when reaching 0. No-op when disabled or already
/// collapsed.
///
/// `tick(dt)` clears both flags, then increases distribution by
/// `layer_rate * dt` (capped at `max_distribution`). Fires
/// `just_stratified` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_stratified()` returns `distribution >= max_distribution && enabled`.
///
/// `is_collapsed()` returns `distribution == 0.0` (not gated by
/// `enabled`).
///
/// `distribution_fraction()` returns
/// `(distribution / max_distribution).clamp(0, 1)`.
///
/// `effective_stratum(scale)` returns `scale * distribution_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — layers at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zonation {
    pub distribution: f32,
    pub max_distribution: f32,
    pub layer_rate: f32,
    pub just_stratified: bool,
    pub just_collapsed: bool,
    pub enabled: bool,
}

impl Zonation {
    pub fn new(max_distribution: f32, layer_rate: f32) -> Self {
        Self {
            distribution: 0.0,
            max_distribution: max_distribution.max(0.1),
            layer_rate: layer_rate.max(0.0),
            just_stratified: false,
            just_collapsed: false,
            enabled: true,
        }
    }

    /// Add distribution; fires `just_stratified` when first reaching max.
    /// No-op when disabled.
    pub fn stratify(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.distribution < self.max_distribution;
        self.distribution = (self.distribution + amount).min(self.max_distribution);
        if was_below && self.distribution >= self.max_distribution {
            self.just_stratified = true;
        }
    }

    /// Reduce distribution; fires `just_collapsed` when reaching 0.
    /// No-op when disabled or already collapsed.
    pub fn collapse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.distribution <= 0.0 {
            return;
        }
        self.distribution = (self.distribution - amount).max(0.0);
        if self.distribution <= 0.0 {
            self.just_collapsed = true;
        }
    }

    /// Clear flags, then increase distribution by `layer_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_stratified = false;
        self.just_collapsed = false;
        if self.enabled && self.layer_rate > 0.0 && self.distribution < self.max_distribution {
            let was_below = self.distribution < self.max_distribution;
            self.distribution =
                (self.distribution + self.layer_rate * dt).min(self.max_distribution);
            if was_below && self.distribution >= self.max_distribution {
                self.just_stratified = true;
            }
        }
    }

    /// `true` when distribution is at maximum and component is enabled.
    pub fn is_stratified(&self) -> bool {
        self.distribution >= self.max_distribution && self.enabled
    }

    /// `true` when distribution is 0 (not gated by `enabled`).
    pub fn is_collapsed(&self) -> bool {
        self.distribution == 0.0
    }

    /// Fraction of maximum distribution [0.0, 1.0].
    pub fn distribution_fraction(&self) -> f32 {
        (self.distribution / self.max_distribution).clamp(0.0, 1.0)
    }

    /// Returns `scale * distribution_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_stratum(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.distribution_fraction()
    }
}

impl Default for Zonation {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zonation {
        Zonation::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_collapsed() {
        let z = z();
        assert_eq!(z.distribution, 0.0);
        assert!(z.is_collapsed());
        assert!(!z.is_stratified());
    }

    #[test]
    fn new_clamps_max_distribution() {
        let z = Zonation::new(-5.0, 1.5);
        assert!((z.max_distribution - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_layer_rate() {
        let z = Zonation::new(100.0, -1.5);
        assert_eq!(z.layer_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zonation::default();
        assert!((z.max_distribution - 100.0).abs() < 1e-5);
        assert!((z.layer_rate - 1.5).abs() < 1e-5);
    }

    // --- stratify ---

    #[test]
    fn stratify_adds_distribution() {
        let mut z = z();
        z.stratify(40.0);
        assert!((z.distribution - 40.0).abs() < 1e-3);
    }

    #[test]
    fn stratify_clamps_at_max() {
        let mut z = z();
        z.stratify(200.0);
        assert!((z.distribution - 100.0).abs() < 1e-3);
    }

    #[test]
    fn stratify_fires_just_stratified_at_max() {
        let mut z = z();
        z.stratify(100.0);
        assert!(z.just_stratified);
        assert!(z.is_stratified());
    }

    #[test]
    fn stratify_no_just_stratified_when_already_at_max() {
        let mut z = z();
        z.distribution = 100.0;
        z.stratify(10.0);
        assert!(!z.just_stratified);
    }

    #[test]
    fn stratify_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.stratify(50.0);
        assert_eq!(z.distribution, 0.0);
    }

    #[test]
    fn stratify_no_op_when_amount_zero() {
        let mut z = z();
        z.stratify(0.0);
        assert_eq!(z.distribution, 0.0);
    }

    // --- collapse ---

    #[test]
    fn collapse_reduces_distribution() {
        let mut z = z();
        z.distribution = 60.0;
        z.collapse(20.0);
        assert!((z.distribution - 40.0).abs() < 1e-3);
    }

    #[test]
    fn collapse_clamps_at_zero() {
        let mut z = z();
        z.distribution = 30.0;
        z.collapse(200.0);
        assert_eq!(z.distribution, 0.0);
    }

    #[test]
    fn collapse_fires_just_collapsed_at_zero() {
        let mut z = z();
        z.distribution = 30.0;
        z.collapse(30.0);
        assert!(z.just_collapsed);
    }

    #[test]
    fn collapse_no_op_when_already_collapsed() {
        let mut z = z();
        z.collapse(10.0);
        assert!(!z.just_collapsed);
    }

    #[test]
    fn collapse_no_op_when_disabled() {
        let mut z = z();
        z.distribution = 50.0;
        z.enabled = false;
        z.collapse(50.0);
        assert!((z.distribution - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_layers_distribution() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.distribution - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_stratified_on_layer_to_max() {
        let mut z = Zonation::new(100.0, 200.0);
        z.distribution = 95.0;
        z.tick(1.0);
        assert!(z.just_stratified);
        assert!(z.is_stratified());
    }

    #[test]
    fn tick_no_layer_when_already_stratified() {
        let mut z = z();
        z.distribution = 100.0;
        z.tick(1.0);
        assert!(!z.just_stratified);
    }

    #[test]
    fn tick_no_layer_when_rate_zero() {
        let mut z = Zonation::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.distribution, 0.0);
    }

    #[test]
    fn tick_no_layer_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.distribution, 0.0);
    }

    #[test]
    fn tick_clears_just_stratified() {
        let mut z = Zonation::new(100.0, 200.0);
        z.distribution = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_stratified);
    }

    #[test]
    fn tick_clears_just_collapsed() {
        let mut z = z();
        z.distribution = 10.0;
        z.collapse(10.0);
        z.tick(0.016);
        assert!(!z.just_collapsed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.distribution - 9.0).abs() < 1e-3);
    }

    // --- is_stratified / is_collapsed ---

    #[test]
    fn is_stratified_false_when_disabled() {
        let mut z = z();
        z.distribution = 100.0;
        z.enabled = false;
        assert!(!z.is_stratified());
    }

    #[test]
    fn is_collapsed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_collapsed());
    }

    // --- distribution_fraction / effective_stratum ---

    #[test]
    fn distribution_fraction_zero_when_collapsed() {
        assert_eq!(z().distribution_fraction(), 0.0);
    }

    #[test]
    fn distribution_fraction_half_at_midpoint() {
        let mut z = z();
        z.distribution = 50.0;
        assert!((z.distribution_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_stratum_zero_when_collapsed() {
        assert_eq!(z().effective_stratum(100.0), 0.0);
    }

    #[test]
    fn effective_stratum_scales_with_distribution() {
        let mut z = z();
        z.distribution = 75.0;
        assert!((z.effective_stratum(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_stratum_zero_when_disabled() {
        let mut z = z();
        z.distribution = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_stratum(100.0), 0.0);
    }
}
