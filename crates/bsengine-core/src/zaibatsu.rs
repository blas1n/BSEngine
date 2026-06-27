use bevy_ecs::prelude::Component;

/// Corporate-conglomerate reach tracker. `reach` builds via `acquire(amount)`
/// and expands passively at `consolidate_rate` per second in `tick(dt)` or
/// is divested immediately via `divest(amount)`.
///
/// Models family-controlled industrial-empire saturation bars, keiretsu-
/// network expansion gauges, cross-holding corporate-reach fill levels,
/// zaibatsu-style vertical-integration progress trackers, market-dominance
/// accumulation meters, monopoly-power saturation indicators, corporate-
/// group influence build-up bars, industrial-conglomerate footprint gauges,
/// or any mechanic where patient vertical integration and strategic cross-
/// shareholding quietly absorbs supplier, distributor, bank, and insurer
/// alike until the whole economy runs through a single family's holding
/// company — right up until regulators or reformers begin to unwind it.
///
/// `acquire(amount)` adds reach; fires `just_dominant` when first
/// reaching `max_reach`. No-op when disabled.
///
/// `divest(amount)` reduces reach immediately; fires `just_dissolved`
/// when reaching 0. No-op when disabled or already dissolved.
///
/// `tick(dt)` clears both flags, then increases reach by
/// `consolidate_rate * dt` (capped at `max_reach`). Fires `just_dominant`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_dominant()` returns `reach >= max_reach && enabled`.
///
/// `is_dissolved()` returns `reach == 0.0` (not gated by `enabled`).
///
/// `reach_fraction()` returns `(reach / max_reach).clamp(0, 1)`.
///
/// `effective_leverage(scale)` returns `scale * reach_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — consolidates at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zaibatsu {
    pub reach: f32,
    pub max_reach: f32,
    pub consolidate_rate: f32,
    pub just_dominant: bool,
    pub just_dissolved: bool,
    pub enabled: bool,
}

impl Zaibatsu {
    pub fn new(max_reach: f32, consolidate_rate: f32) -> Self {
        Self {
            reach: 0.0,
            max_reach: max_reach.max(0.1),
            consolidate_rate: consolidate_rate.max(0.0),
            just_dominant: false,
            just_dissolved: false,
            enabled: true,
        }
    }

    /// Add reach; fires `just_dominant` when first reaching max.
    /// No-op when disabled.
    pub fn acquire(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.reach < self.max_reach;
        self.reach = (self.reach + amount).min(self.max_reach);
        if was_below && self.reach >= self.max_reach {
            self.just_dominant = true;
        }
    }

    /// Reduce reach; fires `just_dissolved` when reaching 0.
    /// No-op when disabled or already dissolved.
    pub fn divest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.reach <= 0.0 {
            return;
        }
        self.reach = (self.reach - amount).max(0.0);
        if self.reach <= 0.0 {
            self.just_dissolved = true;
        }
    }

    /// Clear flags, then increase reach by `consolidate_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_dominant = false;
        self.just_dissolved = false;
        if self.enabled && self.consolidate_rate > 0.0 && self.reach < self.max_reach {
            let was_below = self.reach < self.max_reach;
            self.reach = (self.reach + self.consolidate_rate * dt).min(self.max_reach);
            if was_below && self.reach >= self.max_reach {
                self.just_dominant = true;
            }
        }
    }

    /// `true` when reach is at maximum and component is enabled.
    pub fn is_dominant(&self) -> bool {
        self.reach >= self.max_reach && self.enabled
    }

    /// `true` when reach is 0 (not gated by `enabled`).
    pub fn is_dissolved(&self) -> bool {
        self.reach == 0.0
    }

    /// Fraction of maximum reach [0.0, 1.0].
    pub fn reach_fraction(&self) -> f32 {
        (self.reach / self.max_reach).clamp(0.0, 1.0)
    }

    /// Returns `scale * reach_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_leverage(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.reach_fraction()
    }
}

impl Default for Zaibatsu {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zaibatsu {
        Zaibatsu::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dissolved() {
        let z = z();
        assert_eq!(z.reach, 0.0);
        assert!(z.is_dissolved());
        assert!(!z.is_dominant());
    }

