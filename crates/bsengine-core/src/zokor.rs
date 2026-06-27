use bevy_ecs::prelude::Component;

/// Burrowing-excavation tracker. `burrow` builds via `dig(amount)` and
/// deepens passively at `tunnel_rate` per second in `tick(dt)` or
/// collapses immediately via `cave_in(amount)`.
///
/// Models Asian-burrowing-rodent tunnel meters, excavation progress bars,
/// underground-lair depth gauges, mole-digging accumulation trackers,
/// earthwork fortification fill levels, mine-shaft advancement indicators,
/// dungeon-excavation progress trackers, or any mechanic where steady
/// subterranean burrowing creates an impenetrable underground redoubt.
///
/// `dig(amount)` adds burrow; fires `just_entrenched` when first reaching
/// `max_burrow`. No-op when disabled.
///
/// `cave_in(amount)` reduces burrow immediately; fires `just_collapsed`
/// when reaching 0. No-op when disabled or already collapsed.
///
/// `tick(dt)` clears both flags, then increases burrow by
/// `tunnel_rate * dt` (capped at `max_burrow`). Fires `just_entrenched`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_entrenched()` returns `burrow >= max_burrow && enabled`.
///
/// `is_collapsed()` returns `burrow == 0.0` (not gated by `enabled`).
///
/// `burrow_fraction()` returns `(burrow / max_burrow).clamp(0, 1)`.
///
/// `effective_depth(scale)` returns `scale * burrow_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 3.0)` — tunnels at 3 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zokor {
    pub burrow: f32,
    pub max_burrow: f32,
    pub tunnel_rate: f32,
    pub just_entrenched: bool,
    pub just_collapsed: bool,
    pub enabled: bool,
}

impl Zokor {
    pub fn new(max_burrow: f32, tunnel_rate: f32) -> Self {
        Self {
            burrow: 0.0,
            max_burrow: max_burrow.max(0.1),
            tunnel_rate: tunnel_rate.max(0.0),
            just_entrenched: false,
            just_collapsed: false,
            enabled: true,
        }
    }

    /// Add burrow; fires `just_entrenched` when first reaching max.
    /// No-op when disabled.
    pub fn dig(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.burrow < self.max_burrow;
        self.burrow = (self.burrow + amount).min(self.max_burrow);
        if was_below && self.burrow >= self.max_burrow {
            self.just_entrenched = true;
        }
    }

    /// Reduce burrow; fires `just_collapsed` when reaching 0.
    /// No-op when disabled or already collapsed.
    pub fn cave_in(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.burrow <= 0.0 {
            return;
        }
        self.burrow = (self.burrow - amount).max(0.0);
        if self.burrow <= 0.0 {
            self.just_collapsed = true;
        }
    }

    /// Clear flags, then increase burrow by `tunnel_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_entrenched = false;
        self.just_collapsed = false;
        if self.enabled && self.tunnel_rate > 0.0 && self.burrow < self.max_burrow {
            let was_below = self.burrow < self.max_burrow;
            self.burrow = (self.burrow + self.tunnel_rate * dt).min(self.max_burrow);
            if was_below && self.burrow >= self.max_burrow {
                self.just_entrenched = true;
            }
        }
    }

    /// `true` when burrow is at maximum and component is enabled.
    pub fn is_entrenched(&self) -> bool {
        self.burrow >= self.max_burrow && self.enabled
    }

    /// `true` when burrow is 0 (not gated by `enabled`).
    pub fn is_collapsed(&self) -> bool {
        self.burrow == 0.0
    }

    /// Fraction of maximum burrow [0.0, 1.0].
    pub fn burrow_fraction(&self) -> f32 {
        (self.burrow / self.max_burrow).clamp(0.0, 1.0)
    }

    /// Returns `scale * burrow_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_depth(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.burrow_fraction()
    }
}

