use bevy_ecs::prelude::Component;

/// Seasonal-renewal accumulation tracker named after vernal, the
/// adjective meaning of, relating to, or occurring in the spring —
/// from the Latin vernalis, derived from ver (spring), cognate with
/// the Greek ear and ultimately with the Proto-Indo-European root
/// wes-, which described the warm season when life resumed after
/// winter's suspension. Vernal carries its etymology into its usage:
/// it is the adjective of first things, of thawing ground and
/// lengthening days and the sudden urgency of organisms that have
/// been waiting, stored in seed or dormant bulb or hibernating body,
/// for exactly this moment to resume their interrupted programmes.
/// The vernal equinox marks the astronomical event when the sun
/// crosses the celestial equator moving northward, delivering equal
/// day and night to both hemispheres before the northern half tips
/// into its season of growth; every agricultural calendar in the
/// northern hemisphere has organised itself around this passage, and
/// the festivals that cluster around it — Nowruz, Ostara, Passover,
/// Easter — share a common grammar of emergence, resurrection,
/// and the reinstatement of possibility after a period of cold
/// foreclosure. The word is also pressed into service in natural
/// history: vernal pools are the ephemeral ponds that form from
/// snowmelt and spring rain in depressions that dry out by summer,
/// host to fairy shrimp and wood frog eggs and the particular
/// biodiversity of organisms that have evolved to exploit the brief
/// window between freeze and drought. In game mechanics, vernal
/// energy models the slow accumulation of renewal potential — the
/// gathering of warmth and light and moisture that eventually crosses
/// a threshold and triggers the burst of productive activity that
/// spring makes possible. `bloom` builds via `renew(amount)` and
/// accumulates passively at `thaw_rate` per second in `tick(dt)` or
/// is depleted via `wither(amount)`.
///
/// Models seasonal-renewal fill levels, spring-bloom saturation
/// bars, thaw-energy accumulators, growth-potential gauges, equinox-
/// approach fill levels, dormancy-breaking saturation indicators,
/// vernal-pool fill accumulation bars, resurrection-energy meters,
/// renewal-cycle fill levels, or any mechanic where a world, region,
/// character, or ecosystem slowly accumulates the energy of returning
/// life — seed by seed, degree by degree — until the threshold is
/// crossed and winter's long hold is broken, the fields flush green,
/// and every system that had been waiting in suspended patience
/// resumes the work of growth.
///
/// `renew(amount)` adds bloom; fires `just_bloomed` when first
/// reaching `max_bloom`. No-op when disabled.
///
/// `wither(amount)` reduces bloom immediately; fires `just_dormant`
/// when reaching 0. No-op when disabled or already dormant.
///
/// `tick(dt)` clears both flags, then increases bloom by
/// `thaw_rate * dt` (capped at `max_bloom`). Fires `just_bloomed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_bloomed()` returns `bloom >= max_bloom && enabled`.
///
/// `is_dormant()` returns `bloom == 0.0` (not gated by `enabled`).
///
/// `bloom_fraction()` returns `(bloom / max_bloom).clamp(0, 1)`.
///
/// `effective_renewal(scale)` returns `scale * bloom_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — thaws at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vernal {
    pub bloom: f32,
    pub max_bloom: f32,
    pub thaw_rate: f32,
    pub just_bloomed: bool,
    pub just_dormant: bool,
    pub enabled: bool,
}

impl Vernal {
    pub fn new(max_bloom: f32, thaw_rate: f32) -> Self {
        Self {
            bloom: 0.0,
            max_bloom: max_bloom.max(0.1),
            thaw_rate: thaw_rate.max(0.0),
            just_bloomed: false,
            just_dormant: false,
            enabled: true,
        }
    }

    /// Add bloom; fires `just_bloomed` when first reaching max.
    /// No-op when disabled.
    pub fn renew(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.bloom < self.max_bloom;
        self.bloom = (self.bloom + amount).min(self.max_bloom);
        if was_below && self.bloom >= self.max_bloom {
            self.just_bloomed = true;
        }
    }

    /// Reduce bloom; fires `just_dormant` when reaching 0.
    /// No-op when disabled or already dormant.
    pub fn wither(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.bloom <= 0.0 {
            return;
        }
        self.bloom = (self.bloom - amount).max(0.0);
        if self.bloom <= 0.0 {
            self.just_dormant = true;
        }
    }

    /// Clear flags, then increase bloom by `thaw_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_bloomed = false;
        self.just_dormant = false;
        if self.enabled && self.thaw_rate > 0.0 && self.bloom < self.max_bloom {
            let was_below = self.bloom < self.max_bloom;
            self.bloom = (self.bloom + self.thaw_rate * dt).min(self.max_bloom);
            if was_below && self.bloom >= self.max_bloom {
                self.just_bloomed = true;
            }
        }
    }

    /// `true` when bloom is at maximum and component is enabled.
    pub fn is_bloomed(&self) -> bool {
        self.bloom >= self.max_bloom && self.enabled
    }

    /// `true` when bloom is 0 (not gated by `enabled`).
    pub fn is_dormant(&self) -> bool {
        self.bloom == 0.0
    }

    /// Fraction of maximum bloom [0.0, 1.0].
    pub fn bloom_fraction(&self) -> f32 {
        (self.bloom / self.max_bloom).clamp(0.0, 1.0)
    }

    /// Returns `scale * bloom_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_renewal(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.bloom_fraction()
    }
}

