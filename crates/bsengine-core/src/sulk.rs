use bevy_ecs::prelude::Component;

/// Withdrawn-behaviour debuff that reduces own damage output and the
/// effectiveness of received support. While `sulking`, `sulk_depth` climbs
/// toward 1.0; once the entity stops sulking, it decays back to 0.0.
///
/// `begin_sulk()` sets `sulking = true` and fires `just_sulked` on the false
/// → true transition. No-op when already sulking or disabled.
///
/// `end_sulk()` sets `sulking = false`. The `sulk_depth` continues to decay
/// in subsequent `tick()` calls. No-op when not sulking or disabled.
///
/// `tick(dt)` clears one-frame flags at start; when sulking, increments
/// `sulk_depth` (capped at 1.0); when not sulking, decrements it (floored at
/// 0.0) and fires `just_snapped_out` when it first reaches zero. No-op when
/// disabled.
///
/// `is_sulking()` returns `sulking && enabled`.
///
/// `effective_outgoing(base)` returns `base * (1.0 - sulk_depth)` when
/// enabled, floored at 0.0 — a sulking entity deals less damage the deeper
/// the sulk.
///
/// `effective_support_received(base)` returns
/// `base * (1.0 - support_penalty * sulk_depth)` when enabled — ally heals,
/// buffs, and support effects are partially wasted on a sulking entity.
///
/// Distinct from `Morale` (army-wide numerical morale stat),
/// `Demoralize` (attack/defense debuff applied from outside),
/// `Charm` (forced allegiance flip), and
/// `Taunt` (overrides targeting): Sulk is a **self-reinforcing withdrawal** —
/// the entity wilfully pulls away from the fight; the longer it sulks the
/// deeper the hole, and even allies' support can't fully reach it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Sulk {
    /// Current withdrawal depth [0.0 = alert, 1.0 = fully withdrawn].
    pub sulk_depth: f32,
    /// Rate at which `sulk_depth` increases per second while sulking. Clamped >= 0.0.
    pub sulk_rate: f32,
    /// Rate at which `sulk_depth` decays per second after `end_sulk`. Clamped >= 0.0.
    pub recovery_rate: f32,
    /// Fraction of received support lost at full sulk depth. Clamped [0.0, 1.0].
    pub support_penalty: f32,
    pub sulking: bool,
    pub just_sulked: bool,
    pub just_snapped_out: bool,
    pub enabled: bool,
}

impl Sulk {
    pub fn new(sulk_rate: f32, recovery_rate: f32, support_penalty: f32) -> Self {
        Self {
            sulk_depth: 0.0,
            sulk_rate: sulk_rate.max(0.0),
            recovery_rate: recovery_rate.max(0.0),
            support_penalty: support_penalty.clamp(0.0, 1.0),
            sulking: false,
            just_sulked: false,
            just_snapped_out: false,
            enabled: true,
        }
    }

    /// Begin sulking. Fires `just_sulked` on the first call. No-op when
    /// already sulking or disabled.
    pub fn begin_sulk(&mut self) {
        if !self.enabled || self.sulking {
            return;
        }
        self.sulking = true;
        self.just_sulked = true;
    }

    /// Stop sulking. `sulk_depth` continues to decay each `tick`. No-op when
    /// not sulking or disabled.
    pub fn end_sulk(&mut self) {
        if !self.enabled || !self.sulking {
            return;
        }
        self.sulking = false;
    }

    /// Advance the sulk state. Clears one-frame flags first; grows or decays
    /// `sulk_depth`; fires `just_snapped_out` when depth first reaches 0.0
    /// after being positive. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_sulked = false;
        self.just_snapped_out = false;

        if !self.enabled {
            return;
        }

        if self.sulking {
            self.sulk_depth = (self.sulk_depth + self.sulk_rate * dt).min(1.0);
        } else if self.sulk_depth > 0.0 {
            let prev = self.sulk_depth;
            self.sulk_depth = (self.sulk_depth - self.recovery_rate * dt).max(0.0);
            if prev > 0.0 && self.sulk_depth == 0.0 {
                self.just_snapped_out = true;
            }
        }
    }

    /// `true` when currently sulking and the component is enabled.
    pub fn is_sulking(&self) -> bool {
        self.sulking && self.enabled
    }

    /// Outgoing damage reduced by sulk depth. Returns
    /// `base * (1.0 - sulk_depth)` when enabled, floored at 0.0. Returns
    /// `base` when disabled.
    pub fn effective_outgoing(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.sulk_depth)).max(0.0)
    }

    /// Received support (heals, buffs) partially wasted by sulk. Returns
    /// `base * (1.0 - support_penalty * sulk_depth)` when enabled, floored at
    /// 0.0. Returns `base` when disabled.
    pub fn effective_support_received(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.support_penalty * self.sulk_depth)).max(0.0)
    }
}

