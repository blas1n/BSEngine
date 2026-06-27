use bevy_ecs::prelude::Component;

/// Slate-cutting edge-sharpness tracker. `edge` builds via
/// `hone(amount)` and sharpens passively at `whet_rate` per second
/// in `tick(dt)` or dulls immediately via `dull(amount)`.
///
/// Models roofing-slate tool-edge durability bars, stonecutter's
/// chisel-sharpness fill levels, tile-cutting tool-condition
/// trackers, hand-tool keenness saturation gauges, workshop-
/// maintenance completion meters, artisan blade-sharpness
/// indicators, fieldcraft tool-edge integrity fill levels, edge-
/// tool restoration progress bars, or any mechanic where a
/// craftsperson patiently hones a heavy-bladed implement against
/// a whetstone until the edge holds a line sharp enough to score
/// slate in a single decisive stroke — only for repeated heavy
/// chopping to roll the edge back to a rounded, ineffective bevel
/// that skips across every tile it touches.
///
/// `hone(amount)` adds edge; fires `just_keen` when first reaching
/// `max_edge`. No-op when disabled.
///
/// `dull(amount)` reduces edge immediately; fires `just_blunt`
/// when reaching 0. No-op when disabled or already blunt.
///
/// `tick(dt)` clears both flags, then increases edge by
/// `whet_rate * dt` (capped at `max_edge`). Fires `just_keen`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_keen()` returns `edge >= max_edge && enabled`.
///
/// `is_blunt()` returns `edge == 0.0` (not gated by `enabled`).
///
/// `edge_fraction()` returns `(edge / max_edge).clamp(0, 1)`.
///
/// `effective_bite(scale)` returns `scale * edge_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — whets at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zax {
    pub edge: f32,
    pub max_edge: f32,
    pub whet_rate: f32,
    pub just_keen: bool,
    pub just_blunt: bool,
    pub enabled: bool,
}

impl Zax {
    pub fn new(max_edge: f32, whet_rate: f32) -> Self {
        Self {
            edge: 0.0,
            max_edge: max_edge.max(0.1),
            whet_rate: whet_rate.max(0.0),
            just_keen: false,
            just_blunt: false,
            enabled: true,
        }
    }

    /// Add edge; fires `just_keen` when first reaching max.
    /// No-op when disabled.
    pub fn hone(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.edge < self.max_edge;
        self.edge = (self.edge + amount).min(self.max_edge);
        if was_below && self.edge >= self.max_edge {
            self.just_keen = true;
        }
    }

    /// Reduce edge; fires `just_blunt` when reaching 0.
    /// No-op when disabled or already blunt.
    pub fn dull(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.edge <= 0.0 {
            return;
        }
        self.edge = (self.edge - amount).max(0.0);
        if self.edge <= 0.0 {
            self.just_blunt = true;
        }
    }

    /// Clear flags, then increase edge by `whet_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_keen = false;
        self.just_blunt = false;
        if self.enabled && self.whet_rate > 0.0 && self.edge < self.max_edge {
            let was_below = self.edge < self.max_edge;
            self.edge = (self.edge + self.whet_rate * dt).min(self.max_edge);
            if was_below && self.edge >= self.max_edge {
                self.just_keen = true;
            }
        }
    }

    /// `true` when edge is at maximum and component is enabled.
    pub fn is_keen(&self) -> bool {
        self.edge >= self.max_edge && self.enabled
    }

    /// `true` when edge is 0 (not gated by `enabled`).
    pub fn is_blunt(&self) -> bool {
        self.edge == 0.0
    }

    /// Fraction of maximum edge [0.0, 1.0].
    pub fn edge_fraction(&self) -> f32 {
        (self.edge / self.max_edge).clamp(0.0, 1.0)
    }

    /// Returns `scale * edge_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_bite(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.edge_fraction()
    }
}

impl Default for Zax {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zax {
        Zax::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_blunt() {
        let z = z();
        assert_eq!(z.edge, 0.0);
        assert!(z.is_blunt());
        assert!(!z.is_keen());
    }

