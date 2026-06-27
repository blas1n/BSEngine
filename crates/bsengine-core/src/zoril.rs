use bevy_ecs::prelude::Component;

/// Stench-defense tracker. `stench` builds via `spray(amount)` and
/// intensifies passively at `waft_rate` per second in `tick(dt)` or
/// disperses immediately via `disperse(amount)`.
///
/// Models African-striped-polecat spray meters, skunk-defense fill levels,
/// chemical-deterrent charge bars, noxious-cloud accumulation trackers,
/// area-denial stench gauges, odor-weapon intensity indicators, repellent-
/// saturation meters, or any mechanic where a creature's chemical defense
/// builds to an overwhelming, enemy-repelling threshold.
///
/// `spray(amount)` adds stench; fires `just_noxious` when first reaching
/// `max_stench`. No-op when disabled.
///
/// `disperse(amount)` reduces stench immediately; fires `just_fresh` when
/// reaching 0. No-op when disabled or already fresh.
///
/// `tick(dt)` clears both flags, then increases stench by
/// `waft_rate * dt` (capped at `max_stench`). Fires `just_noxious`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_noxious()` returns `stench >= max_stench && enabled`.
///
/// `is_fresh()` returns `stench == 0.0` (not gated by `enabled`).
///
/// `stench_fraction()` returns `(stench / max_stench).clamp(0, 1)`.
///
/// `effective_reek(scale)` returns `scale * stench_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — wafts at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoril {
    pub stench: f32,
    pub max_stench: f32,
    pub waft_rate: f32,
    pub just_noxious: bool,
    pub just_fresh: bool,
    pub enabled: bool,
}

impl Zoril {
    pub fn new(max_stench: f32, waft_rate: f32) -> Self {
        Self {
            stench: 0.0,
            max_stench: max_stench.max(0.1),
            waft_rate: waft_rate.max(0.0),
            just_noxious: false,
            just_fresh: false,
            enabled: true,
        }
    }

    /// Add stench; fires `just_noxious` when first reaching max.
    /// No-op when disabled.
    pub fn spray(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.stench < self.max_stench;
        self.stench = (self.stench + amount).min(self.max_stench);
        if was_below && self.stench >= self.max_stench {
            self.just_noxious = true;
        }
    }

    /// Reduce stench; fires `just_fresh` when reaching 0.
    /// No-op when disabled or already fresh.
    pub fn disperse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.stench <= 0.0 {
            return;
        }
        self.stench = (self.stench - amount).max(0.0);
        if self.stench <= 0.0 {
            self.just_fresh = true;
        }
    }

    /// Clear flags, then increase stench by `waft_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_noxious = false;
        self.just_fresh = false;
        if self.enabled && self.waft_rate > 0.0 && self.stench < self.max_stench {
            let was_below = self.stench < self.max_stench;
            self.stench = (self.stench + self.waft_rate * dt).min(self.max_stench);
            if was_below && self.stench >= self.max_stench {
                self.just_noxious = true;
            }
        }
    }

    /// `true` when stench is at maximum and component is enabled.
    pub fn is_noxious(&self) -> bool {
        self.stench >= self.max_stench && self.enabled
    }

    /// `true` when stench is 0 (not gated by `enabled`).
    pub fn is_fresh(&self) -> bool {
        self.stench == 0.0
    }

    /// Fraction of maximum stench [0.0, 1.0].
    pub fn stench_fraction(&self) -> f32 {
        (self.stench / self.max_stench).clamp(0.0, 1.0)
    }

    /// Returns `scale * stench_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_reek(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.stench_fraction()
    }
}

impl Default for Zoril {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoril {
        Zoril::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_fresh() {
        let z = z();
        assert_eq!(z.stench, 0.0);
        assert!(z.is_fresh());
        assert!(!z.is_noxious());
    }

