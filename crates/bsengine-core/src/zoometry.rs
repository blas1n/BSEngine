use bevy_ecs::prelude::Component;

/// Anatomical-measurement accumulation tracker named after zoometry,
/// the branch of zoology concerned with the systematic measurement
/// and comparison of proportions, dimensions, and metric relationships
/// in animal bodies. Zoometry is to anatomy what surveying is to
/// geography: it replaces verbal descriptions with numbers, turning
/// "large-headed" into a cephalic index, "long-legged" into a limb-
/// ratio, and "deep-chested" into a thoracic-depth coefficient. The
/// discipline was formalised in the eighteenth and nineteenth centuries
/// alongside comparative anatomy and palaeontology, when naturalists
/// recognised that consistent measurement protocols were the only way
/// to communicate structural differences across collections, observers,
/// and generations. A skull in Paris and a skull in London could be
/// compared without either leaving its cabinet, provided both had been
/// measured to the same landmarks — and those landmarks — glabella,
/// nasion, basion, opisthocranion — were codified into standard
/// craniometric reference systems that are still in use. Zoometry
/// extends from osteometry (bones) and odontometry (teeth) through
/// myometry (muscles) and dermatometry (skin thickness) to the
/// morphometric statistical frameworks of the late twentieth century,
/// in which landmark coordinates replace single distances and
/// multivariate statistics separate allometric growth from genuine
/// shape change. `measurement` builds via `calibrate(amount)` and
/// accumulates passively at `calibrate_rate` per second in `tick(dt)`
/// or degrades via `drift(amount)`.
///
/// Models anatomical-calibration fill levels, morphometric-precision
/// saturation bars, body-proportion measurement trackers, species-
/// survey accuracy gauges, craniometric-data fill levels, limb-ratio
/// calibration saturation indicators, scaling-coefficient accumulation
/// bars, measurement-protocol completion meters, biometric-reference
/// fill levels, or any mechanic where patient measurement slowly builds
/// a precise quantitative portrait of an organism — bone by bone,
/// landmark by landmark, ratio by ratio — until the data set is
/// complete enough to distinguish the species, estimate the age, and
/// reconstruct the living creature from its skeleton.
///
/// `calibrate(amount)` adds measurement; fires `just_calibrated` when
/// first reaching `max_measurement`. No-op when disabled.
///
/// `drift(amount)` reduces measurement immediately; fires
/// `just_uncalibrated` when reaching 0. No-op when disabled or
/// already uncalibrated.
///
/// `tick(dt)` clears both flags, then increases measurement by
/// `calibrate_rate * dt` (capped at `max_measurement`). Fires
/// `just_calibrated` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_calibrated()` returns `measurement >= max_measurement && enabled`.
///
/// `is_uncalibrated()` returns `measurement == 0.0` (not gated by
/// `enabled`).
///
/// `measurement_fraction()` returns
/// `(measurement / max_measurement).clamp(0, 1)`.
///
/// `effective_scale(scale)` returns `scale * measurement_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — calibrates at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoometry {
    pub measurement: f32,
    pub max_measurement: f32,
    pub calibrate_rate: f32,
    pub just_calibrated: bool,
    pub just_uncalibrated: bool,
    pub enabled: bool,
}

impl Zoometry {
    pub fn new(max_measurement: f32, calibrate_rate: f32) -> Self {
        Self {
            measurement: 0.0,
            max_measurement: max_measurement.max(0.1),
            calibrate_rate: calibrate_rate.max(0.0),
            just_calibrated: false,
            just_uncalibrated: false,
            enabled: true,
        }
    }

    /// Add measurement; fires `just_calibrated` when first reaching max.
    /// No-op when disabled.
    pub fn calibrate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.measurement < self.max_measurement;
        self.measurement = (self.measurement + amount).min(self.max_measurement);
        if was_below && self.measurement >= self.max_measurement {
            self.just_calibrated = true;
        }
    }

    /// Reduce measurement; fires `just_uncalibrated` when reaching 0.
    /// No-op when disabled or already uncalibrated.
    pub fn drift(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.measurement <= 0.0 {
            return;
        }
        self.measurement = (self.measurement - amount).max(0.0);
        if self.measurement <= 0.0 {
            self.just_uncalibrated = true;
        }
    }

    /// Clear flags, then increase measurement by `calibrate_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_calibrated = false;
        self.just_uncalibrated = false;
        if self.enabled && self.calibrate_rate > 0.0 && self.measurement < self.max_measurement {
            let was_below = self.measurement < self.max_measurement;
            self.measurement =
                (self.measurement + self.calibrate_rate * dt).min(self.max_measurement);
            if was_below && self.measurement >= self.max_measurement {
                self.just_calibrated = true;
            }
        }
    }

    /// `true` when measurement is at maximum and component is enabled.
    pub fn is_calibrated(&self) -> bool {
        self.measurement >= self.max_measurement && self.enabled
    }

    /// `true` when measurement is 0 (not gated by `enabled`).
    pub fn is_uncalibrated(&self) -> bool {
        self.measurement == 0.0
    }

    /// Fraction of maximum measurement [0.0, 1.0].
    pub fn measurement_fraction(&self) -> f32 {
        (self.measurement / self.max_measurement).clamp(0.0, 1.0)
    }

    /// Returns `scale * measurement_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_scale(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.measurement_fraction()
    }
}

