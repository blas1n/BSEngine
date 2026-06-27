use bevy_ecs::prelude::Component;

/// Apex-ascent tracker. `altitude` climbs via `ascend(amount)` and drifts
/// back down passively at `descent_rate` per second in `tick(dt)` or
/// immediately via `descend(amount)`.
///
/// Models altitude meters, peak-performance gauges, momentum-apex trackers,
/// achievement-peak bars, crowd-hype climbers, or any mechanic where a
/// value rises toward a ceiling before gradually falling back down.
///
/// `ascend(amount)` adds altitude; fires `just_peaked` when first reaching
/// `max_altitude`. No-op when disabled.
///
/// `descend(amount)` reduces altitude immediately; fires `just_grounded`
/// when reaching 0. No-op when disabled or already grounded.
///
/// `tick(dt)` clears both flags, then drifts altitude down by
/// `descent_rate * dt` (floored at 0). Fires `just_grounded` when reaching
/// 0 via drift. No-op when disabled or rate is 0.
///
/// `is_peaked()` returns `altitude >= max_altitude && enabled`.
///
/// `is_grounded()` returns `altitude == 0.0` (not gated by `enabled`).
///
/// `altitude_fraction()` returns `(altitude / max_altitude).clamp(0, 1)`.
///
/// `effective_height(scale)` returns `scale * altitude_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 10.0)` — drifts at 10 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zenith {
    pub altitude: f32,
    pub max_altitude: f32,
    pub descent_rate: f32,
    pub just_peaked: bool,
    pub just_grounded: bool,
    pub enabled: bool,
}

impl Zenith {
    pub fn new(max_altitude: f32, descent_rate: f32) -> Self {
        Self {
            altitude: 0.0,
            max_altitude: max_altitude.max(0.1),
            descent_rate: descent_rate.max(0.0),
            just_peaked: false,
            just_grounded: false,
            enabled: true,
        }
    }

    /// Add altitude; fires `just_peaked` when first reaching max.
    /// No-op when disabled.
    pub fn ascend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.altitude < self.max_altitude;
        self.altitude = (self.altitude + amount).min(self.max_altitude);
        if was_below && self.altitude >= self.max_altitude {
            self.just_peaked = true;
        }
    }

    /// Reduce altitude; fires `just_grounded` when reaching 0.
    /// No-op when disabled or already grounded.
    pub fn descend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.altitude <= 0.0 {
            return;
        }
        self.altitude = (self.altitude - amount).max(0.0);
        if self.altitude <= 0.0 {
            self.just_grounded = true;
        }
    }

    /// Clear flags, then drift altitude down by `descent_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_grounded = false;
        if self.enabled && self.descent_rate > 0.0 && self.altitude > 0.0 {
            self.altitude = (self.altitude - self.descent_rate * dt).max(0.0);
            if self.altitude <= 0.0 {
                self.just_grounded = true;
            }
        }
    }

    /// `true` when altitude is at maximum and component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.altitude >= self.max_altitude && self.enabled
    }

    /// `true` when altitude is 0 (not gated by `enabled`).
    pub fn is_grounded(&self) -> bool {
        self.altitude == 0.0
    }

    /// Fraction of maximum altitude [0.0, 1.0].
    pub fn altitude_fraction(&self) -> f32 {
        (self.altitude / self.max_altitude).clamp(0.0, 1.0)
    }

    /// Returns `scale * altitude_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_height(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.altitude_fraction()
    }
}