    #[test]
    fn new_clamps_max_reach() {
        let z = Zaibatsu::new(-5.0, 1.5);
        assert!((z.max_reach - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_consolidate_rate() {
        let z = Zaibatsu::new(100.0, -1.5);
        assert_eq!(z.consolidate_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zaibatsu::default();
        assert!((z.max_reach - 100.0).abs() < 1e-5);
        assert!((z.consolidate_rate - 1.5).abs() < 1e-5);
    }

    // --- acquire ---

    #[test]
    fn acquire_adds_reach() {
        let mut z = z();
        z.acquire(40.0);
        assert!((z.reach - 40.0).abs() < 1e-3);
    }

    #[test]
    fn acquire_clamps_at_max() {
        let mut z = z();
        z.acquire(200.0);
        assert!((z.reach - 100.0).abs() < 1e-3);
    }

    #[test]
    fn acquire_fires_just_dominant_at_max() {
        let mut z = z();
        z.acquire(100.0);
        assert!(z.just_dominant);
        assert!(z.is_dominant());
    }

    #[test]
    fn acquire_no_just_dominant_when_already_at_max() {
        let mut z = z();
        z.reach = 100.0;
        z.acquire(10.0);
        assert!(!z.just_dominant);
    }

    #[test]
    fn acquire_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.acquire(50.0);
        assert_eq!(z.reach, 0.0);
    }

    #[test]
    fn acquire_no_op_when_amount_zero() {
        let mut z = z();
        z.acquire(0.0);
        assert_eq!(z.reach, 0.0);
    }

    // --- divest ---

    #[test]
    fn divest_reduces_reach() {
        let mut z = z();
        z.reach = 60.0;
        z.divest(20.0);
        assert!((z.reach - 40.0).abs() < 1e-3);
    }

    #[test]
    fn divest_clamps_at_zero() {
        let mut z = z();
        z.reach = 30.0;
        z.divest(200.0);
        assert_eq!(z.reach, 0.0);
    }

    #[test]
    fn divest_fires_just_dissolved_at_zero() {
        let mut z = z();
        z.reach = 30.0;
        z.divest(30.0);
        assert!(z.just_dissolved);
    }

    #[test]
    fn divest_no_op_when_already_dissolved() {
        let mut z = z();
        z.divest(10.0);
        assert!(!z.just_dissolved);
    }

    #[test]
    fn divest_no_op_when_disabled() {
        let mut z = z();
        z.reach = 50.0;
        z.enabled = false;
        z.divest(50.0);
        assert!((z.reach - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_consolidates_reach() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.reach - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_dominant_on_consolidate_to_max() {
        let mut z = Zaibatsu::new(100.0, 200.0);
        z.reach = 95.0;
        z.tick(1.0);
        assert!(z.just_dominant);
        assert!(z.is_dominant());
    }

    #[test]
    fn tick_no_consolidate_when_already_dominant() {
        let mut z = z();
        z.reach = 100.0;
        z.tick(1.0);
        assert!(!z.just_dominant);
    }

    #[test]
    fn tick_no_consolidate_when_rate_zero() {
        let mut z = Zaibatsu::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.reach, 0.0);
    }

    #[test]
    fn tick_no_consolidate_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.reach, 0.0);
    }

    #[test]
    fn tick_clears_just_dominant() {
        let mut z = Zaibatsu::new(100.0, 200.0);
        z.reach = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_dominant);
    }

    #[test]
    fn tick_clears_just_dissolved() {
        let mut z = z();
        z.reach = 10.0;
        z.divest(10.0);
        z.tick(0.016);
        assert!(!z.just_dissolved);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.reach - 9.0).abs() < 1e-3);
    }

    // --- is_dominant / is_dissolved ---

    #[test]
    fn is_dominant_false_when_disabled() {
        let mut z = z();
        z.reach = 100.0;
        z.enabled = false;
        assert!(!z.is_dominant());
    }

    #[test]
    fn is_dissolved_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_dissolved());
    }

    // --- reach_fraction / effective_leverage ---

    #[test]
    fn reach_fraction_zero_when_dissolved() {
        assert_eq!(z().reach_fraction(), 0.0);
    }

    #[test]
    fn reach_fraction_half_at_midpoint() {
        let mut z = z();
        z.reach = 50.0;
        assert!((z.reach_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_leverage_zero_when_dissolved() {
        assert_eq!(z().effective_leverage(100.0), 0.0);
    }

    #[test]
    fn effective_leverage_scales_with_reach() {
        let mut z = z();
        z.reach = 75.0;
        assert!((z.effective_leverage(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_leverage_zero_when_disabled() {
        let mut z = z();
        z.reach = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_leverage(100.0), 0.0);
    }
}
