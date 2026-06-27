use bevy_ecs::prelude::Component;

/// Hybrid-trait blending tracker. `hybridization` builds via
/// `crossbreed(amount)` and intensifies passively at `blend_rate`
/// per second in `tick(dt)` or is diluted immediately via
/// `dilute(amount)`.
///
/// Models interspecies hybrid-vigor fill levels, crossbreed gene-
/// expression intensity gauges, wild-domestic trait-blending
/// saturation bars, equine-hybrid stamina accumulation trackers,
/// zorse-temperament-balance meters, hybrid-fertility window
/// indicators, genetic-chimera integration fill levels, selective-
/// breeding saturation trackers, or any mechanic where two
/// genetically distinct lineages blend their traits in increasing
/// proportion until the offspring embodies peak hybrid vigour —
/// right up until back-crossing dilutes the hybrid character back
/// toward either pure parental line.
///
/// `crossbreed(amount)` adds hybridization; fires `just_hybrid`
/// when first reaching `max_hybridization`. No-op when disabled.
///
/// `dilute(amount)` reduces hybridization immediately; fires
/// `just_pure` when reaching 0. No-op when disabled or already pure.
///
/// `tick(dt)` clears both flags, then increases hybridization by
/// `blend_rate * dt` (capped at `max_hybridization`). Fires
/// `just_hybrid` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_hybrid()` returns `hybridization >= max_hybridization && enabled`.
///
/// `is_pure()` returns `hybridization == 0.0` (not gated by `enabled`).
///
/// `hybridization_fraction()` returns `(hybridization / max_hybridization).clamp(0, 1)`.
///
/// `effective_vigor(scale)` returns `scale * hybridization_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — blends at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zebroid {
    pub hybridization: f32,
    pub max_hybridization: f32,
    pub blend_rate: f32,
    pub just_hybrid: bool,
    pub just_pure: bool,
    pub enabled: bool,
}

impl Zebroid {
    pub fn new(max_hybridization: f32, blend_rate: f32) -> Self {
        Self {
            hybridization: 0.0,
            max_hybridization: max_hybridization.max(0.1),
            blend_rate: blend_rate.max(0.0),
            just_hybrid: false,
            just_pure: false,
            enabled: true,
        }
    }

    /// Add hybridization; fires `just_hybrid` when first reaching max.
    /// No-op when disabled.
    pub fn crossbreed(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.hybridization < self.max_hybridization;
        self.hybridization = (self.hybridization + amount).min(self.max_hybridization);
        if was_below && self.hybridization >= self.max_hybridization {
            self.just_hybrid = true;
        }
    }

    /// Reduce hybridization; fires `just_pure` when reaching 0.
    /// No-op when disabled or already pure.
    pub fn dilute(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.hybridization <= 0.0 {
            return;
        }
        self.hybridization = (self.hybridization - amount).max(0.0);
        if self.hybridization <= 0.0 {
            self.just_pure = true;
        }
    }

    /// Clear flags, then increase hybridization by `blend_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_hybrid = false;
        self.just_pure = false;
        if self.enabled && self.blend_rate > 0.0 && self.hybridization < self.max_hybridization {
            let was_below = self.hybridization < self.max_hybridization;
            self.hybridization =
                (self.hybridization + self.blend_rate * dt).min(self.max_hybridization);
            if was_below && self.hybridization >= self.max_hybridization {
                self.just_hybrid = true;
            }
        }
    }

    /// `true` when hybridization is at maximum and component is enabled.
    pub fn is_hybrid(&self) -> bool {
        self.hybridization >= self.max_hybridization && self.enabled
    }

    /// `true` when hybridization is 0 (not gated by `enabled`).
    pub fn is_pure(&self) -> bool {
        self.hybridization == 0.0
    }

    /// Fraction of maximum hybridization [0.0, 1.0].
    pub fn hybridization_fraction(&self) -> f32 {
        (self.hybridization / self.max_hybridization).clamp(0.0, 1.0)
    }

    /// Returns `scale * hybridization_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vigor(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.hybridization_fraction()
    }
}

impl Default for Zebroid {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zebroid {
        Zebroid::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_pure() {
        let z = z();
        assert_eq!(z.hybridization, 0.0);
        assert!(z.is_pure());
        assert!(!z.is_hybrid());
    }

