use bevy_ecs::prelude::Component;

/// Anti-stealth detection pulse that forces hidden entities within a radius to
/// become visible.
///
/// While active, the detection system should query all nearby entities with a
/// `Stealth` component and suppress their stealth within `radius` world units.
/// `activate(duration)` starts or extends the reveal (high-watermark). `tick(dt)`
/// counts down and sets `just_expired` when the pulse ends.
///
/// Distinct from `Scan` (active radar that pings for positions),
/// `Vision` (sight-line/field-of-view tracking), and `Notice` (proximity alert):
/// Reveal is specifically an anti-stealth field — it doesn't detect arbitrary
/// entities, it only counters entities that are actively hiding.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Reveal {
    pub duration: f32,
    pub timer: f32,
    /// World-unit radius within which hidden entities are forced visible.
    /// Clamped ≥ 0.0.
    pub radius: f32,
    pub just_activated: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Reveal {
    pub fn new(radius: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            radius: radius.max(0.0),
            just_activated: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Start or extend the reveal for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer. No-op when
    /// disabled.
    pub fn activate(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_activated = true;
            }
        }
    }

    /// End the reveal pulse immediately.
    pub fn deactivate(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_expired = true;
        }
    }

    /// Advance the timer; sets `just_expired` when the reveal ends naturally.
    pub fn tick(&mut self, dt: f32) {
        self.just_activated = false;
        self.just_expired = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_expired = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Whether a target at `distance` world units is within the reveal field.
    pub fn in_range(&self, distance: f32) -> bool {
        self.is_active() && distance <= self.radius
    }

    /// Fraction of the reveal duration remaining [1.0 = just activated, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Reveal {
    fn default() -> Self {
        Self::new(12.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_starts_reveal() {
        let mut r = Reveal::new(12.0);
        r.activate(5.0);
        assert!(r.is_active());
        assert!(r.just_activated);
    }

    #[test]
    fn activate_extends_on_longer_duration() {
        let mut r = Reveal::new(12.0);
        r.activate(3.0);
        r.tick(0.016);
        r.activate(8.0);
        assert!((r.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn activate_no_extend_on_shorter_duration() {
        let mut r = Reveal::new(12.0);
        r.activate(8.0);
        r.activate(3.0);
        assert!((r.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn just_activated_not_set_on_extend() {
        let mut r = Reveal::new(12.0);
        r.activate(3.0);
        r.tick(0.016);
        r.activate(8.0);
        assert!(!r.just_activated);
    }

    #[test]
    fn deactivate_ends_reveal() {
        let mut r = Reveal::new(12.0);
        r.activate(5.0);
        r.deactivate();
        assert!(!r.is_active());
        assert!(r.just_expired);
    }

    #[test]
    fn tick_expires_reveal() {
        let mut r = Reveal::new(12.0);
        r.activate(1.0);
        r.tick(1.1);
        assert!(!r.is_active());
        assert!(r.just_expired);
    }

    #[test]
    fn tick_clears_just_activated() {
        let mut r = Reveal::new(12.0);
        r.activate(5.0);
        r.tick(0.016);
        assert!(!r.just_activated);
    }

    #[test]
    fn in_range_true_while_active_within_radius() {
        let mut r = Reveal::new(12.0);
        r.activate(5.0);
        assert!(r.in_range(10.0));
        assert!(r.in_range(12.0));
    }

    #[test]
    fn in_range_false_beyond_radius() {
        let mut r = Reveal::new(12.0);
        r.activate(5.0);
        assert!(!r.in_range(13.0));
    }

    #[test]
    fn in_range_false_when_not_active() {
        let r = Reveal::new(12.0);
        assert!(!r.in_range(5.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut r = Reveal::new(12.0);
        r.activate(4.0);
        r.tick(2.0);
        assert!((r.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_activate_no_op() {
        let mut r = Reveal::new(12.0);
        r.enabled = false;
        r.activate(5.0);
        assert!(!r.is_active());
    }

    #[test]
    fn can_reactivate_after_expiry() {
        let mut r = Reveal::new(12.0);
        r.activate(0.5);
        r.tick(1.0);
        r.tick(0.016);
        r.activate(3.0);
        assert!(r.is_active());
        assert!(r.just_activated);
    }
}
