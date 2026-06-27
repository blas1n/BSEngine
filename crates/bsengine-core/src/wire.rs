use bevy_ecs::prelude::Component;

/// Manual connection counter. Models the number of active wire-links this
/// entity maintains to others — power feeds, data channels, tether lines.
///
/// Unlike timer-based stack accumulators (`Yelp`), Wire has **no time-based
/// decay**. Connections persist until explicitly severed. All state changes
/// go through `connect()` and `sever()`.
///
/// `connect()` adds one connection (up to `max_wires`) and fires
/// `just_connected`. No-op when at cap or disabled.
///
/// `sever()` removes one connection (down to 0) and fires `just_severed`.
/// No-op when already at 0.
///
/// `sever_all()` immediately removes all connections and fires `just_severed`.
/// No-op when already at 0.
///
/// `tick(dt)` clears both one-frame flags. No other state changes; Wire
/// carries no time-based logic.
///
/// `is_networked()` returns `wire_count > 0 && enabled`.
///
/// `is_full()` returns `wire_count >= max_wires && enabled`.
///
/// `wire_fraction()` returns `(wire_count as f32 / max_wires as f32).clamp(0.0, 1.0)`.
///
/// `effective_network(base)` returns `base * (1.0 + wire_fraction())` when
/// enabled — network bonus scales with active connections; returns `base`
/// unchanged when disabled.
///
/// Distinct from `Tether` (spatial constraint), `Joint` (physics link), and
/// `Yelp` (timer-decaying stacks): Wire models **persistent logical links** —
/// nothing removes them except an explicit `sever()` call.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wire {
    /// Number of active connections [0, max_wires].
    pub wire_count: u32,
    /// Maximum simultaneous connections. Clamped >= 1.
    pub max_wires: u32,
    pub just_connected: bool,
    pub just_severed: bool,
    pub enabled: bool,
}

impl Wire {
    pub fn new(max_wires: u32) -> Self {
        Self {
            wire_count: 0,
            max_wires: max_wires.max(1),
            just_connected: false,
            just_severed: false,
            enabled: true,
        }
    }

    /// Add one connection. No-op when at cap or disabled. Fires
    /// `just_connected`.
    pub fn connect(&mut self) {
        if !self.enabled || self.wire_count >= self.max_wires {
            return;
        }
        self.wire_count += 1;
        self.just_connected = true;
    }

    /// Remove one connection. No-op when at 0. Fires `just_severed`.
    pub fn sever(&mut self) {
        if self.wire_count == 0 {
            return;
        }
        self.wire_count -= 1;
        self.just_severed = true;
    }

    /// Remove all connections. No-op when already at 0. Fires `just_severed`.
    pub fn sever_all(&mut self) {
        if self.wire_count == 0 {
            return;
        }
        self.wire_count = 0;
        self.just_severed = true;
    }

    /// Advance one frame: clear one-frame flags. No time-based state changes.
    pub fn tick(&mut self, _dt: f32) {
        self.just_connected = false;
        self.just_severed = false;
    }

    /// `true` when at least one connection is active and component is enabled.
    pub fn is_networked(&self) -> bool {
        self.wire_count > 0 && self.enabled
    }

    /// `true` when at maximum connections and component is enabled.
    pub fn is_full(&self) -> bool {
        self.wire_count >= self.max_wires && self.enabled
    }

    /// Connection count as a fraction of maximum [0.0, 1.0].
    pub fn wire_fraction(&self) -> f32 {
        (self.wire_count as f32 / self.max_wires as f32).clamp(0.0, 1.0)
    }

    /// Scale `base` by active connection count. Returns
    /// `base * (1.0 + wire_fraction())` when enabled; `base` otherwise.
    pub fn effective_network(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.wire_fraction())
    }
}

