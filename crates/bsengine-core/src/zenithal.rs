use bevy_ecs::prelude::Component;

/// Peak-elevation tracker. `elevation` builds via `ascend(amount)` and
/// climbs passively at `rise_rate` per second in `tick(dt)` or drops
/// immediately via `descend(amount)`.
///
/// Models celestial-body peak-altitude gauges, sun-angle elevation bars,
/// sky-dome zenith-tracking meters, astrolabe reading accumulators,
/// solar-noon intensity fill levels, star-culmination progress trackers,
/// observatory-dome rotation gauges, or any mechanic where something
/// climbs toward its highest point before descent.
///
/// `ascend(amount)` adds elevation; fires `just_peaked` when first
/// reaching `max_elevation`. No-op when disabled.
///
/// `descend(amount)` reduces elevation immediately; fires `just_nadir`
/// when reaching 0. No-op when disabled or already at nadir.
///
/// `tick(dt)` clears both flags, then increases elevation by
/// `rise_rate * dt` (capped at `max_elevation`). Fires `just_peaked`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_peaked()` returns `elevation >= max_elevation && enabled`.
///
/// `is_nadir()` returns `elevation == 0.0` (not gated by `enabled`).
///
/// `elevation_fraction()` returns `(elevation / max_elevation).clamp(0, 1)`.
///
/// `effective_altitude(scale)` returns `scale * elevation_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — rises at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zenithal {
    pub elevation: f32,
    pub max_elevation: f32,
    pub rise_rate: f32,
    pub just_peaked: bool,
    pub just_nadir: bool,
    pub enabled: bool,
}

impl Zenithal {
    pub fn new(max_elevation: f32, rise_rate: f32) -> Self {
        Self {
            elevation: 0.0,
            max_elevation: max_elevation.max(0.1),
            rise_rate: rise_rate.max(0.0),
            just_peaked: false,
            just_nadir: false,
            enabled: true,
        }
    }

    /// Add elevation; fires `just_peaked` when first reaching max.
    /// No-op when disabled.
    pub fn ascend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.elevation < self.max_elevation;
        self.elevation = (self.elevation + amount).min(self.max_elevation);
        if was_below && self.elevation >= self.max_elevation {
            self.just_peaked = true;
        }
    }

    /// Reduce elevation; fires `just_nadir` when reaching 0.
    /// No-op when disabled or already at nadir.
    pub fn descend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.elevation <= 0.0 {
            return;
        }
        self.elevation = (self.elevation - amount).max(0.0);
        if self.elevation <= 0.0 {
            self.just_nadir = true;
        }
    }

    /// Clear flags, then increase elevation by `rise_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_nadir = false;
        if self.enabled && self.rise_rate > 0.0 && self.elevation < self.max_elevation {
            let was_below = self.elevation < self.max_elevation;
            self.elevation = (self.elevation + self.rise_rate * dt).min(self.max_elevation);
            if was_below && self.elevation >= self.max_elevation {
                self.just_peaked = true;
            }
        }
    }

    /// `true` when elevation is at maximum and component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.elevation >= self.max_elevation && self.enabled
    }

    /// `true` when elevation is 0 (not gated by `enabled`).
    pub fn is_nadir(&self) -> bool {
        self.elevation == 0.0
    }

    /// Fraction of maximum elevation [0.0, 1.0].
    pub fn elevation_fraction(&self) -> f32 {
        (self.elevation / self.max_elevation).clamp(0.0, 1.0)
    }

    /// Returns `scale * elevation_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_altitude(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.elevation_fraction()
    }
}

impl Default for Zenithal {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zenithal {
        Zenithal::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_at_nadir() {
        let z = z();
        assert_eq!(z.elevation, 0.0);
        assert!(z.is_nadir());
        assert!(!z.is_peaked());
    }

