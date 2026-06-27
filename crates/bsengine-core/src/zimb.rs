use bevy_ecs::prelude::Component;

/// Insect-swarm intensity tracker. `swarm` builds via `hatch(amount)`
/// and intensifies passively at `swarm_rate` per second in `tick(dt)` or
/// disperses immediately via `disperse(amount)`.
///
/// Models African-gadfly swarm meters, livestock-pest-pressure fill levels,
/// biting-insect attack-intensity accumulators, summer-plague density
/// gauges, cattle-torment escalation trackers, tsetse-like-fly cloud
/// build-up indicators, mosquito-swarm saturation bars, farmstead-pest
/// infestation fill levels, or any mechanic where a relentless cloud of
/// biting insects builds to a maddening swarm that drives livestock to
/// frenzied flight across the savanna.
///
/// `hatch(amount)` adds swarm; fires `just_swarming` when first
/// reaching `max_swarm`. No-op when disabled.
///
/// `disperse(amount)` reduces swarm immediately; fires `just_dispersed`
/// when reaching 0. No-op when disabled or already dispersed.
///
/// `tick(dt)` clears both flags, then increases swarm by
/// `swarm_rate * dt` (capped at `max_swarm`). Fires `just_swarming`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_swarming()` returns `swarm >= max_swarm && enabled`.
///
/// `is_dispersed()` returns `swarm == 0.0` (not gated by `enabled`).
///
/// `swarm_fraction()` returns `(swarm / max_swarm).clamp(0, 1)`.
///
/// `effective_torment(scale)` returns `scale * swarm_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 3.0)` — hatches at 3 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zimb {
    pub swarm: f32,
    pub max_swarm: f32,
    pub swarm_rate: f32,
    pub just_swarming: bool,
    pub just_dispersed: bool,
    pub enabled: bool,
}

impl Zimb {
    pub fn new(max_swarm: f32, swarm_rate: f32) -> Self {
        Self {
            swarm: 0.0,
            max_swarm: max_swarm.max(0.1),
            swarm_rate: swarm_rate.max(0.0),
            just_swarming: false,
            just_dispersed: false,
            enabled: true,
        }
    }

    /// Add swarm; fires `just_swarming` when first reaching max.
    /// No-op when disabled.
    pub fn hatch(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.swarm < self.max_swarm;
        self.swarm = (self.swarm + amount).min(self.max_swarm);
        if was_below && self.swarm >= self.max_swarm {
            self.just_swarming = true;
        }
    }

    /// Reduce swarm; fires `just_dispersed` when reaching 0.
    /// No-op when disabled or already dispersed.
    pub fn disperse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.swarm <= 0.0 {
            return;
        }
        self.swarm = (self.swarm - amount).max(0.0);
        if self.swarm <= 0.0 {
            self.just_dispersed = true;
        }
    }

    /// Clear flags, then increase swarm by `swarm_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_swarming = false;
        self.just_dispersed = false;
        if self.enabled && self.swarm_rate > 0.0 && self.swarm < self.max_swarm {
            let was_below = self.swarm < self.max_swarm;
            self.swarm = (self.swarm + self.swarm_rate * dt).min(self.max_swarm);
            if was_below && self.swarm >= self.max_swarm {
                self.just_swarming = true;
            }
        }
    }

    /// `true` when swarm is at maximum and component is enabled.
    pub fn is_swarming(&self) -> bool {
        self.swarm >= self.max_swarm && self.enabled
    }

    /// `true` when swarm is 0 (not gated by `enabled`).
    pub fn is_dispersed(&self) -> bool {
        self.swarm == 0.0
    }

    /// Fraction of maximum swarm [0.0, 1.0].
    pub fn swarm_fraction(&self) -> f32 {
        (self.swarm / self.max_swarm).clamp(0.0, 1.0)
    }

    /// Returns `scale * swarm_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_torment(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.swarm_fraction()
    }
}

