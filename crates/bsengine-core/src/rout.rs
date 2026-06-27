use bevy_ecs::prelude::Component;

/// Crowd-control debuff that forces the entity to flee at elevated speed.
///
/// While `is_active()`, movement systems should apply `flee_speed(base_speed)`
/// as the entity's movement speed and steer the entity away from the threat
/// source (direction is managed by the caller — Rout only controls magnitude
/// and timing). `apply(duration)` uses high-watermark. `tick(dt)` counts down
/// and sets `just_recovered` when the rout fades.
///
/// Distinct from `Fear` (broad panic that overrides all actions), `Root` (no
/// movement at all), and `Charm` (entity fights for the opposing side): Rout
/// is a targeted tactical debuff — the entity is forced to flee but retains
/// its speed (and then some), making it fast but dangerous to chase.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Rout {
    pub duration: f32,
    pub timer: f32,
    /// Speed multiplier applied while routing. Clamped ≥ 1.0.
    /// e.g. 1.5 = entity flees at 150% of its normal movement speed.
    pub flee_speed_multiplier: f32,
    pub just_routed: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Rout {
    pub fn new(flee_speed_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            flee_speed_multiplier: flee_speed_multiplier.max(1.0),
            just_routed: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Apply or extend the rout for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer. No-op when
    /// disabled.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_routed = true;
            }
        }
    }

    /// End the rout immediately (e.g., entity is stunned, reaches a wall).
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_recovered = true;
        }
    }

    /// Advance the timer; sets `just_recovered` when the rout expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_routed = false;
        self.just_recovered = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_recovered = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Movement speed while routing. Returns `base * flee_speed_multiplier`
    /// when active and enabled, `base` otherwise.
    pub fn flee_speed(&self, base: f32) -> f32 {
        if self.is_active() && self.enabled {
            base * self.flee_speed_multiplier
        } else {
            base
        }
    }

    /// Fraction of the rout duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Rout {
    fn default() -> Self {
        Self::new(1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_rout() {
        let mut r = Rout::new(1.5);
        r.apply(3.0);
        assert!(r.is_active());
        assert!(r.just_routed);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut r = Rout::new(1.5);
        r.apply(2.0);
        r.tick(0.016);
        r.apply(5.0);
        assert!((r.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut r = Rout::new(1.5);
        r.apply(5.0);
        r.apply(2.0);
        assert!((r.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn just_routed_not_set_on_extend() {
        let mut r = Rout::new(1.5);
        r.apply(2.0);
        r.tick(0.016);
        r.apply(5.0);
        assert!(!r.just_routed);
    }

    #[test]
    fn clear_ends_rout() {
        let mut r = Rout::new(1.5);
        r.apply(3.0);
        r.clear();
        assert!(!r.is_active());
        assert!(r.just_recovered);
    }

    #[test]
    fn tick_expires_rout() {
        let mut r = Rout::new(1.5);
        r.apply(1.0);
        r.tick(1.1);
        assert!(!r.is_active());
        assert!(r.just_recovered);
    }

    #[test]
    fn tick_clears_just_routed() {
        let mut r = Rout::new(1.5);
        r.apply(3.0);
        r.tick(0.016);
        assert!(!r.just_routed);
    }

    #[test]
    fn flee_speed_boosted_while_active() {
        let mut r = Rout::new(1.5);
        r.apply(3.0);
        assert!((r.flee_speed(10.0) - 15.0).abs() < 1e-4); // 10 * 1.5
    }

    #[test]
    fn flee_speed_unaffected_when_inactive() {
        let r = Rout::new(1.5);
        assert!((r.flee_speed(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut r = Rout::new(1.5);
        r.apply(4.0);
        r.tick(2.0);
        assert!((r.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn flee_speed_multiplier_clamped_to_one() {
        let r = Rout::new(0.5); // below 1.0
        assert!((r.flee_speed_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut r = Rout::new(1.5);
        r.enabled = false;
        r.apply(3.0);
        assert!(!r.is_active());
    }

    #[test]
    fn disabled_flee_speed_unaffected() {
        let mut r = Rout::new(1.5);
        r.apply(3.0);
        r.enabled = false;
        assert!((r.flee_speed(10.0) - 10.0).abs() < 1e-5);
    }
}