impl Default for Sulk {
    fn default() -> Self {
        Self::new(0.3, 0.15, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_alert() {
        let s = Sulk::new(0.3, 0.15, 0.5);
        assert!(!s.sulking);
        assert_eq!(s.sulk_depth, 0.0);
        assert!(!s.is_sulking());
    }

    #[test]
    fn begin_sulk_sets_sulking() {
        let mut s = Sulk::new(0.3, 0.15, 0.5);
        s.begin_sulk();
        assert!(s.sulking);
        assert!(s.just_sulked);
        assert!(s.is_sulking());
    }

    #[test]
    fn begin_sulk_no_op_when_already_sulking() {
        let mut s = Sulk::new(0.3, 0.15, 0.5);
        s.begin_sulk();
        s.tick(0.0);
        s.begin_sulk();
        assert!(!s.just_sulked);
    }

    #[test]
    fn begin_sulk_no_op_when_disabled() {
        let mut s = Sulk::new(0.3, 0.15, 0.5);
        s.enabled = false;
        s.begin_sulk();
        assert!(!s.sulking);
    }

    #[test]
    fn end_sulk_clears_sulking() {
        let mut s = Sulk::new(0.3, 0.15, 0.5);
        s.begin_sulk();
        s.end_sulk();
        assert!(!s.sulking);
    }

    #[test]
    fn end_sulk_no_op_when_not_sulking() {
        let mut s = Sulk::new(0.3, 0.15, 0.5);
        s.end_sulk(); // no panic
        assert!(!s.sulking);
    }

    #[test]
    fn end_sulk_no_op_when_disabled() {
        let mut s = Sulk::new(0.3, 0.15, 0.5);
        s.begin_sulk();
        s.enabled = false;
        s.end_sulk();
        assert!(s.sulking); // no-op
    }

    #[test]
    fn tick_increases_depth_while_sulking() {
        let mut s = Sulk::new(0.5, 0.15, 0.5);
        s.begin_sulk();
        s.tick(1.0);
        assert!((s.sulk_depth - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_depth_at_one() {
        let mut s = Sulk::new(1.0, 0.15, 0.5);
        s.begin_sulk();
        s.tick(10.0);
        assert!((s.sulk_depth - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_depth_after_end_sulk() {
        let mut s = Sulk::new(1.0, 0.5, 0.5);
        s.begin_sulk();
        s.tick(1.0); // depth = 1.0
        s.end_sulk();
        s.tick(1.0); // depth = 0.5
        assert!((s.sulk_depth - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_snapped_out_when_depth_reaches_zero() {
        let mut s = Sulk::new(1.0, 1.0, 0.5);
        s.begin_sulk();
        s.tick(0.5); // depth = 0.5
        s.end_sulk();
        s.tick(0.5); // depth = 0.0
        assert!(s.just_snapped_out);
    }

    #[test]
    fn tick_no_just_snapped_out_while_depth_still_positive() {
        let mut s = Sulk::new(1.0, 0.5, 0.5);
        s.begin_sulk();
        s.tick(1.0); // depth = 1.0
        s.end_sulk();
        s.tick(0.5); // depth = 0.5
        assert!(!s.just_snapped_out);
    }

    #[test]
    fn tick_no_just_snapped_out_if_depth_was_already_zero() {
        let mut s = Sulk::new(0.3, 0.5, 0.5);
        s.tick(1.0); // depth was 0 the whole time
        assert!(!s.just_snapped_out);
    }

    #[test]
    fn tick_clears_just_sulked() {
        let mut s = Sulk::new(0.3, 0.15, 0.5);
        s.begin_sulk();
        s.tick(0.016);
        assert!(!s.just_sulked);
    }

    #[test]
    fn tick_clears_just_snapped_out_next_frame() {
        let mut s = Sulk::new(1.0, 1.0, 0.5);
        s.begin_sulk();
        s.tick(1.0); // depth = 1.0
        s.end_sulk();
        s.tick(1.0); // just_snapped_out = true
        s.tick(0.016); // cleared
        assert!(!s.just_snapped_out);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut s = Sulk::new(0.5, 0.15, 0.5);
        s.begin_sulk();
        s.enabled = false;
        s.tick(10.0);
        assert_eq!(s.sulk_depth, 0.0);
    }

    #[test]
    fn tick_no_growth_when_alert_and_depth_zero() {
        let mut s = Sulk::new(0.5, 0.15, 0.5);
        s.tick(5.0);
        assert_eq!(s.sulk_depth, 0.0);
    }

    #[test]
    fn is_sulking_false_when_disabled() {
        let mut s = Sulk::new(0.3, 0.15, 0.5);
        s.sulking = true;
        s.enabled = false;
        assert!(!s.is_sulking());
    }

    #[test]
    fn effective_outgoing_reduced_by_depth() {
        let mut s = Sulk::new(1.0, 0.15, 0.5);
        s.begin_sulk();
        s.tick(1.0); // depth = 1.0
                     // 100 * (1 - 1.0) = 0
        assert!((s.effective_outgoing(100.0)).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_partial_depth() {
        let mut s = Sulk::new(0.5, 0.15, 0.5);
        s.begin_sulk();
        s.tick(1.0); // depth = 0.5
                     // 100 * (1 - 0.5) = 50
        assert!((s.effective_outgoing(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_base_when_depth_zero() {
        let s = Sulk::new(0.3, 0.15, 0.5);
        assert!((s.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_base_when_disabled() {
        let mut s = Sulk::new(0.3, 0.15, 0.5);
        s.sulk_depth = 1.0;
        s.enabled = false;
        assert!((s.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_floored_at_zero() {
        let mut s = Sulk::new(1.0, 0.0, 0.5);
        s.begin_sulk();
        s.tick(1.0);
        assert_eq!(s.effective_outgoing(100.0), 0.0);
    }

    #[test]
    fn effective_support_received_reduced_by_depth_and_penalty() {
        let mut s = Sulk::new(1.0, 0.15, 0.4);
        s.begin_sulk();
        s.tick(1.0); // depth = 1.0
                     // 100 * (1 - 0.4 * 1.0) = 60
        assert!((s.effective_support_received(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_support_received_partial_depth() {
        let mut s = Sulk::new(1.0, 0.15, 0.5);
        s.begin_sulk();
        s.tick(0.5); // depth = 0.5
                     // 100 * (1 - 0.5 * 0.5) = 75
        assert!((s.effective_support_received(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_support_received_base_when_depth_zero() {
        let s = Sulk::new(0.3, 0.15, 0.5);
        assert!((s.effective_support_received(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_support_received_base_when_disabled() {
        let mut s = Sulk::new(0.3, 0.15, 0.5);
        s.sulk_depth = 1.0;
        s.enabled = false;
        assert!((s.effective_support_received(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn sulk_rate_clamped_to_zero() {
        let s = Sulk::new(-0.5, 0.15, 0.5);
        assert_eq!(s.sulk_rate, 0.0);
    }

    #[test]
    fn recovery_rate_clamped_to_zero() {
        let s = Sulk::new(0.3, -0.5, 0.5);
        assert_eq!(s.recovery_rate, 0.0);
    }

    #[test]
    fn support_penalty_clamped_to_one() {
        let s = Sulk::new(0.3, 0.15, 2.0);
        assert!((s.support_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn support_penalty_clamped_to_zero() {
        let s = Sulk::new(0.3, 0.15, -0.5);
        assert_eq!(s.support_penalty, 0.0);
    }

    #[test]
    fn re_begin_after_snap_out_fires_just_sulked() {
        let mut s = Sulk::new(1.0, 1.0, 0.5);
        s.begin_sulk();
        s.tick(0.0); // clear flags
        s.end_sulk();
        s.tick(10.0); // fully recovered
        s.begin_sulk();
        assert!(s.just_sulked);
    }

    #[test]
    fn depth_persists_after_end_sulk_until_recovered() {
        let mut s = Sulk::new(1.0, 0.5, 0.5);
        s.begin_sulk();
        s.tick(1.0); // depth = 1.0
        s.end_sulk();
        assert!(s.sulk_depth > 0.0);
        assert!(s.effective_outgoing(100.0) < 100.0);
    }
}
