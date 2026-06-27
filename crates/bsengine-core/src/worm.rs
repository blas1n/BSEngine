use bevy_ecs::prelude::Component;

/// Ratchet-style burrowing accumulator. Models a persistent effect that
/// tunnels deeper over time and stays at whatever depth it reached — it does
/// NOT decay passively. Only an explicit `expel()` removes it.
///
/// `burrow()` starts the worm burrowing; no-op if already burrowing or
/// disabled.
///
/// `expel()` forcibly removes the worm: sets `burrowing` to `false`, resets
/// `worm_depth` to 0, fires `just_expelled`. No-op if not burrowing.
///
/// `tick(dt)` clears both one-frame flags first. If `burrowing` and
/// `worm_depth < max_depth`: increases `worm_depth` by `burrow_rate * dt`
/// (capped at `max_depth`); fires `just_burrowed` the first time it reaches
/// the cap. No-op (beyond flag clear) when disabled.
///
/// `is_deep()` returns `worm_depth >= max_depth && enabled`.
///
/// `depth_fraction()` returns `(worm_depth / max_depth).clamp(0.0, 1.0)`.
///
/// `effective_penetration(base)` returns `base * (1.0 + depth_fraction())`
/// when enabled — penetration scales with how deep the worm has burrowed;
/// returns `base` unchanged when disabled.
///
/// Distinct from `Whelm` (decays when not active) and `Rot` (constant tick
/// damage): Worm models a **ratchet accumulator** — once burrowed, the depth
/// is locked in until explicitly expelled. The entity that hosts the worm
/// pays the cost of growing penetration until it manages to expel it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Worm {
    /// Current burrow depth [0.0, max_depth].
    pub worm_depth: f32,
    /// Maximum burrow depth. Clamped >= 1.0.
    pub max_depth: f32,
    /// Depth increase per second while burrowing. Clamped >= 0.0.
    pub burrow_rate: f32,
    pub burrowing: bool,
    pub just_burrowed: bool,
    pub just_expelled: bool,
    pub enabled: bool,
}

impl Worm {
    pub fn new(max_depth: f32, burrow_rate: f32) -> Self {
        Self {
            worm_depth: 0.0,
            max_depth: max_depth.max(1.0),
            burrow_rate: burrow_rate.max(0.0),
            burrowing: false,
            just_burrowed: false,
            just_expelled: false,
            enabled: true,
        }
    }

    /// Start burrowing. No-op if already burrowing or disabled.
    pub fn burrow(&mut self) {
        if !self.enabled || self.burrowing {
            return;
        }
        self.burrowing = true;
    }

    /// Forcibly expel the worm: stops burrowing, resets depth to 0, fires
    /// `just_expelled`. No-op if not burrowing.
    pub fn expel(&mut self) {
        if !self.burrowing {
            return;
        }
        self.burrowing = false;
        self.worm_depth = 0.0;
        self.just_expelled = true;
    }

    /// Advance one frame: clear flags, then deepen if burrowing and not yet
    /// at max. No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_burrowed = false;
        self.just_expelled = false;

        if !self.enabled {
            return;
        }

        if self.burrowing && self.worm_depth < self.max_depth {
            self.worm_depth = (self.worm_depth + self.burrow_rate * dt).min(self.max_depth);
            if self.worm_depth >= self.max_depth {
                self.just_burrowed = true;
            }
        }
    }

    /// `true` when fully burrowed and component is enabled.
    pub fn is_deep(&self) -> bool {
        self.worm_depth >= self.max_depth && self.enabled
    }

    /// Burrow depth as a fraction of maximum [0.0, 1.0].
    pub fn depth_fraction(&self) -> f32 {
        (self.worm_depth / self.max_depth).clamp(0.0, 1.0)
    }

    /// Scale `base` by burrow depth. Returns `base * (1.0 + depth_fraction())`
    /// when enabled; `base` otherwise.
    pub fn effective_penetration(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.depth_fraction())
    }
}