    #[test]
    fn new_clamps_max_stench() {
        let z = Zoril::new(-5.0, 2.0);
        assert!((z.max_stench - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_waft_rate() {
        let z = Zoril::new(100.0, -3.0);
        assert_eq!(z.waft_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoril::default();
        assert!((z.max_stench - 100.0).abs() < 1e-5);
        assert!((z.waft_rate - 2.0).abs() < 1e-5);
    }

    // --- spray ---

    #[test]
    fn spray_adds_stench() {
        let mut z = z();
        z.spray(40.0);
        assert!((z.stench - 40.0).abs() < 1e-3);
    }

    #[test]
    fn spray_clamps_at_max() {
        let mut z = z();
        z.spray(200.0);
        assert!((z.stench - 100.0).abs() < 1e-3);
    }

    #[test]
    fn spray_fires_just_noxious_at_max() {
        let mut z = z();
        z.spray(100.0);
        assert!(z.just_noxious);
        assert!(z.is_noxious());
    }

    #[test]
    fn spray_no_just_noxious_when_already_at_max() {
        let mut z = z();
        z.stench = 100.0;
        z.spray(10.0);
        assert!(!z.just_noxious);
    }

    #[test]
    fn spray_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.spray(50.0);
        assert_eq!(z.stench, 0.0);
    }

    #[test]
    fn spray_no_op_when_amount_zero() {
        let mut z = z();
        z.spray(0.0);
        assert_eq!(z.stench, 0.0);
    }

    // --- disperse ---

    #[test]
    fn disperse_reduces_stench() {
        let mut z = z();
        z.stench = 60.0;
        z.disperse(20.0);
        assert!((z.stench - 40.0).abs() < 1e-3);
    }

    #[test]
    fn disperse_clamps_at_zero() {
        let mut z = z();
        z.stench = 30.0;
        z.disperse(200.0);
        assert_eq!(z.stench, 0.0);
    }

    #[test]
    fn disperse_fires_just_fresh_at_zero() {
        let mut z = z();
        z.stench = 30.0;
        z.disperse(30.0);
        assert!(z.just_fresh);
    }

    #[test]
    fn disperse_no_op_when_already_fresh() {
        let mut z = z();
        z.disperse(10.0);
        assert!(!z.just_fresh);
    }

    #[test]
    fn disperse_no_op_when_disabled() {
        let mut z = z();
        z.stench = 50.0;
        z.enabled = false;
        z.disperse(50.0);
        assert!((z.stench - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_wafts_stench() {
        let mut z = z(); // rate=2
        z.tick(1.0); // 0 + 2 = 2
        assert!((z.stench - 2.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_noxious_on_waft_to_max() {
        let mut z = Zoril::new(100.0, 200.0);
        z.stench = 95.0;
        z.tick(1.0);
        assert!(z.just_noxious);
        assert!(z.is_noxious());
    }

    #[test]
    fn tick_no_waft_when_already_noxious() {
        let mut z = z();
        z.stench = 100.0;
        z.tick(1.0);
        assert!(!z.just_noxious);
    }

    #[test]
    fn tick_no_waft_when_rate_zero() {
        let mut z = Zoril::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.stench, 0.0);
    }

    #[test]
    fn tick_no_waft_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.stench, 0.0);
    }

    #[test]
    fn tick_clears_just_noxious() {
        let mut z = Zoril::new(100.0, 200.0);
        z.stench = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_noxious);
    }

    #[test]
    fn tick_clears_just_fresh() {
        let mut z = z();
        z.stench = 10.0;
        z.disperse(10.0);
        z.tick(0.016);
        assert!(!z.just_fresh);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 2*3 = 6
        assert!((z.stench - 6.0).abs() < 1e-3);
    }

    // --- is_noxious / is_fresh ---

    #[test]
    fn is_noxious_false_when_disabled() {
        let mut z = z();
        z.stench = 100.0;
        z.enabled = false;
        assert!(!z.is_noxious());
    }

    #[test]
    fn is_fresh_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_fresh());
    }

    // --- stench_fraction / effective_reek ---

    #[test]
    fn stench_fraction_zero_when_fresh() {
        assert_eq!(z().stench_fraction(), 0.0);
    }

    #[test]
    fn stench_fraction_half_at_midpoint() {
        let mut z = z();
        z.stench = 50.0;
        assert!((z.stench_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_reek_zero_when_fresh() {
        assert_eq!(z().effective_reek(100.0), 0.0);
    }

    #[test]
    fn effective_reek_scales_with_stench() {
        let mut z = z();
        z.stench = 55.0;
        assert!((z.effective_reek(100.0) - 55.0).abs() < 1e-3);
    }

    #[test]
    fn effective_reek_zero_when_disabled() {
        let mut z = z();
        z.stench = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_reek(100.0), 0.0);
    }
}
