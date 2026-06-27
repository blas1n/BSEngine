use bevy_ecs::prelude::Component;

/// Aquatic-bloom tracker. `drift` builds via `bloom(amount)` and
/// proliferates passively at `bloom_rate` per second in `tick(dt)` or
/// is consumed immediately via `consume(amount)`.
///
/// Models ocean-ecosystem plankton-bloom meters, aquatic-biome health
/// fill levels, marine-food-chain stability gauges, algae-proliferation
/// accumulators, underwater-nutrient saturation bars, phytoplankton
/// density trackers, lake-bloom intensity indicators, or any mechanic
/// where microscopic aquatic life builds to an ecosystem-altering bloom.
///
/// `bloom(amount)` adds drift; fires `just_bloomed` when first reaching
/// `max_drift`. No-op when disabled.
///
/// `consume(amount)` reduces drift immediately; fires `just_clear` when
/// reaching 0. No-op when disabled or already clear.
///
/// `tick(dt)` clears both flags, then increases drift by
/// `bloom_rate * dt` (capped at `max_drift`). Fires `just_bloomed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_bloomed()` returns `drift >= max_drift && enabled`.
///
/// `is_clear()` returns `drift == 0.0` (not gated by `enabled`).
///
/// `drift_fraction()` returns `(drift / max_drift).clamp(0, 1)`.
///
/// `effective_density(scale)` returns `scale * drift_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — proliferates at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooplankton {
    pub drift: f32,
    pub max_drift: f32,
    pub bloom_rate: f32,
    pub just_bloomed: bool,
    pub just_clear: bool,
    pub enabled: bool,
}

impl Zooplankton {
    pub fn new(max_drift: f32, bloom_rate: f32) -> Self {
        Self {
            drift: 0.0,
            max_drift: max_drift.max(0.1),
            bloom_rate: bloom_rate.max(0.0),
            just_bloomed: false,
            just_clear: false,
            enabled: true,
        }
    }

    /// Add drift; fires `just_bloomed` when first reaching max.
    /// No-op when disabled.
    pub fn bloom(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.drift < self.max_drift;
        self.drift = (self.drift + amount).min(self.max_drift);
        if was_below && self.drift >= self.max_drift {
            self.just_bloomed = true;
        }
    }

    /// Reduce drift; fires `just_clear` when reaching 0.
    /// No-op when disabled or already clear.
    pub fn consume(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.drift <= 0.0 {
            return;
        }
        self.drift = (self.drift - amount).max(0.0);
        if self.drift <= 0.0 {
            self.just_clear = true;
        }
    }

    /// Clear flags, then increase drift by `bloom_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_bloomed = false;
        self.just_clear = false;
        if self.enabled && self.bloom_rate > 0.0 && self.drift < self.max_drift {
            let was_below = self.drift < self.max_drift;
            self.drift = (self.drift + self.bloom_rate * dt).min(self.max_drift);
            if was_below && self.drift >= self.max_drift {
                self.just_bloomed = true;
            }
        }
    }

    /// `true` when drift is at maximum and component is enabled.
    pub fn is_bloomed(&self) -> bool {
        self.drift >= self.max_drift && self.enabled
    }

    /// `true` when drift is 0 (not gated by `enabled`).
    pub fn is_clear(&self) -> bool {
        self.drift == 0.0
    }

    /// Fraction of maximum drift [0.0, 1.0].
    pub fn drift_fraction(&self) -> f32 {
        (self.drift / self.max_drift).clamp(0.0, 1.0)
    }

    /// Returns `scale * drift_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_density(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.drift_fraction()
    }
}

impl Default for Zooplankton {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zooplankton {
        Zooplankton::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_clear() {
        let z = z();
        assert_eq!(z.drift, 0.0);
        assert!(z.is_clear());
        assert!(!z.is_bloomed());
    }