impl Default for Vernal {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vernal {
        Vernal::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dormant() {
        let v = v();
        assert_eq!(v.bloom, 0.0);
        assert!(v.is_dormant());
        assert!(!v.is_bloomed());
    }

    #[test]
    fn new_clamps_max_bloom() {
        let v = Vernal::new(-5.0, 1.5);
        assert!((v.max_bloom - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_thaw_rate() {
        let v = Vernal::new(100.0, -1.5);
        assert_eq!(v.thaw_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Vernal::default();
        assert!((v.max_bloom - 100.0).abs() < 1e-5);
        assert!((v.thaw_rate - 1.5).abs() < 1e-5);
    }

    // --- renew ---

    #[test]
    fn renew_adds_bloom() {
        let mut v = v();
        v.renew(40.0);
        assert!((v.bloom - 40.0).abs() < 1e-3);
    }

    #[test]
    fn renew_clamps_at_max() {
        let mut v = v();
        v.renew(200.0);
        assert!((v.bloom - 100.0).abs() < 1e-3);
    }

    #[test]
    fn renew_fires_just_bloomed_at_max() {
        let mut v = v();
        v.renew(100.0);
        assert!(v.just_bloomed);
        assert!(v.is_bloomed());
    }

    #[test]
    fn renew_no_just_bloomed_when_already_at_max() {
        let mut v = v();
        v.bloom = 100.0;
        v.renew(10.0);
        assert!(!v.just_bloomed);
    }

    #[test]
    fn renew_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.renew(50.0);
        assert_eq!(v.bloom, 0.0);
    }

    #[test]
    fn renew_no_op_when_amount_zero() {
        let mut v = v();
        v.renew(0.0);
        assert_eq!(v.bloom, 0.0);
    }

    // --- wither ---

    #[test]
    fn wither_reduces_bloom() {
        let mut v = v();
        v.bloom = 60.0;
        v.wither(20.0);
        assert!((v.bloom - 40.0).abs() < 1e-3);
    }

    #[test]
    fn wither_clamps_at_zero() {
        let mut v = v();
        v.bloom = 30.0;
        v.wither(200.0);
        assert_eq!(v.bloom, 0.0);
    }

    #[test]
    fn wither_fires_just_dormant_at_zero() {
        let mut v = v();
        v.bloom = 30.0;
        v.wither(30.0);
        assert!(v.just_dormant);
    }

    #[test]
    fn wither_no_op_when_already_dormant() {
        let mut v = v();
        v.wither(10.0);
        assert!(!v.just_dormant);
    }

    #[test]
    fn wither_no_op_when_disabled() {
        let mut v = v();
        v.bloom = 50.0;
        v.enabled = false;
        v.wither(50.0);
        assert!((v.bloom - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_bloom() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.bloom - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_bloomed_on_bloom_to_max() {
        let mut v = Vernal::new(100.0, 200.0);
        v.bloom = 95.0;
        v.tick(1.0);
        assert!(v.just_bloomed);
        assert!(v.is_bloomed());
    }

    #[test]
    fn tick_no_build_when_already_bloomed() {
        let mut v = v();
        v.bloom = 100.0;
        v.tick(1.0);
        assert!(!v.just_bloomed);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Vernal::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.bloom, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.bloom, 0.0);
    }

    #[test]
    fn tick_clears_just_bloomed() {
        let mut v = Vernal::new(100.0, 200.0);
        v.bloom = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_bloomed);
    }

    #[test]
    fn tick_clears_just_dormant() {
        let mut v = v();
        v.bloom = 10.0;
        v.wither(10.0);
        v.tick(0.016);
        assert!(!v.just_dormant);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.bloom - 9.0).abs() < 1e-3);
    }

    // --- is_bloomed / is_dormant ---

    #[test]
    fn is_bloomed_false_when_disabled() {
        let mut v = v();
        v.bloom = 100.0;
        v.enabled = false;
        assert!(!v.is_bloomed());
    }

    #[test]
    fn is_dormant_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_dormant());
    }

    // --- bloom_fraction / effective_renewal ---

    #[test]
    fn bloom_fraction_zero_when_dormant() {
        assert_eq!(v().bloom_fraction(), 0.0);
    }

    #[test]
    fn bloom_fraction_half_at_midpoint() {
        let mut v = v();
        v.bloom = 50.0;
        assert!((v.bloom_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_renewal_zero_when_dormant() {
        assert_eq!(v().effective_renewal(100.0), 0.0);
    }

    #[test]
    fn effective_renewal_scales_with_bloom() {
        let mut v = v();
        v.bloom = 75.0;
        assert!((v.effective_renewal(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_renewal_zero_when_disabled() {
        let mut v = v();
        v.bloom = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_renewal(100.0), 0.0);
    }
}
