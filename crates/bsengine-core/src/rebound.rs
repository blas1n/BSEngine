use bevy_ecs::prelude::Component;

/// Surface contact elasticity: when the entity hits a solid surface or
/// absorbs a heavy impact, the physics system calls `check_impact(speed)`.
/// If `speed ≥ min_speed` and the component is enabled, the entity bounces
/// back at `speed * rebound_coefficient`, `just_rebounded` is set, and
/// `last_rebound_speed` records the computed velocity. `tick()` clears the
/// one-frame flag.
///
/// `rebound_speed(impact_speed)` computes the output speed without triggering
/// state changes — useful for rendering or predictive systems.
///
/// Distinct from `Recoil` (gun-fire kickback / screen shake on the shooter),
/// `Knockback` (pushing the entity away from a damage source), and `Ricochet`
/// (deflecting a projectile off a surface): Rebound is **surface contact
/// elasticity** — the entity itself bounces like a rubber ball proportional
/// to the incoming impact speed, not a projectile or gun-fire event.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Rebound {
    /// Fraction of impact speed that becomes rebound speed. Clamped to [0.0, 1.0].
    pub rebound_coefficient: f32,
    /// Minimum impact speed required to trigger a rebound. Clamped ≥ 0.0.
    pub min_speed: f32,
    /// Computed speed of the last rebound. Zero when no rebound has fired.
    pub last_rebound_speed: f32,
    pub just_rebounded: bool,
    pub enabled: bool,
}

impl Rebound {
    pub fn new(rebound_coefficient: f32, min_speed: f32) -> Self {
        Self {
            rebound_coefficient: rebound_coefficient.clamp(0.0, 1.0),
            min_speed: min_speed.max(0.0),
            last_rebound_speed: 0.0,
            just_rebounded: false,
            enabled: true,
        }
    }

    /// Call when the entity makes a surface contact with the given impact
    /// `speed`. If `speed ≥ min_speed` and enabled, triggers a rebound:
    /// sets `just_rebounded`, stores the computed rebound speed in
    /// `last_rebound_speed`, and returns `true`. Returns `false` otherwise.
    pub fn check_impact(&mut self, speed: f32) -> bool {
        if !self.enabled || speed < self.min_speed {
            return false;
        }
        self.last_rebound_speed = speed * self.rebound_coefficient;
        self.just_rebounded = true;
        true
    }

    /// Compute the rebound speed for a given impact speed without triggering
    /// state changes. Returns `0.0` for negative speeds.
    pub fn rebound_speed(&self, impact_speed: f32) -> f32 {
        if impact_speed < 0.0 {
            return 0.0;
        }
        impact_speed * self.rebound_coefficient
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_rebounded = false;
    }
}

impl Default for Rebound {
    fn default() -> Self {
        Self::new(0.5, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_impact_triggers_above_threshold() {
        let mut r = Rebound::new(0.5, 2.0);
        assert!(r.check_impact(4.0));
        assert!(r.just_rebounded);
        assert!((r.last_rebound_speed - 2.0).abs() < 1e-5);
    }

    #[test]
    fn check_impact_no_trigger_below_threshold() {
        let mut r = Rebound::new(0.5, 2.0);
        assert!(!r.check_impact(1.5));
        assert!(!r.just_rebounded);
        assert_eq!(r.last_rebound_speed, 0.0);
    }

    #[test]
    fn check_impact_triggers_at_exact_threshold() {
        let mut r = Rebound::new(0.5, 2.0);
        assert!(r.check_impact(2.0));
        assert!(r.just_rebounded);
    }

    #[test]
    fn rebound_speed_calculation() {
        let r = Rebound::new(0.6, 2.0);
        assert!((r.rebound_speed(10.0) - 6.0).abs() < 1e-5);
    }

    #[test]
    fn rebound_speed_zero_for_negative() {
        let r = Rebound::new(0.5, 2.0);
        assert_eq!(r.rebound_speed(-3.0), 0.0);
    }

    #[test]
    fn tick_clears_just_rebounded() {
        let mut r = Rebound::new(0.5, 2.0);
        r.check_impact(5.0);
        r.tick();
        assert!(!r.just_rebounded);
    }

    #[test]
    fn last_rebound_speed_persists_after_tick() {
        let mut r = Rebound::new(0.5, 2.0);
        r.check_impact(6.0);
        r.tick();
        assert!((r.last_rebound_speed - 3.0).abs() < 1e-5);
    }

    #[test]
    fn zero_coefficient_no_bounce_speed() {
        let mut r = Rebound::new(0.0, 0.0);
        r.check_impact(10.0);
        assert!(r.just_rebounded); // still triggers
        assert_eq!(r.last_rebound_speed, 0.0);
    }

    #[test]
    fn full_coefficient_full_bounce() {
        let mut r = Rebound::new(1.0, 0.0);
        r.check_impact(8.0);
        assert!((r.last_rebound_speed - 8.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_check_impact_no_trigger() {
        let mut r = Rebound::new(0.5, 2.0);
        r.enabled = false;
        assert!(!r.check_impact(10.0));
        assert!(!r.just_rebounded);
    }

    #[test]
    fn disabled_rebound_speed_still_computes() {
        let mut r = Rebound::new(0.5, 2.0);
        r.enabled = false;
        // rebound_speed is a pure computation, not gated by enabled
        assert!((r.rebound_speed(10.0) - 5.0).abs() < 1e-5);
    }
}