    #[test]
    fn new_clamps_max_drift() {
        let z = Zooplankton::new(-5.0, 1.0);
        assert!((z.max_drift - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_bloom_rate() {
        let z = Zooplankton::new(100.0, -3.0);
        assert_eq!(z.bloom_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zooplankton::default();
        assert!((z.max_drift - 100.0).abs() < 1e-5);
        assert!((z.bloom_rate - 1.0).abs() < 1e-5);
    }

    // --- bloom ---

    #[test]
    fn bloom_adds_drift() {
        let mut z = z();
        z.bloom(40.0);
        assert!((z.drift - 40.0).abs() < 1e-3);
    }

    #[test]
    fn bloom_clamps_at_max() {
        let mut z = z();
        z.bloom(200.0);
        assert!((z.drift - 100.0).abs() < 1e-3);
    }

    #[test]
    fn bloom_fires_just_bloomed_at_max() {
        let mut z = z();
        z.bloom(100.0);
        assert!(z.just_bloomed);
        assert!(z.is_bloomed());
    }

    #[test]
    fn bloom_no_just_bloomed_when_already_at_max() {
        let mut z = z();
        z.drift = 100.0;
        z.bloom(10.0);
        assert!(!z.just_bloomed);
    }

    #[test]
    fn bloom_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.bloom(50.0);
        assert_eq!(z.drift, 0.0);
    }

    #[test]
    fn bloom_no_op_when_amount_zero() {
        let mut z = z();
        z.bloom(0.0);
        assert_eq!(z.drift, 0.0);
    }

    // --- consume ---

    #[test]
    fn consume_reduces_drift() {
        let mut z = z();
        z.drift = 60.0;
        z.consume(20.0);
        assert!((z.drift - 40.0).abs() < 1e-3);
    }

    #[test]
    fn consume_clamps_at_zero() {
        let mut z = z();
        z.drift = 30.0;
        z.consume(200.0);
        assert_eq!(z.drift, 0.0);
    }

    #[test]
    fn consume_fires_just_clear_at_zero() {
        let mut z = z();
        z.drift = 30.0;
        z.consume(30.0);
        assert!(z.just_clear);
    }

    #[test]
    fn consume_no_op_when_already_clear() {
        let mut z = z();
        z.consume(10.0);
        assert!(!z.just_clear);
    }

    #[test]
    fn consume_no_op_when_disabled() {
        let mut z = z();
        z.drift = 50.0;
        z.enabled = false;
        z.consume(50.0);
        assert!((z.drift - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_proliferates_drift() {
        let mut z = z(); // rate=1
        z.tick(1.0); // 0 + 1 = 1
        assert!((z.drift - 1.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_bloomed_on_bloom_to_max() {
        let mut z = Zooplankton::new(100.0, 200.0);
        z.drift = 95.0;
        z.tick(1.0);
        assert!(z.just_bloomed);
        assert!(z.is_bloomed());
    }

    #[test]
    fn tick_no_bloom_when_already_bloomed() {
        let mut z = z();
        z.drift = 100.0;
        z.tick(1.0);
        assert!(!z.just_bloomed);
    }

    #[test]
    fn tick_no_bloom_when_rate_zero() {
        let mut z = Zooplankton::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.drift, 0.0);
    }

    #[test]
    fn tick_no_bloom_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.drift, 0.0);
    }

    #[test]
    fn tick_clears_just_bloomed() {
        let mut z = Zooplankton::new(100.0, 200.0);
        z.drift = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_bloomed);
    }

    #[test]
    fn tick_clears_just_clear() {
        let mut z = z();
        z.drift = 10.0;
        z.consume(10.0);
        z.tick(0.016);
        assert!(!z.just_clear);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(5.0); // 1*5 = 5
        assert!((z.drift - 5.0).abs() < 1e-3);
    }

    // --- is_bloomed / is_clear ---

    #[test]
    fn is_bloomed_false_when_disabled() {
        let mut z = z();
        z.drift = 100.0;
        z.enabled = false;
        assert!(!z.is_bloomed());
    }

    #[test]
    fn is_clear_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_clear());
    }

    // --- drift_fraction / effective_density ---

    #[test]
    fn drift_fraction_zero_when_clear() {
        assert_eq!(z().drift_fraction(), 0.0);
    }

    #[test]
    fn drift_fraction_half_at_midpoint() {
        let mut z = z();
        z.drift = 50.0;
        assert!((z.drift_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_density_zero_when_clear() {
        assert_eq!(z().effective_density(100.0), 0.0);
    }

    #[test]
    fn effective_density_scales_with_drift() {
        let mut z = z();
        z.drift = 60.0;
        assert!((z.effective_density(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_density_zero_when_disabled() {
        let mut z = z();
        z.drift = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_density(100.0), 0.0);
    }
}
