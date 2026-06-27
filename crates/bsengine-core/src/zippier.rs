use bevy_ecs::prelude::Component;

/// Speed-comparison tracker. `pace` builds via `accelerate(amount)` and
/// increases passively at `sprint_rate` per second in `tick(dt)` or is
/// slowed immediately via `decelerate(amount)`.
///
/// Models racing-speed comparison bars, velocity-advantage fill levels,
/// competitive-pace escalation trackers, overtaking-momentum gauges,
/// sprint-burst accumulation indicators, courier-efficiency meters,
/// wind-resistance saturation bars, greyhound-acceleration trackers,
/// quick-delivery urgency fill levels, or any mechanic where relative
/// quickness over a rival accumulates into a decisive speed advantage
/// before circumstance robs the faster party of its comfortable lead
/// and things begin again at equal footing.
///
/// `accelerate(amount)` adds pace; fires `just_fastest` when first
/// reaching `max_pace`. No-op when disabled.
///
/// `decelerate(amount)` reduces pace immediately; fires `just_stalled`
/// when reaching 0. No-op when disabled or already stalled.
///
/// `tick(dt)` clears both flags, then increases pace by
/// `sprint_rate * dt` (capped at `max_pace`). Fires `just_fastest`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_fastest()` returns `pace >= max_pace && enabled`.
///
/// `is_stalled()` returns `pace == 0.0` (not gated by `enabled`).
///
/// `pace_fraction()` returns `(pace / max_pace).clamp(0, 1)`.
///
/// `effective_speed(scale)` returns `scale * pace_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — sprints at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zippier {
    pub pace: f32,
    pub max_pace: f32,
    pub sprint_rate: f32,
    pub just_fastest: bool,
    pub just_stalled: bool,
    pub enabled: bool,
}

impl Zippier {
    pub fn new(max_pace: f32, sprint_rate: f32) -> Self {
        Self {
            pace: 0.0,
            max_pace: max_pace.max(0.1),
            sprint_rate: sprint_rate.max(0.0),
            just_fastest: false,
            just_stalled: false,
            enabled: true,
        }
    }

    /// Add pace; fires `just_fastest` when first reaching max.
    /// No-op when disabled.
    pub fn accelerate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.pace < self.max_pace;
        self.pace = (self.pace + amount).min(self.max_pace);
        if was_below && self.pace >= self.max_pace {
            self.just_fastest = true;
        }
    }

    /// Reduce pace; fires `just_stalled` when reaching 0.
    /// No-op when disabled or already stalled.
    pub fn decelerate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pace <= 0.0 {
            return;
        }
        self.pace = (self.pace - amount).max(0.0);
        if self.pace <= 0.0 {
            self.just_stalled = true;
        }
    }

    /// Clear flags, then increase pace by `sprint_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_fastest = false;
        self.just_stalled = false;
        if self.enabled && self.sprint_rate > 0.0 && self.pace < self.max_pace {
            let was_below = self.pace < self.max_pace;
            self.pace = (self.pace + self.sprint_rate * dt).min(self.max_pace);
            if was_below && self.pace >= self.max_pace {
                self.just_fastest = true;
            }
        }
    }

    /// `true` when pace is at maximum and component is enabled.
    pub fn is_fastest(&self) -> bool {
        self.pace >= self.max_pace && self.enabled
    }

    /// `true` when pace is 0 (not gated by `enabled`).
    pub fn is_stalled(&self) -> bool {
        self.pace == 0.0
    }

    /// Fraction of maximum pace [0.0, 1.0].
    pub fn pace_fraction(&self) -> f32 {
        (self.pace / self.max_pace).clamp(0.0, 1.0)
    }

    /// Returns `scale * pace_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_speed(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.pace_fraction()
    }
}

impl Default for Zippier {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zippier {
        Zippier::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_stalled() {
        let z = z();
        assert_eq!(z.pace, 0.0);
        assert!(z.is_stalled());
        assert!(!z.is_fastest());
    }