impl Default for Wire {
    fn default() -> Self {
        Self::new(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wire {
        Wire::new(4)
    }

    #[test]
    fn new_starts_empty() {
        let w = w();
        assert_eq!(w.wire_count, 0);
        assert!(!w.just_connected);
        assert!(!w.just_severed);
        assert!(!w.is_networked());
        assert!(!w.is_full());
    }

    #[test]
    fn connect_increments_count() {
        let mut w = w();
        w.connect();
        assert_eq!(w.wire_count, 1);
    }

    #[test]
    fn connect_fires_just_connected() {
        let mut w = w();
        w.connect();
        assert!(w.just_connected);
    }

    #[test]
    fn connect_no_op_at_cap() {
        let mut w = w(); // max=4
        for _ in 0..4 {
            w.connect();
        }
        w.just_connected = false;
        w.connect(); // at cap, no-op
        assert!(!w.just_connected);
        assert_eq!(w.wire_count, 4);
    }

    #[test]
    fn connect_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.connect();
        assert_eq!(w.wire_count, 0);
        assert!(!w.just_connected);
    }

    #[test]
    fn sever_decrements_count() {
        let mut w = w();
        w.connect();
        w.connect();
        w.sever();
        assert_eq!(w.wire_count, 1);
    }

    #[test]
    fn sever_fires_just_severed() {
        let mut w = w();
        w.connect();
        w.sever();
        assert!(w.just_severed);
    }

    #[test]
    fn sever_no_op_at_zero() {
        let mut w = w();
        w.sever(); // already 0
        assert!(!w.just_severed);
    }

    #[test]
    fn sever_all_resets_count() {
        let mut w = w();
        for _ in 0..3 {
            w.connect();
        }
        w.sever_all();
        assert_eq!(w.wire_count, 0);
    }

    #[test]
    fn sever_all_fires_just_severed() {
        let mut w = w();
        w.connect();
        w.sever_all();
        assert!(w.just_severed);
    }

    #[test]
    fn sever_all_no_op_at_zero() {
        let mut w = w();
        w.sever_all();
        assert!(!w.just_severed);
    }

    #[test]
    fn tick_clears_just_connected() {
        let mut w = w();
        w.connect();
        w.tick(0.016);
        assert!(!w.just_connected);
    }

    #[test]
    fn tick_clears_just_severed() {
        let mut w = w();
        w.connect();
        w.sever();
        w.tick(0.016);
        assert!(!w.just_severed);
    }

    #[test]
    fn tick_does_not_change_count() {
        let mut w = w();
        w.connect();
        w.connect();
        w.tick(100.0); // no time-based decay
        assert_eq!(w.wire_count, 2);
    }

    #[test]
    fn is_networked_true_with_connections() {
        let mut w = w();
        w.connect();
        assert!(w.is_networked());
    }

    #[test]
    fn is_networked_false_when_empty() {
        let w = w();
        assert!(!w.is_networked());
    }

    #[test]
    fn is_networked_false_when_disabled() {
        let mut w = w();
        w.connect();
        w.enabled = false;
        assert!(!w.is_networked());
    }

    #[test]
    fn is_full_true_at_cap() {
        let mut w = w(); // max=4
        for _ in 0..4 {
            w.connect();
        }
        assert!(w.is_full());
    }

    #[test]
    fn is_full_false_below_cap() {
        let mut w = w();
        w.connect();
        assert!(!w.is_full());
    }

    #[test]
    fn is_full_false_when_disabled() {
        let mut w = w();
        for _ in 0..4 {
            w.connect();
        }
        w.enabled = false;
        assert!(!w.is_full());
    }

    #[test]
    fn wire_fraction_zero_when_empty() {
        let w = w();
        assert_eq!(w.wire_fraction(), 0.0);
    }

    #[test]
    fn wire_fraction_half_at_midpoint() {
        let mut w = Wire::new(4); // max=4
        w.connect();
        w.connect(); // 2/4 = 0.5
        assert!((w.wire_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn wire_fraction_one_at_cap() {
        let mut w = w();
        for _ in 0..4 {
            w.connect();
        }
        assert!((w.wire_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_network_base_when_empty() {
        let w = w();
        assert!((w.effective_network(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_network_scaled_at_half_connections() {
        let mut w = Wire::new(4);
        w.connect();
        w.connect(); // fraction = 0.5
                     // 100 * (1 + 0.5) = 150
        assert!((w.effective_network(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_network_doubled_at_cap() {
        let mut w = w();
        for _ in 0..4 {
            w.connect();
        }
        // 100 * (1 + 1.0) = 200
        assert!((w.effective_network(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_network_passthrough_when_disabled() {
        let mut w = w();
        w.connect();
        w.enabled = false;
        assert!((w.effective_network(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_wires_clamped_to_one() {
        let w = Wire::new(0);
        assert_eq!(w.max_wires, 1);
    }

    #[test]
    fn connect_sever_cycle_single() {
        let mut w = w();
        w.connect();
        w.sever();
        assert_eq!(w.wire_count, 0);
        w.connect();
        assert_eq!(w.wire_count, 1);
        assert!(w.just_connected);
    }

    #[test]
    fn sever_all_after_full() {
        let mut w = w();
        for _ in 0..4 {
            w.connect();
        }
        w.sever_all();
        assert_eq!(w.wire_count, 0);
        assert!(!w.is_full());
    }

    #[test]
    fn multiple_connect_sever_sequence() {
        let mut w = Wire::new(3);
        w.connect(); // 1
        w.connect(); // 2
        w.sever(); // 1
        w.connect(); // 2
        w.connect(); // 3
        assert_eq!(w.wire_count, 3);
        assert!(w.is_full());
    }
}