impl Default for Zimb {
    fn default() -> Self {
        Self::new(100.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zimb {
        Zimb::new(100.0, 3.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_dispersed() {
        let z = z();
        assert_eq!(z.swarm, 0.0);
        assert!(z.is_dispersed());
        assert!(!z.is_swarming());
    }

    #[test]
    fn new_clamps_max_swarm() {
        let z = Zimb::new(-5.0, 3.0);
        assert!((z.max_swarm - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_swarm_rate() {
        let z = Zimb::new(100.0, -3.0);
        assert_eq!(z.swarm_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zimb::default();
        assert!((z.max_swarm - 100.0).abs() < 1e-5);
        assert!((z.swarm_rate - 3.0).abs() < 1e-5);
    }

    // --- hatch ---

    #[test]
    fn hatch_adds_swarm() {
        let mut z = z();
        z.hatch(40.0);
        assert!((z.swarm - 40.0).abs() < 1e-3);
    }

    #[test]
    fn hatch_clamps_at_max() {
        let mut z = z();
        z.hatch(200.0);
        assert!((z.swarm - 100.0).abs() < 1e-3);
    }

    #[test]
    fn hatch_fires_just_swarming_at_max() {
        let mut z = z();
        z.hatch(100.0);
        assert!(z.just_swarming);
        assert!(z.is_swarming());
    }

    #[test]
    fn hatch_no_just_swarming_when_already_at_max() {
        let mut z = z();
        z.swarm = 100.0;
        z.hatch(10.0);
        assert!(!z.just_swarming);
    }

    #[test]
    fn hatch_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.hatch(50.0);
        assert_eq!(z.swarm, 0.0);
    }

    #[test]
    fn hatch_no_op_when_amount_zero() {
        let mut z = z();
        z.hatch(0.0);
        assert_eq!(z.swarm, 0.0);
    }

    // --- disperse ---

    #[test]
    fn disperse_reduces_swarm() {
        let mut z = z();
        z.swarm = 60.0;
        z.disperse(20.0);
        assert!((z.swarm - 40.0).abs() < 1e-3);
    }

    #[test]
    fn disperse_clamps_at_zero() {
        let mut z = z();
        z.swarm = 30.0;
        z.disperse(200.0);
        assert_eq!(z.swarm, 0.0);
    }

    #[test]
    fn disperse_fires_just_dispersed_at_zero() {
        let mut z = z();
        z.swarm = 30.0;
        z.disperse(30.0);
        assert!(z.just_dispersed);
    }

    #[test]
    fn disperse_no_op_when_already_dispersed() {
        let mut z = z();
        z.disperse(10.0);
        assert!(!z.just_dispersed);
    }

    #[test]
    fn disperse_no_op_when_disabled() {
        let mut z = z();
        z.swarm = 50.0;
        z.enabled = false;
        z.disperse(50.0);
        assert!((z.swarm - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_hatches_swarm() {
        let mut z = z(); // rate=3
        z.tick(1.0); // 0 + 3 = 3
        assert!((z.swarm - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_swarming_on_hatch_to_max() {
        let mut z = Zimb::new(100.0, 200.0);
        z.swarm = 95.0;
        z.tick(1.0);
        assert!(z.just_swarming);
        assert!(z.is_swarming());
    }

    #[test]
    fn tick_no_hatch_when_already_swarming() {
        let mut z = z();
        z.swarm = 100.0;
        z.tick(1.0);
        assert!(!z.just_swarming);
    }

    #[test]
    fn tick_no_hatch_when_rate_zero() {
        let mut z = Zimb::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.swarm, 0.0);
    }

    #[test]
    fn tick_no_hatch_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.swarm, 0.0);
    }

    #[test]
    fn tick_clears_just_swarming() {
        let mut z = Zimb::new(100.0, 200.0);
        z.swarm = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_swarming);
    }

    #[test]
    fn tick_clears_just_dispersed() {
        let mut z = z();
        z.swarm = 10.0;
        z.disperse(10.0);
        z.tick(0.016);
        assert!(!z.just_dispersed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=3
        z.tick(4.0); // 3*4 = 12
        assert!((z.swarm - 12.0).abs() < 1e-3);
    }

    // --- is_swarming / is_dispersed ---

    #[test]
    fn is_swarming_false_when_disabled() {
        let mut z = z();
        z.swarm = 100.0;
        z.enabled = false;
        assert!(!z.is_swarming());
    }

    #[test]
    fn is_dispersed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_dispersed());
    }

    // --- swarm_fraction / effective_torment ---

    #[test]
    fn swarm_fraction_zero_when_dispersed() {
        assert_eq!(z().swarm_fraction(), 0.0);
    }

    #[test]
    fn swarm_fraction_half_at_midpoint() {
        let mut z = z();
        z.swarm = 50.0;
        assert!((z.swarm_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_torment_zero_when_dispersed() {
        assert_eq!(z().effective_torment(100.0), 0.0);
    }

    #[test]
    fn effective_torment_scales_with_swarm() {
        let mut z = z();
        z.swarm = 75.0;
        assert!((z.effective_torment(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_torment_zero_when_disabled() {
        let mut z = z();
        z.swarm = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_torment(100.0), 0.0);
    }
}