    #[test]
    fn new_clamps_max_pace() {
        let z = Zippier::new(-5.0, 4.0);
        assert!((z.max_pace - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_sprint_rate() {
        let z = Zippier::new(100.0, -4.0);
        assert_eq!(z.sprint_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zippier::default();
        assert!((z.max_pace - 100.0).abs() < 1e-5);
        assert!((z.sprint_rate - 4.0).abs() < 1e-5);
    }

    // --- accelerate ---

    #[test]
    fn accelerate_adds_pace() {
        let mut z = z();
        z.accelerate(40.0);
        assert!((z.pace - 40.0).abs() < 1e-3);
    }

    #[test]
    fn accelerate_clamps_at_max() {
        let mut z = z();
        z.accelerate(200.0);
        assert!((z.pace - 100.0).abs() < 1e-3);
    }

    #[test]
    fn accelerate_fires_just_fastest_at_max() {
        let mut z = z();
        z.accelerate(100.0);
        assert!(z.just_fastest);
        assert!(z.is_fastest());
    }

    #[test]
    fn accelerate_no_just_fastest_when_already_at_max() {
        let mut z = z();
        z.pace = 100.0;
        z.accelerate(10.0);
        assert!(!z.just_fastest);
    }

    #[test]
    fn accelerate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.accelerate(50.0);
        assert_eq!(z.pace, 0.0);
    }

    #[test]
    fn accelerate_no_op_when_amount_zero() {
        let mut z = z();
        z.accelerate(0.0);
        assert_eq!(z.pace, 0.0);
    }

    // --- decelerate ---

    #[test]
    fn decelerate_reduces_pace() {
        let mut z = z();
        z.pace = 60.0;
        z.decelerate(20.0);
        assert!((z.pace - 40.0).abs() < 1e-3);
    }

    #[test]
    fn decelerate_clamps_at_zero() {
        let mut z = z();
        z.pace = 30.0;
        z.decelerate(200.0);
        assert_eq!(z.pace, 0.0);
    }

    #[test]
    fn decelerate_fires_just_stalled_at_zero() {
        let mut z = z();
        z.pace = 30.0;
        z.decelerate(30.0);
        assert!(z.just_stalled);
    }

    #[test]
    fn decelerate_no_op_when_already_stalled() {
        let mut z = z();
        z.decelerate(10.0);
        assert!(!z.just_stalled);
    }

    #[test]
    fn decelerate_no_op_when_disabled() {
        let mut z = z();
        z.pace = 50.0;
        z.enabled = false;
        z.decelerate(50.0);
        assert!((z.pace - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_increases_pace() {
        let mut z = z(); // rate=4
        z.tick(2.0); // 0 + 4*2 = 8
        assert!((z.pace - 8.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_fastest_on_sprint_to_max() {
        let mut z = Zippier::new(100.0, 200.0);
        z.pace = 95.0;
        z.tick(1.0);
        assert!(z.just_fastest);
        assert!(z.is_fastest());
    }

    #[test]
    fn tick_no_sprint_when_already_fastest() {
        let mut z = z();
        z.pace = 100.0;
        z.tick(1.0);
        assert!(!z.just_fastest);
    }

    #[test]
    fn tick_no_sprint_when_rate_zero() {
        let mut z = Zippier::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.pace, 0.0);
    }

    #[test]
    fn tick_no_sprint_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.pace, 0.0);
    }

    #[test]
    fn tick_clears_just_fastest() {
        let mut z = Zippier::new(100.0, 200.0);
        z.pace = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_fastest);
    }

    #[test]
    fn tick_clears_just_stalled() {
        let mut z = z();
        z.pace = 10.0;
        z.decelerate(10.0);
        z.tick(0.016);
        assert!(!z.just_stalled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=4
        z.tick(3.0); // 4*3 = 12
        assert!((z.pace - 12.0).abs() < 1e-3);
    }

    // --- is_fastest / is_stalled ---

    #[test]
    fn is_fastest_false_when_disabled() {
        let mut z = z();
        z.pace = 100.0;
        z.enabled = false;
        assert!(!z.is_fastest());
    }

    #[test]
    fn is_stalled_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_stalled());
    }

    // --- pace_fraction / effective_speed ---

    #[test]
    fn pace_fraction_zero_when_stalled() {
        assert_eq!(z().pace_fraction(), 0.0);
    }

    #[test]
    fn pace_fraction_half_at_midpoint() {
        let mut z = z();
        z.pace = 50.0;
        assert!((z.pace_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_zero_when_stalled() {
        assert_eq!(z().effective_speed(100.0), 0.0);
    }

    #[test]
    fn effective_speed_scales_with_pace() {
        let mut z = z();
        z.pace = 75.0;
        assert!((z.effective_speed(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_zero_when_disabled() {
        let mut z = z();
        z.pace = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_speed(100.0), 0.0);
    }
}