    #[test]
    fn new_clamps_max_edge() {
        let z = Zax::new(-5.0, 2.0);
        assert!((z.max_edge - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_whet_rate() {
        let z = Zax::new(100.0, -2.0);
        assert_eq!(z.whet_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zax::default();
        assert!((z.max_edge - 100.0).abs() < 1e-5);
        assert!((z.whet_rate - 2.0).abs() < 1e-5);
    }

    // --- hone ---

    #[test]
    fn hone_adds_edge() {
        let mut z = z();
        z.hone(40.0);
        assert!((z.edge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn hone_clamps_at_max() {
        let mut z = z();
        z.hone(200.0);
        assert!((z.edge - 100.0).abs() < 1e-3);
    }

    #[test]
    fn hone_fires_just_keen_at_max() {
        let mut z = z();
        z.hone(100.0);
        assert!(z.just_keen);
        assert!(z.is_keen());
    }

    #[test]
    fn hone_no_just_keen_when_already_at_max() {
        let mut z = z();
        z.edge = 100.0;
        z.hone(10.0);
        assert!(!z.just_keen);
    }

    #[test]
    fn hone_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.hone(50.0);
        assert_eq!(z.edge, 0.0);
    }

    #[test]
    fn hone_no_op_when_amount_zero() {
        let mut z = z();
        z.hone(0.0);
        assert_eq!(z.edge, 0.0);
    }

    // --- dull ---

    #[test]
    fn dull_reduces_edge() {
        let mut z = z();
        z.edge = 60.0;
        z.dull(20.0);
        assert!((z.edge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dull_clamps_at_zero() {
        let mut z = z();
        z.edge = 30.0;
        z.dull(200.0);
        assert_eq!(z.edge, 0.0);
    }

    #[test]
    fn dull_fires_just_blunt_at_zero() {
        let mut z = z();
        z.edge = 30.0;
        z.dull(30.0);
        assert!(z.just_blunt);
    }

    #[test]
    fn dull_no_op_when_already_blunt() {
        let mut z = z();
        z.dull(10.0);
        assert!(!z.just_blunt);
    }

    #[test]
    fn dull_no_op_when_disabled() {
        let mut z = z();
        z.edge = 50.0;
        z.enabled = false;
        z.dull(50.0);
        assert!((z.edge - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_whets_edge() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.edge - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_keen_on_whet_to_max() {
        let mut z = Zax::new(100.0, 200.0);
        z.edge = 95.0;
        z.tick(1.0);
        assert!(z.just_keen);
        assert!(z.is_keen());
    }

    #[test]
    fn tick_no_whet_when_already_keen() {
        let mut z = z();
        z.edge = 100.0;
        z.tick(1.0);
        assert!(!z.just_keen);
    }

    #[test]
    fn tick_no_whet_when_rate_zero() {
        let mut z = Zax::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.edge, 0.0);
    }

    #[test]
    fn tick_no_whet_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.edge, 0.0);
    }

    #[test]
    fn tick_clears_just_keen() {
        let mut z = Zax::new(100.0, 200.0);
        z.edge = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_keen);
    }

    #[test]
    fn tick_clears_just_blunt() {
        let mut z = z();
        z.edge = 10.0;
        z.dull(10.0);
        z.tick(0.016);
        assert!(!z.just_blunt);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.edge - 10.0).abs() < 1e-3);
    }

    // --- is_keen / is_blunt ---

    #[test]
    fn is_keen_false_when_disabled() {
        let mut z = z();
        z.edge = 100.0;
        z.enabled = false;
        assert!(!z.is_keen());
    }

    #[test]
    fn is_blunt_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_blunt());
    }

    // --- edge_fraction / effective_bite ---

    #[test]
    fn edge_fraction_zero_when_blunt() {
        assert_eq!(z().edge_fraction(), 0.0);
    }

    #[test]
    fn edge_fraction_half_at_midpoint() {
        let mut z = z();
        z.edge = 50.0;
        assert!((z.edge_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_bite_zero_when_blunt() {
        assert_eq!(z().effective_bite(100.0), 0.0);
    }

    #[test]
    fn effective_bite_scales_with_edge() {
        let mut z = z();
        z.edge = 75.0;
        assert!((z.effective_bite(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_bite_zero_when_disabled() {
        let mut z = z();
        z.edge = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_bite(100.0), 0.0);
    }
}