impl Default for Zenith {
    fn default() -> Self {
        Self::new(100.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zenith {
        Zenith::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_grounded() {
        let z = z();
        assert_eq!(z.altitude, 0.0);
        assert!(z.is_grounded());
        assert!(!z.is_peaked());
    }

    #[test]
    fn new_clamps_max_altitude() {
        let z = Zenith::new(-5.0, 10.0);
        assert!((z.max_altitude - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_descent_rate() {
        let z = Zenith::new(100.0, -3.0);
        assert_eq!(z.descent_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zenith::default();
        assert!((z.max_altitude - 100.0).abs() < 1e-5);
        assert!((z.descent_rate - 10.0).abs() < 1e-5);
    }

    // --- ascend ---

    #[test]
    fn ascend_adds_altitude() {
        let mut z = z();
        z.ascend(40.0);
        assert!((z.altitude - 40.0).abs() < 1e-3);
    }

    #[test]
    fn ascend_clamps_at_max() {
        let mut z = z();
        z.ascend(200.0);
        assert!((z.altitude - 100.0).abs() < 1e-3);
    }

    #[test]
    fn ascend_fires_just_peaked_at_max() {
        let mut z = z();
        z.ascend(100.0);
        assert!(z.just_peaked);
        assert!(z.is_peaked());
    }

    #[test]
    fn ascend_no_just_peaked_when_already_at_max() {
        let mut z = z();
        z.altitude = 100.0;
        z.ascend(10.0);
        assert!(!z.just_peaked);
    }

    #[test]
    fn ascend_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.ascend(50.0);
        assert_eq!(z.altitude, 0.0);
    }

    #[test]
    fn ascend_no_op_when_amount_zero() {
        let mut z = z();
        z.ascend(0.0);
        assert_eq!(z.altitude, 0.0);
    }

    // --- descend ---

    #[test]
    fn descend_reduces_altitude() {
        let mut z = z();
        z.altitude = 60.0;
        z.descend(20.0);
        assert!((z.altitude - 40.0).abs() < 1e-3);
    }

    #[test]
    fn descend_clamps_at_zero() {
        let mut z = z();
        z.altitude = 30.0;
        z.descend(200.0);
        assert_eq!(z.altitude, 0.0);
    }

    #[test]
    fn descend_fires_just_grounded_at_zero() {
        let mut z = z();
        z.altitude = 30.0;
        z.descend(30.0);
        assert!(z.just_grounded);
    }

    #[test]
    fn descend_no_op_when_already_grounded() {
        let mut z = z();
        z.descend(10.0);
        assert!(!z.just_grounded);
    }

    #[test]
    fn descend_no_op_when_disabled() {
        let mut z = z();
        z.altitude = 50.0;
        z.enabled = false;
        z.descend(50.0);
        assert!((z.altitude - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_drifts_altitude_down() {
        let mut z = z(); // descent=10
        z.altitude = 60.0;
        z.tick(1.0); // 60 - 10 = 50
        assert!((z.altitude - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_grounded_on_drift_to_zero() {
        let mut z = Zenith::new(100.0, 200.0);
        z.altitude = 5.0;
        z.tick(1.0);
        assert!(z.just_grounded);
        assert!(z.is_grounded());
    }

    #[test]
    fn tick_no_drift_when_already_grounded() {
        let mut z = z();
        z.tick(10.0);
        assert!(!z.just_grounded);
    }

    #[test]
    fn tick_no_drift_when_rate_zero() {
        let mut z = Zenith::new(100.0, 0.0);
        z.altitude = 50.0;
        z.tick(100.0);
        assert!((z.altitude - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_drift_when_disabled() {
        let mut z = z();
        z.altitude = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.altitude - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut z = z();
        z.ascend(100.0);
        z.tick(0.016);
        assert!(!z.just_peaked);
    }

    #[test]
    fn tick_clears_just_grounded() {
        let mut z = Zenith::new(100.0, 200.0);
        z.altitude = 5.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_grounded);
    }

    #[test]
    fn tick_scales_drift_with_dt() {
        let mut z = z(); // descent=10
        z.altitude = 100.0;
        z.tick(3.0); // 100 - 10*3 = 70
        assert!((z.altitude - 70.0).abs() < 1e-3);
    }

    // --- is_peaked / is_grounded ---

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut z = z();
        z.altitude = 100.0;
        z.enabled = false;
        assert!(!z.is_peaked());
    }

    #[test]
    fn is_grounded_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_grounded());
    }

    // --- altitude_fraction / effective_height ---

    #[test]
    fn altitude_fraction_zero_when_grounded() {
        assert_eq!(z().altitude_fraction(), 0.0);
    }

    #[test]
    fn altitude_fraction_half_at_midpoint() {
        let mut z = z();
        z.altitude = 50.0;
        assert!((z.altitude_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_height_zero_when_grounded() {
        assert_eq!(z().effective_height(100.0), 0.0);
    }

    #[test]
    fn effective_height_scales_with_altitude() {
        let mut z = z();
        z.altitude = 70.0;
        assert!((z.effective_height(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_height_zero_when_disabled() {
        let mut z = z();
        z.altitude = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_height(100.0), 0.0);
    }
}