    #[test]
    fn new_clamps_max_elevation() {
        let z = Zenithal::new(-5.0, 2.0);
        assert!((z.max_elevation - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_rise_rate() {
        let z = Zenithal::new(100.0, -3.0);
        assert_eq!(z.rise_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zenithal::default();
        assert!((z.max_elevation - 100.0).abs() < 1e-5);
        assert!((z.rise_rate - 2.0).abs() < 1e-5);
    }

    // --- ascend ---

    #[test]
    fn ascend_adds_elevation() {
        let mut z = z();
        z.ascend(40.0);
        assert!((z.elevation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn ascend_clamps_at_max() {
        let mut z = z();
        z.ascend(200.0);
        assert!((z.elevation - 100.0).abs() < 1e-3);
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
        z.elevation = 100.0;
        z.ascend(10.0);
        assert!(!z.just_peaked);
    }

    #[test]
    fn ascend_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.ascend(50.0);
        assert_eq!(z.elevation, 0.0);
    }

    #[test]
    fn ascend_no_op_when_amount_zero() {
        let mut z = z();
        z.ascend(0.0);
        assert_eq!(z.elevation, 0.0);
    }

    // --- descend ---

    #[test]
    fn descend_reduces_elevation() {
        let mut z = z();
        z.elevation = 60.0;
        z.descend(20.0);
        assert!((z.elevation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn descend_clamps_at_zero() {
        let mut z = z();
        z.elevation = 30.0;
        z.descend(200.0);
        assert_eq!(z.elevation, 0.0);
    }

    #[test]
    fn descend_fires_just_nadir_at_zero() {
        let mut z = z();
        z.elevation = 30.0;
        z.descend(30.0);
        assert!(z.just_nadir);
    }

    #[test]
    fn descend_no_op_when_already_at_nadir() {
        let mut z = z();
        z.descend(10.0);
        assert!(!z.just_nadir);
    }

    #[test]
    fn descend_no_op_when_disabled() {
        let mut z = z();
        z.elevation = 50.0;
        z.enabled = false;
        z.descend(50.0);
        assert!((z.elevation - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_rises_elevation() {
        let mut z = z(); // rate=2
        z.tick(1.0); // 0 + 2 = 2
        assert!((z.elevation - 2.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_peaked_on_rise_to_max() {
        let mut z = Zenithal::new(100.0, 200.0);
        z.elevation = 95.0;
        z.tick(1.0);
        assert!(z.just_peaked);
        assert!(z.is_peaked());
    }

    #[test]
    fn tick_no_rise_when_already_peaked() {
        let mut z = z();
        z.elevation = 100.0;
        z.tick(1.0);
        assert!(!z.just_peaked);
    }

    #[test]
    fn tick_no_rise_when_rate_zero() {
        let mut z = Zenithal::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.elevation, 0.0);
    }

    #[test]
    fn tick_no_rise_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.elevation, 0.0);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut z = Zenithal::new(100.0, 200.0);
        z.elevation = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_peaked);
    }

    #[test]
    fn tick_clears_just_nadir() {
        let mut z = z();
        z.elevation = 10.0;
        z.descend(10.0);
        z.tick(0.016);
        assert!(!z.just_nadir);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 2*3 = 6
        assert!((z.elevation - 6.0).abs() < 1e-3);
    }

    // --- is_peaked / is_nadir ---

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut z = z();
        z.elevation = 100.0;
        z.enabled = false;
        assert!(!z.is_peaked());
    }

    #[test]
    fn is_nadir_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_nadir());
    }

    // --- elevation_fraction / effective_altitude ---

    #[test]
    fn elevation_fraction_zero_at_nadir() {
        assert_eq!(z().elevation_fraction(), 0.0);
    }

    #[test]
    fn elevation_fraction_half_at_midpoint() {
        let mut z = z();
        z.elevation = 50.0;
        assert!((z.elevation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_altitude_zero_at_nadir() {
        assert_eq!(z().effective_altitude(100.0), 0.0);
    }

    #[test]
    fn effective_altitude_scales_with_elevation() {
        let mut z = z();
        z.elevation = 60.0;
        assert!((z.effective_altitude(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_altitude_zero_when_disabled() {
        let mut z = z();
        z.elevation = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_altitude(100.0), 0.0);
    }
}
