use bevy_ecs::prelude::Component;

/// Periodic detection pulse for awareness and sensor systems.
///
/// On each pulse (`just_pulsed = true`), the AI or sensor system should query
/// all entities within `radius` units. Results are not stored here — this
/// component only drives the timing and radius; the caller decides what to do
/// with the detected entities.
///
/// `tick(dt)` advances the timer. `trigger()` fires an immediate pulse
/// regardless of the timer (e.g. an active-scan ability). `set_radius(r)` lets
/// the system dynamically shrink the scan radius (e.g. when deafened or blinded).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Scan {
    /// Detection range in world units.
    pub radius: f32,
    /// Seconds between automatic scan pulses.
    pub interval: f32,
    /// Counts up to `interval`; resets on each pulse.
    pub timer: f32,
    /// True on the first frame a scan pulse fires (natural or triggered).
    pub just_pulsed: bool,
    pub enabled: bool,
}

impl Scan {
    pub fn new(radius: f32, interval: f32) -> Self {
        Self {
            radius: radius.max(0.0),
            interval: interval.max(0.0),
            timer: 0.0,
            just_pulsed: false,
            enabled: true,
        }
    }

    /// Advance the scan timer by `dt` seconds. Fires a pulse when timer ≥ interval.
    pub fn tick(&mut self, dt: f32) {
        self.just_pulsed = false;

        if !self.enabled || self.interval <= 0.0 {
            return;
        }

        self.timer += dt;
        if self.timer >= self.interval {
            self.timer -= self.interval;
            self.just_pulsed = true;
        }
    }

    /// Fire an immediate scan pulse regardless of the timer.
    pub fn trigger(&mut self) {
        if self.enabled {
            self.just_pulsed = true;
        }
    }

    /// Update the scan radius (e.g. shrink when awareness is impaired).
    pub fn set_radius(&mut self, radius: f32) {
        self.radius = radius.max(0.0);
    }

    /// Fraction of the current interval elapsed [0.0, 1.0].
    pub fn charge_fraction(&self) -> f32 {
        if self.interval <= 0.0 {
            return 1.0;
        }
        (self.timer / self.interval).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_fires_at_interval() {
        let mut s = Scan::new(10.0, 2.0);
        s.tick(1.9);
        assert!(!s.just_pulsed);
        s.tick(0.2); // crosses 2.0
        assert!(s.just_pulsed);
    }

    #[test]
    fn timer_wraps_after_pulse() {
        let mut s = Scan::new(10.0, 2.0);
        s.tick(2.5); // 0.5 remainder
        assert!(s.just_pulsed);
        assert!((s.timer - 0.5).abs() < 1e-5);
    }

    #[test]
    fn just_pulsed_clears_next_tick() {
        let mut s = Scan::new(10.0, 1.0);
        s.tick(1.1);
        assert!(s.just_pulsed);
        s.tick(0.1);
        assert!(!s.just_pulsed);
    }

    #[test]
    fn trigger_fires_immediately() {
        let mut s = Scan::new(10.0, 5.0);
        s.trigger();
        assert!(s.just_pulsed);
    }

    #[test]
    fn disabled_does_not_pulse() {
        let mut s = Scan::new(10.0, 1.0);
        s.enabled = false;
        s.tick(2.0);
        assert!(!s.just_pulsed);
    }

    #[test]
    fn disabled_trigger_ignored() {
        let mut s = Scan::new(10.0, 1.0);
        s.enabled = false;
        s.trigger();
        assert!(!s.just_pulsed);
    }

    #[test]
    fn charge_fraction_zero_to_one() {
        let mut s = Scan::new(10.0, 4.0);
        s.tick(1.0);
        assert!((s.charge_fraction() - 0.25).abs() < 1e-5);
    }

    #[test]
    fn set_radius_updates_radius() {
        let mut s = Scan::new(10.0, 1.0);
        s.set_radius(5.0);
        assert!((s.radius - 5.0).abs() < 1e-5);
    }
}