    #[test]
    fn new_clamps_max_hybridization() {
        let z = Zebroid::new(-5.0, 1.5);
        assert!((z.max_hybridization - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_blend_rate() {
        let z = Zebroid::new(100.0, -1.5);
        assert_eq!(z.blend_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zebroid::default();
        assert!((z.max_hybridization - 100.0).abs() < 1e-5);
        assert!((z.blend_rate - 1.5).abs() < 1e-5);
    }

    // --- crossbreed ---

    #[test]
    fn crossbreed_adds_hybridization() {
        let mut z = z();
        z.crossbreed(40.0);
        assert!((z.hybridization - 40.0).abs() < 1e-3);
    }

    #[test]
    fn crossbreed_clamps_at_max() {
        let mut z = z();
        z.crossbreed(200.0);
        assert!((z.hybridization - 100.0).abs() < 1e-3);
    }

    #[test]
    fn crossbreed_fires_just_hybrid_at_max() {
        let mut z = z();
        z.crossbreed(100.0);
        assert!(z.just_hybrid);
        assert!(z.is_hybrid());
    }

    #[test]
    fn crossbreed_no_just_hybrid_when_already_at_max() {
        let mut z = z();
        z.hybridization = 100.0;
        z.crossbreed(10.0);
        assert!(!z.just_hybrid);
    }

    #[test]
    fn crossbreed_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.crossbreed(50.0);
        assert_eq!(z.hybridization, 0.0);
    }

    #[test]
    fn crossbreed_no_op_when_amount_zero() {
        let mut z = z();
        z.crossbreed(0.0);
        assert_eq!(z.hybridization, 0.0);
    }

    // --- dilute ---

    #[test]
    fn dilute_reduces_hybridization() {
        let mut z = z();
        z.hybridization = 60.0;
        z.dilute(20.0);
        assert!((z.hybridization - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dilute_clamps_at_zero() {
        let mut z = z();
        z.hybridization = 30.0;
        z.dilute(200.0);
        assert_eq!(z.hybridization, 0.0);
    }

    #[test]
    fn dilute_fires_just_pure_at_zero() {
        let mut z = z();
        z.hybridization = 30.0;
        z.dilute(30.0);
        assert!(z.just_pure);
    }

    #[test]
    fn dilute_no_op_when_already_pure() {
        let mut z = z();
        z.dilute(10.0);
        assert!(!z.just_pure);
    }

    #[test]
    fn dilute_no_op_when_disabled() {
        let mut z = z();
        z.hybridization = 50.0;
        z.enabled = false;
        z.dilute(50.0);
        assert!((z.hybridization - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_blends_hybridization() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.hybridization - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_hybrid_on_blend_to_max() {
        let mut z = Zebroid::new(100.0, 200.0);
        z.hybridization = 95.0;
        z.tick(1.0);
        assert!(z.just_hybrid);
        assert!(z.is_hybrid());
    }

    #[test]
    fn tick_no_blend_when_already_hybrid() {
        let mut z = z();
        z.hybridization = 100.0;
        z.tick(1.0);
        assert!(!z.just_hybrid);
    }

    #[test]
    fn tick_no_blend_when_rate_zero() {
        let mut z = Zebroid::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.hybridization, 0.0);
    }

    #[test]
    fn tick_no_blend_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.hybridization, 0.0);
    }

    #[test]
    fn tick_clears_just_hybrid() {
        let mut z = Zebroid::new(100.0, 200.0);
        z.hybridization = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_hybrid);
    }

    #[test]
    fn tick_clears_just_pure() {
        let mut z = z();
        z.hybridization = 10.0;
        z.dilute(10.0);
        z.tick(0.016);
        assert!(!z.just_pure);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.hybridization - 9.0).abs() < 1e-3);
    }

    // --- is_hybrid / is_pure ---

    #[test]
    fn is_hybrid_false_when_disabled() {
        let mut z = z();
        z.hybridization = 100.0;
        z.enabled = false;
        assert!(!z.is_hybrid());
    }

    #[test]
    fn is_pure_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_pure());
    }

    // --- hybridization_fraction / effective_vigor ---

    #[test]
    fn hybridization_fraction_zero_when_pure() {
        assert_eq!(z().hybridization_fraction(), 0.0);
    }

    #[test]
    fn hybridization_fraction_half_at_midpoint() {
        let mut z = z();
        z.hybridization = 50.0;
        assert!((z.hybridization_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_vigor_zero_when_pure() {
        assert_eq!(z().effective_vigor(100.0), 0.0);
    }

    #[test]
    fn effective_vigor_scales_with_hybridization() {
        let mut z = z();
        z.hybridization = 75.0;
        assert!((z.effective_vigor(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vigor_zero_when_disabled() {
        let mut z = z();
        z.hybridization = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_vigor(100.0), 0.0);
    }
}
