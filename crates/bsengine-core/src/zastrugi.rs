use bevy_ecs::prelude::Component;

/// Wind-carved snow-ridge accumulation tracker. `drift` builds via
/// `accumulate(amount)` and compacts passively at `scour_rate` per second
/// in `tick(dt)` or is eroded immediately via `erode(amount)`.
///
/// Models wind-carved parallel snowfield-ridge fill levels, glacial-
/// ice-pavement surface-hardness gauges, blizzard-scouring depth
/// accumulators, tundra-sastrugi formation intensity trackers,
/// permafrost-surface roughness bars, Antarctic katabatic-wind
/// erosion indicators, ski-run surface-rutting saturation meters,
/// airfield-snowfield surface-condition fill levels, or any mechanic
/// where relentless polar winds carve a flat snowfield into a
/// parallel series of knife-edged ridges and hollows that harden
/// progressively until a sustained warm spell softens and re-levels
/// the entire surface back to featureless white.
///
/// `accumulate(amount)` adds drift; fires `just_hardened` when first
/// reaching `max_drift`. No-op when disabled.
///
/// `erode(amount)` reduces drift immediately; fires `just_scoured`
/// when reaching 0. No-op when disabled or already scoured.
///
/// `tick(dt)` clears both flags, then increases drift by
/// `scour_rate * dt` (capped at `max_drift`). Fires `just_hardened`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_hardened()` returns `drift >= max_drift && enabled`.
///
/// `is_scoured()` returns `drift == 0.0` (not gated by `enabled`).
///
/// `drift_fraction()` returns `(drift / max_drift).clamp(0, 1)`.
///
/// `effective_firmness(scale)` returns `scale * drift_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — compacts at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zastrugi {
    pub drift: f32,
    pub max_drift: f32,
    pub scour_rate: f32,
    pub just_hardened: bool,
    pub just_scoured: bool,
    pub enabled: bool,
}

impl Zastrugi {
    pub fn new(max_drift: f32, scour_rate: f32) -> Self {
        Self {
            drift: 0.0,
            max_drift: max_drift.max(0.1),
            scour_rate: scour_rate.max(0.0),
            just_hardened: false,
            just_scoured: false,
            enabled: true,
        }
    }

    /// Add drift; fires `just_hardened` when first reaching max.
    /// No-op when disabled.
    pub fn accumulate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.drift < self.max_drift;
        self.drift = (self.drift + amount).min(self.max_drift);
        if was_below && self.drift >= self.max_drift {
            self.just_hardened = true;
        }
    }

    /// Reduce drift; fires `just_scoured` when reaching 0.
    /// No-op when disabled or already scoured.
    pub fn erode(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.drift <= 0.0 {
            return;
        }
        self.drift = (self.drift - amount).max(0.0);
        if self.drift <= 0.0 {
            self.just_scoured = true;
        }
    }

    /// Clear flags, then increase drift by `scour_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_hardened = false;
        self.just_scoured = false;
        if self.enabled && self.scour_rate > 0.0 && self.drift < self.max_drift {
            let was_below = self.drift < self.max_drift;
            self.drift = (self.drift + self.scour_rate * dt).min(self.max_drift);
            if was_below && self.drift >= self.max_drift {
                self.just_hardened = true;
            }
        }
    }

    /// `true` when drift is at maximum and component is enabled.
    pub fn is_hardened(&self) -> bool {
        self.drift >= self.max_drift && self.enabled
    }

    /// `true` when drift is 0 (not gated by `enabled`).
    pub fn is_scoured(&self) -> bool {
        self.drift == 0.0
    }

    /// Fraction of maximum drift [0.0, 1.0].
    pub fn drift_fraction(&self) -> f32 {
        (self.drift / self.max_drift).clamp(0.0, 1.0)
    }

    /// Returns `scale * drift_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_firmness(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.drift_fraction()
    }
}

impl Default for Zastrugi {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zastrugi {
        Zastrugi::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_scoured() {
        let z = z();
        assert_eq!(z.drift, 0.0);
        assert!(z.is_scoured());
        assert!(!z.is_hardened());
    }

