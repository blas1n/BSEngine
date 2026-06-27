use bevy_ecs::prelude::Component;

/// Navigation-bearing tracker. `bearing` builds via `tack(amount)` and
/// advances passively at `drift_rate` per second in `tick(dt)` or is
/// lost immediately via `veer(amount)`.
///
/// Models sailing-course progress, heading-lock meters, wind-alignment
/// gauges, vehicle bearing indicators, compass-lock trackers, or any
/// mechanic where an entity gradually locks onto a course through
/// sustained effort and can be knocked off that bearing by interference.
///
/// `tack(amount)` adds bearing; fires `just_on_course` when first
/// reaching `max_bearing`. No-op when disabled.
///
/// `veer(amount)` reduces bearing immediately; fires `just_adrift` when
/// reaching 0. No-op when disabled or already adrift.
///
/// `tick(dt)` clears both flags, then advances bearing by
/// `drift_rate * dt` (capped at `max_bearing`). Fires `just_on_course`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_on_course()` returns `bearing >= max_bearing && enabled`.
///
/// `is_adrift()` returns `bearing == 0.0` (not gated by `enabled`).
///
/// `bearing_fraction()` returns `(bearing / max_bearing).clamp(0, 1)`.
///
/// `effective_heading(scale)` returns `scale * bearing_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — drifts into bearing at 5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zebec {
    pub bearing: f32,
    pub max_bearing: f32,
    pub drift_rate: f32,
    pub just_on_course: bool,
    pub just_adrift: bool,
    pub enabled: bool,
}

impl Zebec {
    pub fn new(max_bearing: f32, drift_rate: f32) -> Self {
        Self {
            bearing: 0.0,
            max_bearing: max_bearing.max(0.1),
            drift_rate: drift_rate.max(0.0),
            just_on_course: false,
            just_adrift: false,
            enabled: true,
        }
    }

    /// Add bearing; fires `just_on_course` when first reaching max.
    /// No-op when disabled.
    pub fn tack(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.bearing < self.max_bearing;
        self.bearing = (self.bearing + amount).min(self.max_bearing);
        if was_below && self.bearing >= self.max_bearing {
            self.just_on_course = true;
        }
    }

    /// Reduce bearing; fires `just_adrift` when reaching 0.
    /// No-op when disabled or already adrift.
    pub fn veer(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.bearing <= 0.0 {
            return;
        }
        self.bearing = (self.bearing - amount).max(0.0);
        if self.bearing <= 0.0 {
            self.just_adrift = true;
        }
    }

    /// Clear flags, then advance bearing by `drift_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_on_course = false;
        self.just_adrift = false;
        if self.enabled && self.drift_rate > 0.0 && self.bearing < self.max_bearing {
            let was_below = self.bearing < self.max_bearing;
            self.bearing = (self.bearing + self.drift_rate * dt).min(self.max_bearing);
            if was_below && self.bearing >= self.max_bearing {
                self.just_on_course = true;
            }
        }
    }

    /// `true` when bearing is at maximum and component is enabled.
    pub fn is_on_course(&self) -> bool {
        self.bearing >= self.max_bearing && self.enabled
    }

    /// `true` when bearing is 0 (not gated by `enabled`).
    pub fn is_adrift(&self) -> bool {
        self.bearing == 0.0
    }

    /// Fraction of maximum bearing [0.0, 1.0].
    pub fn bearing_fraction(&self) -> f32 {
        (self.bearing / self.max_bearing).clamp(0.0, 1.0)
    }

    /// Returns `scale * bearing_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_heading(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.bearing_fraction()
    }
}