impl Default for Zokor {
    fn default() -> Self {
        Self::new(100.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zokor {
        Zokor::new(100.0, 3.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_collapsed() {
        let z = z();
        assert_eq!(z.burrow, 0.0);
        assert!(z.is_collapsed());
        assert!(!z.is_entrenched());
    }

    #[test]
    fn new_clamps_max_burrow() {
        let z = Zokor::new(-5.0, 3.0);
        assert!((z.max_burrow - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_tunnel_rate() {
        let z = Zokor::new(100.0, -3.0);
        assert_eq!(z.tunnel_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zokor::default();
        assert!((z.max_burrow - 100.0).abs() < 1e-5);
        assert!((z.tunnel_rate - 3.0).abs() < 1e-5);
    }

    // --- dig ---

    #[test]
    fn dig_adds_burrow() {
        let mut z = z();
        z.dig(40.0);
        assert!((z.burrow - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dig_clamps_at_max() {
        let mut z = z();
        z.dig(200.0);
        assert!((z.burrow - 100.0).abs() < 1e-3);
    }

    #[test]
    fn dig_fires_just_entrenched_at_max() {
        let mut z = z();
        z.dig(100.0);
        assert!(z.just_entrenched);
        assert!(z.is_entrenched());
    }

    #[test]
    fn dig_no_just_entrenched_when_already_at_max() {
        let mut z = z();
        z.burrow = 100.0;
        z.dig(10.0);
        assert!(!z.just_entrenched);
    }

    #[test]
    fn dig_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.dig(50.0);
        assert_eq!(z.burrow, 0.0);
    }

    #[test]
    fn dig_no_op_when_amount_zero() {
        let mut z = z();
        z.dig(0.0);
        assert_eq!(z.burrow, 0.0);
    }

    // --- cave_in ---

    #[test]
    fn cave_in_reduces_burrow() {
        let mut z = z();
        z.burrow = 60.0;
        z.cave_in(20.0);
        assert!((z.burrow - 40.0).abs() < 1e-3);
    }

    #[test]
    fn cave_in_clamps_at_zero() {
        let mut z = z();
        z.burrow = 30.0;
        z.cave_in(200.0);
        assert_eq!(z.burrow, 0.0);
    }

    #[test]
    fn cave_in_fires_just_collapsed_at_zero() {
        let mut z = z();
        z.burrow = 30.0;
        z.cave_in(30.0);
        assert!(z.just_collapsed);
    }

    #[test]
    fn cave_in_no_op_when_already_collapsed() {
        let mut z = z();
        z.cave_in(10.0);
        assert!(!z.just_collapsed);
    }

    #[test]
    fn cave_in_no_op_when_disabled() {
        let mut z = z();
        z.burrow = 50.0;
        z.enabled = false;
        z.cave_in(50.0);
        assert!((z.burrow - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_deepens_burrow() {
        let mut z = z(); // rate=3
        z.tick(1.0); // 0 + 3 = 3
        assert!((z.burrow - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_entrenched_on_tunnel_to_max() {
        let mut z = Zokor::new(100.0, 200.0);
        z.burrow = 95.0;
        z.tick(1.0);
        assert!(z.just_entrenched);
        assert!(z.is_entrenched());
    }

    #[test]
    fn tick_no_tunnel_when_already_entrenched() {
        let mut z = z();
        z.burrow = 100.0;
        z.tick(1.0);
        assert!(!z.just_entrenched);
    }

    #[test]
    fn tick_no_tunnel_when_rate_zero() {
        let mut z = Zokor::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.burrow, 0.0);
    }

    #[test]
    fn tick_no_tunnel_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.burrow, 0.0);
    }

    #[test]
    fn tick_clears_just_entrenched() {
        let mut z = Zokor::new(100.0, 200.0);
        z.burrow = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_entrenched);
    }

    #[test]
    fn tick_clears_just_collapsed() {
        let mut z = z();
        z.burrow = 10.0;
        z.cave_in(10.0);
        z.tick(0.016);
        assert!(!z.just_collapsed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=3
        z.tick(3.0); // 3*3 = 9
        assert!((z.burrow - 9.0).abs() < 1e-3);
    }

    // --- is_entrenched / is_collapsed ---

    #[test]
    fn is_entrenched_false_when_disabled() {
        let mut z = z();
        z.burrow = 100.0;
        z.enabled = false;
        assert!(!z.is_entrenched());
    }

    #[test]
    fn is_collapsed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_collapsed());
    }

    // --- burrow_fraction / effective_depth ---

    #[test]
    fn burrow_fraction_zero_when_collapsed() {
        assert_eq!(z().burrow_fraction(), 0.0);
    }

    #[test]
    fn burrow_fraction_half_at_midpoint() {
        let mut z = z();
        z.burrow = 50.0;
        assert!((z.burrow_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_depth_zero_when_collapsed() {
        assert_eq!(z().effective_depth(100.0), 0.0);
    }

    #[test]
    fn effective_depth_scales_with_burrow() {
        let mut z = z();
        z.burrow = 70.0;
        assert!((z.effective_depth(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_depth_zero_when_disabled() {
        let mut z = z();
        z.burrow = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_depth(100.0), 0.0);
    }
}