impl Default for Zoometry {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoometry {
        Zoometry::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_uncalibrated() {
        let z = z();
        assert_eq!(z.measurement, 0.0);
        assert!(z.is_uncalibrated());
        assert!(!z.is_calibrated());
    }

    #[test]
    fn new_clamps_max_measurement() {
        let z = Zoometry::new(-5.0, 1.5);
        assert!((z.max_measurement - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_calibrate_rate() {
        let z = Zoometry::new(100.0, -1.5);
        assert_eq!(z.calibrate_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoometry::default();
        assert!((z.max_measurement - 100.0).abs() < 1e-5);
        assert!((z.calibrate_rate - 1.5).abs() < 1e-5);
    }

    // --- calibrate ---

    #[test]
    fn calibrate_adds_measurement() {
        let mut z = z();
        z.calibrate(40.0);
        assert!((z.measurement - 40.0).abs() < 1e-3);
    }

    #[test]
    fn calibrate_clamps_at_max() {
        let mut z = z();
        z.calibrate(200.0);
        assert!((z.measurement - 100.0).abs() < 1e-3);
    }

    #[test]
    fn calibrate_fires_just_calibrated_at_max() {
        let mut z = z();
        z.calibrate(100.0);
        assert!(z.just_calibrated);
        assert!(z.is_calibrated());
    }

    #[test]
    fn calibrate_no_just_calibrated_when_already_at_max() {
        let mut z = z();
        z.measurement = 100.0;
        z.calibrate(10.0);
        assert!(!z.just_calibrated);
    }

    #[test]
    fn calibrate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.calibrate(50.0);
        assert_eq!(z.measurement, 0.0);
    }

    #[test]
    fn calibrate_no_op_when_amount_zero() {
        let mut z = z();
        z.calibrate(0.0);
        assert_eq!(z.measurement, 0.0);
    }

    // --- drift ---

    #[test]
    fn drift_reduces_measurement() {
        let mut z = z();
        z.measurement = 60.0;
        z.drift(20.0);
        assert!((z.measurement - 40.0).abs() < 1e-3);
    }

    #[test]
    fn drift_clamps_at_zero() {
        let mut z = z();
        z.measurement = 30.0;
        z.drift(200.0);
        assert_eq!(z.measurement, 0.0);
    }

    #[test]
    fn drift_fires_just_uncalibrated_at_zero() {
        let mut z = z();
        z.measurement = 30.0;
        z.drift(30.0);
        assert!(z.just_uncalibrated);
    }

    #[test]
    fn drift_no_op_when_already_uncalibrated() {
        let mut z = z();
        z.drift(10.0);
        assert!(!z.just_uncalibrated);
    }

    #[test]
    fn drift_no_op_when_disabled() {
        let mut z = z();
        z.measurement = 50.0;
        z.enabled = false;
        z.drift(50.0);
        assert!((z.measurement - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_calibrates_measurement() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.measurement - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_calibrated_on_calibrate_to_max() {
        let mut z = Zoometry::new(100.0, 200.0);
        z.measurement = 95.0;
        z.tick(1.0);
        assert!(z.just_calibrated);
        assert!(z.is_calibrated());
    }

    #[test]
    fn tick_no_calibrate_when_already_calibrated() {
        let mut z = z();
        z.measurement = 100.0;
        z.tick(1.0);
        assert!(!z.just_calibrated);
    }

    #[test]
    fn tick_no_calibrate_when_rate_zero() {
        let mut z = Zoometry::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.measurement, 0.0);
    }

    #[test]
    fn tick_no_calibrate_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.measurement, 0.0);
    }

    #[test]
    fn tick_clears_just_calibrated() {
        let mut z = Zoometry::new(100.0, 200.0);
        z.measurement = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_calibrated);
    }

    #[test]
    fn tick_clears_just_uncalibrated() {
        let mut z = z();
        z.measurement = 10.0;
        z.drift(10.0);
        z.tick(0.016);
        assert!(!z.just_uncalibrated);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.measurement - 9.0).abs() < 1e-3);
    }

    // --- is_calibrated / is_uncalibrated ---

    #[test]
    fn is_calibrated_false_when_disabled() {
        let mut z = z();
        z.measurement = 100.0;
        z.enabled = false;
        assert!(!z.is_calibrated());
    }

    #[test]
    fn is_uncalibrated_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_uncalibrated());
    }

    // --- measurement_fraction / effective_scale ---

    #[test]
    fn measurement_fraction_zero_when_uncalibrated() {
        assert_eq!(z().measurement_fraction(), 0.0);
    }

    #[test]
    fn measurement_fraction_half_at_midpoint() {
        let mut z = z();
        z.measurement = 50.0;
        assert!((z.measurement_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_scale_zero_when_uncalibrated() {
        assert_eq!(z().effective_scale(100.0), 0.0);
    }

    #[test]
    fn effective_scale_scales_with_measurement() {
        let mut z = z();
        z.measurement = 75.0;
        assert!((z.effective_scale(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_scale_zero_when_disabled() {
        let mut z = z();
        z.measurement = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_scale(100.0), 0.0);
    }
}