impl Default for Worm {
    fn default() -> Self {
        Self::new(10.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Worm {
        Worm::new(10.0, 1.5)
    }

    #[test]
    fn new_starts_idle() {
        let w = w();
        assert_eq!(w.worm_depth, 0.0);
        assert!(!w.burrowing);
        assert!(!w.just_burrowed);
        assert!(!w.just_expelled);
        assert!(!w.is_deep());
    }

    #[test]
    fn burrow_sets_burrowing() {
        let mut w = w();
        w.burrow();
        assert!(w.burrowing);
    }

    #[test]
    fn burrow_no_op_when_already_burrowing() {
        let mut w = w();
        w.burrow();
        w.tick(1.0); // 1.5
        w.burrow();
        assert!(w.burrowing);
        assert!((w.worm_depth - 1.5).abs() < 1e-4);
    }

    #[test]
    fn burrow_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.burrow();
        assert!(!w.burrowing);
    }

    #[test]
    fn tick_deepens_while_burrowing() {
        let mut w = w(); // burrow_rate=1.5
        w.burrow();
        w.tick(1.0); // 1.5
        assert!((w.worm_depth - 1.5).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max_depth() {
        let mut w = w();
        w.burrow();
        w.tick(100.0); // capped at 10
        assert!((w.worm_depth - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_deepen_without_burrow() {
        let mut w = w();
        w.tick(5.0);
        assert_eq!(w.worm_depth, 0.0);
    }

    #[test]
    fn tick_no_deepen_when_at_max() {
        let mut w = w();
        w.burrow();
        w.tick(100.0); // fully in
        let depth = w.worm_depth;
        w.tick(1.0); // no further change
        assert!((w.worm_depth - depth).abs() < 1e-5);
    }

    #[test]
    fn tick_depth_persists_when_not_burrowing() {
        let mut w = w();
        w.burrow();
        w.tick(2.0); // 3.0
        w.burrowing = false; // stop without expelling
        w.tick(10.0); // depth stays — no passive decay
        assert!((w.worm_depth - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.burrow();
        w.enabled = false;
        w.tick(5.0);
        assert_eq!(w.worm_depth, 0.0);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_burrowed = true;
        w.just_expelled = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_burrowed);
        assert!(!w.just_expelled);
    }

    #[test]
    fn just_burrowed_fires_at_max_depth() {
        let mut w = w(); // burrow_rate=1.5 → 10/1.5 ≈ 6.67s
        w.burrow();
        w.tick(7.0); // past max
        assert!(w.just_burrowed);
    }

    #[test]
    fn just_burrowed_clears_next_tick() {
        let mut w = w();
        w.burrow();
        w.tick(7.0); // deep
        w.tick(0.016);
        assert!(!w.just_burrowed);
    }

    #[test]
    fn just_burrowed_fires_only_once() {
        let mut w = w();
        w.burrow();
        w.tick(7.0); // deep
        w.tick(0.016); // cleared
        w.tick(1.0); // still at max, no re-fire
        assert!(!w.just_burrowed);
    }

    #[test]
    fn expel_stops_burrowing() {
        let mut w = w();
        w.burrow();
        w.tick(2.0);
        w.expel();
        assert!(!w.burrowing);
    }

    #[test]
    fn expel_resets_depth_to_zero() {
        let mut w = w();
        w.burrow();
        w.tick(2.0); // 3.0
        w.expel();
        assert_eq!(w.worm_depth, 0.0);
    }

    #[test]
    fn expel_fires_just_expelled() {
        let mut w = w();
        w.burrow();
        w.tick(2.0);
        w.expel();
        assert!(w.just_expelled);
    }

    #[test]
    fn expel_no_op_when_not_burrowing() {
        let mut w = w();
        w.expel();
        assert!(!w.just_expelled);
    }

    #[test]
    fn just_expelled_clears_next_tick() {
        let mut w = w();
        w.burrow();
        w.tick(1.0);
        w.expel();
        w.tick(0.016);
        assert!(!w.just_expelled);
    }

    #[test]
    fn burrow_again_after_expel() {
        let mut w = w();
        w.burrow();
        w.tick(2.0); // 3.0
        w.expel(); // reset
        w.burrow(); // start again
        w.tick(1.0); // 1.5
        assert!((w.worm_depth - 1.5).abs() < 1e-4);
        assert!(w.burrowing);
    }

    #[test]
    fn is_deep_true_at_max() {
        let mut w = w();
        w.burrow();
        w.tick(100.0);
        assert!(w.is_deep());
    }

    #[test]
    fn is_deep_false_below_max() {
        let mut w = w();
        w.burrow();
        w.tick(1.0); // 1.5 < 10.0
        assert!(!w.is_deep());
    }

    #[test]
    fn is_deep_false_when_disabled() {
        let mut w = w();
        w.burrow();
        w.tick(100.0);
        w.enabled = false;
        assert!(!w.is_deep());
    }

    #[test]
    fn is_deep_false_after_expel() {
        let mut w = w();
        w.burrow();
        w.tick(100.0);
        w.expel();
        assert!(!w.is_deep());
    }

    #[test]
    fn depth_fraction_zero_when_shallow() {
        let w = w();
        assert_eq!(w.depth_fraction(), 0.0);
    }

    #[test]
    fn depth_fraction_half_at_midpoint() {
        let mut w = w();
        w.worm_depth = 5.0;
        assert!((w.depth_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn depth_fraction_one_at_max() {
        let mut w = w();
        w.burrow();
        w.tick(100.0);
        assert!((w.depth_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_penetration_base_when_shallow() {
        let w = w();
        assert!((w.effective_penetration(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_penetration_scaled_at_half_depth() {
        let mut w = w();
        w.worm_depth = 5.0; // fraction = 0.5
                            // 100 * (1 + 0.5) = 150
        assert!((w.effective_penetration(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_penetration_doubled_at_max_depth() {
        let mut w = w();
        w.burrow();
        w.tick(100.0); // full depth
                       // 100 * (1 + 1.0) = 200
        assert!((w.effective_penetration(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_penetration_passthrough_when_disabled() {
        let mut w = w();
        w.burrow();
        w.tick(100.0);
        w.enabled = false;
        assert!((w.effective_penetration(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_depth_clamped_to_one() {
        let w = Worm::new(0.0, 1.5);
        assert!((w.max_depth - 1.0).abs() < 1e-5);
    }

    #[test]
    fn burrow_rate_clamped_to_zero() {
        let w = Worm::new(10.0, -2.0);
        assert_eq!(w.burrow_rate, 0.0);
    }
}