impl Default for Zebec {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zebec {
        Zebec::new(100.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_adrift() {
        let z = z();
        assert_eq!(z.bearing, 0.0);
        assert!(z.is_adrift());
        assert!(!z.is_on_course());
    }

    #[test]
    fn new_clamps_max_bearing() {
        let z = Zebec::new(-5.0, 5.0);
        assert!((z.max_bearing - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_drift_rate() {
        let z = Zebec::new(100.0, -3.0);
        assert_eq!(z.drift_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zebec::default();
        assert!((z.max_bearing - 100.0).abs() < 1e-5);
        assert!((z.drift_rate - 5.0).abs() < 1e-5);
    }

    // --- tack ---

    #[test]
    fn tack_adds_bearing() {
        let mut z = z();
        z.tack(40.0);
        assert!((z.bearing - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tack_clamps_at_max() {
        let mut z = z();
        z.tack(200.0);
        assert!((z.bearing - 100.0).abs() < 1e-3);
    }

    #[test]
    fn tack_fires_just_on_course_at_max() {
        let mut z = z();
        z.tack(100.0);
        assert!(z.just_on_course);
        assert!(z.is_on_course());
    }

    #[test]
    fn tack_no_just_on_course_when_already_at_max() {
        let mut z = z();
        z.bearing = 100.0;
        z.tack(10.0);
        assert!(!z.just_on_course);
    }

    #[test]
    fn tack_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tack(50.0);
        assert_eq!(z.bearing, 0.0);
    }

    #[test]
    fn tack_no_op_when_amount_zero() {
        let mut z = z();
        z.tack(0.0);
        assert_eq!(z.bearing, 0.0);
    }

    // --- veer ---

    #[test]
    fn veer_reduces_bearing() {
        let mut z = z();
        z.bearing = 60.0;
        z.veer(20.0);
        assert!((z.bearing - 40.0).abs() < 1e-3);
    }

    #[test]
    fn veer_clamps_at_zero() {
        let mut z = z();
        z.bearing = 30.0;
        z.veer(200.0);
        assert_eq!(z.bearing, 0.0);
    }

    #[test]
    fn veer_fires_just_adrift_at_zero() {
        let mut z = z();
        z.bearing = 30.0;
        z.veer(30.0);
        assert!(z.just_adrift);
    }

    #[test]
    fn veer_no_op_when_already_adrift() {
        let mut z = z();
        z.veer(10.0);
        assert!(!z.just_adrift);
    }

    #[test]
    fn veer_no_op_when_disabled() {
        let mut z = z();
        z.bearing = 50.0;
        z.enabled = false;
        z.veer(50.0);
        assert!((z.bearing - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_drifts_bearing() {
        let mut z = z(); // drift=5
        z.tick(1.0); // 0 + 5 = 5
        assert!((z.bearing - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_on_course_on_drift_to_max() {
        let mut z = Zebec::new(100.0, 200.0);
        z.bearing = 95.0;
        z.tick(1.0);
        assert!(z.just_on_course);
        assert!(z.is_on_course());
    }

    #[test]
    fn tick_no_drift_when_already_at_max() {
        let mut z = z();
        z.bearing = 100.0;
        z.tick(1.0);
        assert!(!z.just_on_course);
    }

    #[test]
    fn tick_no_drift_when_rate_zero() {
        let mut z = Zebec::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.bearing, 0.0);
    }

    #[test]
    fn tick_no_drift_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.bearing, 0.0);
    }

    #[test]
    fn tick_clears_just_on_course() {
        let mut z = Zebec::new(100.0, 200.0);
        z.bearing = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_on_course);
    }

    #[test]
    fn tick_clears_just_adrift() {
        let mut z = z();
        z.bearing = 10.0;
        z.veer(10.0);
        z.tick(0.016);
        assert!(!z.just_adrift);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // drift=5
        z.tick(5.0); // 5*5 = 25
        assert!((z.bearing - 25.0).abs() < 1e-3);
    }

    // --- is_on_course / is_adrift ---

    #[test]
    fn is_on_course_false_when_disabled() {
        let mut z = z();
        z.bearing = 100.0;
        z.enabled = false;
        assert!(!z.is_on_course());
    }

    #[test]
    fn is_adrift_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_adrift());
    }

    // --- bearing_fraction / effective_heading ---

    #[test]
    fn bearing_fraction_zero_when_adrift() {
        assert_eq!(z().bearing_fraction(), 0.0);
    }

    #[test]
    fn bearing_fraction_half_at_midpoint() {
        let mut z = z();
        z.bearing = 50.0;
        assert!((z.bearing_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_heading_zero_when_adrift() {
        assert_eq!(z().effective_heading(100.0), 0.0);
    }

    #[test]
    fn effective_heading_scales_with_bearing() {
        let mut z = z();
        z.bearing = 55.0;
        assert!((z.effective_heading(100.0) - 55.0).abs() < 1e-3);
    }

    #[test]
    fn effective_heading_zero_when_disabled() {
        let mut z = z();
        z.bearing = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_heading(100.0), 0.0);
    }
}