    #[test]
    fn new_clamps_max_drift() {
        let z = Zastrugi::new(-5.0, 1.0);
        assert!((z.max_drift - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_scour_rate() {
        let z = Zastrugi::new(100.0, -1.0);
        assert_eq!(z.scour_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zastrugi::default();
        assert!((z.max_drift - 100.0).abs() < 1e-5);
        assert!((z.scour_rate - 1.0).abs() < 1e-5);
    }

    // --- accumulate ---

    #[test]
    fn accumulate_adds_drift() {
        let mut z = z();
        z.accumulate(40.0);
        assert!((z.drift - 40.0).abs() < 1e-3);
    }

    #[test]
    fn accumulate_clamps_at_max() {
        let mut z = z();
        z.accumulate(200.0);
        assert!((z.drift - 100.0).abs() < 1e-3);
    }

    #[test]
    fn accumulate_fires_just_hardened_at_max() {
        let mut z = z();
        z.accumulate(100.0);
        assert!(z.just_hardened);
        assert!(z.is_hardened());
    }

    #[test]
    fn accumulate_no_just_hardened_when_already_at_max() {
        let mut z = z();
        z.drift = 100.0;
        z.accumulate(10.0);
        assert!(!z.just_hardened);
    }

    #[test]
    fn accumulate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.accumulate(50.0);
        assert_eq!(z.drift, 0.0);
    }

    #[test]
    fn accumulate_no_op_when_amount_zero() {
        let mut z = z();
        z.accumulate(0.0);
        assert_eq!(z.drift, 0.0);
    }

    // --- erode ---

    #[test]
    fn erode_reduces_drift() {
        let mut z = z();
        z.drift = 60.0;
        z.erode(20.0);
        assert!((z.drift - 40.0).abs() < 1e-3);
    }

    #[test]
    fn erode_clamps_at_zero() {
        let mut z = z();
        z.drift = 30.0;
        z.erode(200.0);
        assert_eq!(z.drift, 0.0);
    }

    #[test]
    fn erode_fires_just_scoured_at_zero() {
        let mut z = z();
        z.drift = 30.0;
        z.erode(30.0);
        assert!(z.just_scoured);
    }

    #[test]
    fn erode_no_op_when_already_scoured() {
        let mut z = z();
        z.erode(10.0);
        assert!(!z.just_scoured);
    }

    #[test]
    fn erode_no_op_when_disabled() {
        let mut z = z();
        z.drift = 50.0;
        z.enabled = false;
        z.erode(50.0);
        assert!((z.drift - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_compacts_drift() {
        let mut z = z(); // rate=1
        z.tick(5.0); // 0 + 1*5 = 5
        assert!((z.drift - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_hardened_on_compact_to_max() {
        let mut z = Zastrugi::new(100.0, 200.0);
        z.drift = 95.0;
        z.tick(1.0);
        assert!(z.just_hardened);
        assert!(z.is_hardened());
    }

    #[test]
    fn tick_no_compact_when_already_hardened() {
        let mut z = z();
        z.drift = 100.0;
        z.tick(1.0);
        assert!(!z.just_hardened);
    }

    #[test]
    fn tick_no_compact_when_rate_zero() {
        let mut z = Zastrugi::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.drift, 0.0);
    }

    #[test]
    fn tick_no_compact_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.drift, 0.0);
    }

    #[test]
    fn tick_clears_just_hardened() {
        let mut z = Zastrugi::new(100.0, 200.0);
        z.drift = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_hardened);
    }

    #[test]
    fn tick_clears_just_scoured() {
        let mut z = z();
        z.drift = 10.0;
        z.erode(10.0);
        z.tick(0.016);
        assert!(!z.just_scoured);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(8.0); // 1*8 = 8
        assert!((z.drift - 8.0).abs() < 1e-3);
    }

    // --- is_hardened / is_scoured ---

    #[test]
    fn is_hardened_false_when_disabled() {
        let mut z = z();
        z.drift = 100.0;
        z.enabled = false;
        assert!(!z.is_hardened());
    }

    #[test]
    fn is_scoured_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_scoured());
    }

    // --- drift_fraction / effective_firmness ---

    #[test]
    fn drift_fraction_zero_when_scoured() {
        assert_eq!(z().drift_fraction(), 0.0);
    }

    #[test]
    fn drift_fraction_half_at_midpoint() {
        let mut z = z();
        z.drift = 50.0;
        assert!((z.drift_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_firmness_zero_when_scoured() {
        assert_eq!(z().effective_firmness(100.0), 0.0);
    }

    #[test]
    fn effective_firmness_scales_with_drift() {
        let mut z = z();
        z.drift = 75.0;
        assert!((z.effective_firmness(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_firmness_zero_when_disabled() {
        let mut z = z();
        z.drift = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_firmness(100.0), 0.0);
    }
}
