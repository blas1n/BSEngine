use bevy_ecs::prelude::Component;

/// Hybrid-vigor tracker. `hybrid` builds via `crossbreed(amount)` and
/// intensifies passively at `blend_rate` per second in `tick(dt)` or
/// reverts immediately via `revert(amount)`.
///
/// Models hybrid-creature stat buffs, cross-species synergy meters,
/// chimera-fusion progress bars, gene-splice effectiveness gauges,
/// mixed-bloodline potency trackers, crossbred-ability fill levels,
/// mutation-blend accumulators, or any mechanic where combining two
/// distinct lineages produces escalating hybrid advantages.
///
/// `crossbreed(amount)` adds hybrid vigor; fires `just_vigorous` when
/// first reaching `max_hybrid`. No-op when disabled.
///
/// `revert(amount)` reduces hybrid immediately; fires `just_reverted`
/// when reaching 0. No-op when disabled or already reverted.
///
/// `tick(dt)` clears both flags, then increases hybrid by
/// `blend_rate * dt` (capped at `max_hybrid`). Fires `just_vigorous`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_vigorous()` returns `hybrid >= max_hybrid && enabled`.
///
/// `is_reverted()` returns `hybrid == 0.0` (not gated by `enabled`).
///
/// `hybrid_fraction()` returns `(hybrid / max_hybrid).clamp(0, 1)`.
///
/// `effective_vigor(scale)` returns `scale * hybrid_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 7.0)` — blends hybrid at 7 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zorse {
    pub hybrid: f32,
    pub max_hybrid: f32,
    pub blend_rate: f32,
    pub just_vigorous: bool,
    pub just_reverted: bool,
    pub enabled: bool,
}

impl Zorse {
    pub fn new(max_hybrid: f32, blend_rate: f32) -> Self {
        Self {
            hybrid: 0.0,
            max_hybrid: max_hybrid.max(0.1),
            blend_rate: blend_rate.max(0.0),
            just_vigorous: false,
            just_reverted: false,
            enabled: true,
        }
    }

    /// Add hybrid vigor; fires `just_vigorous` when first reaching max.
    /// No-op when disabled.
    pub fn crossbreed(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.hybrid < self.max_hybrid;
        self.hybrid = (self.hybrid + amount).min(self.max_hybrid);
        if was_below && self.hybrid >= self.max_hybrid {
            self.just_vigorous = true;
        }
    }

    /// Reduce hybrid; fires `just_reverted` when reaching 0.
    /// No-op when disabled or already reverted.
    pub fn revert(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.hybrid <= 0.0 {
            return;
        }
        self.hybrid = (self.hybrid - amount).max(0.0);
        if self.hybrid <= 0.0 {
            self.just_reverted = true;
        }
    }

    /// Clear flags, then increase hybrid by `blend_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_vigorous = false;
        self.just_reverted = false;
        if self.enabled && self.blend_rate > 0.0 && self.hybrid < self.max_hybrid {
            let was_below = self.hybrid < self.max_hybrid;
            self.hybrid = (self.hybrid + self.blend_rate * dt).min(self.max_hybrid);
            if was_below && self.hybrid >= self.max_hybrid {
                self.just_vigorous = true;
            }
        }
    }

    /// `true` when hybrid is at maximum and component is enabled.
    pub fn is_vigorous(&self) -> bool {
        self.hybrid >= self.max_hybrid && self.enabled
    }

    /// `true` when hybrid is 0 (not gated by `enabled`).
    pub fn is_reverted(&self) -> bool {
        self.hybrid == 0.0
    }

    /// Fraction of maximum hybrid [0.0, 1.0].
    pub fn hybrid_fraction(&self) -> f32 {
        (self.hybrid / self.max_hybrid).clamp(0.0, 1.0)
    }

    /// Returns `scale * hybrid_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vigor(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.hybrid_fraction()
    }
}

impl Default for Zorse {
    fn default() -> Self {
        Self::new(100.0, 7.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zorse {
        Zorse::new(100.0, 7.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_reverted() {
        let z = z();
        assert_eq!(z.hybrid, 0.0);
        assert!(z.is_reverted());
        assert!(!z.is_vigorous());
    }

    #[test]
    fn new_clamps_max_hybrid() {
        let z = Zorse::new(-5.0, 7.0);
        assert!((z.max_hybrid - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_blend_rate() {
        let z = Zorse::new(100.0, -3.0);
        assert_eq!(z.blend_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zorse::default();
        assert!((z.max_hybrid - 100.0).abs() < 1e-5);
        assert!((z.blend_rate - 7.0).abs() < 1e-5);
    }

    // --- crossbreed ---

    #[test]
    fn crossbreed_adds_hybrid() {
        let mut z = z();
        z.crossbreed(40.0);
        assert!((z.hybrid - 40.0).abs() < 1e-3);
    }

    #[test]
    fn crossbreed_clamps_at_max() {
        let mut z = z();
        z.crossbreed(200.0);
        assert!((z.hybrid - 100.0).abs() < 1e-3);
    }

    #[test]
    fn crossbreed_fires_just_vigorous_at_max() {
        let mut z = z();
        z.crossbreed(100.0);
        assert!(z.just_vigorous);
        assert!(z.is_vigorous());
    }

    #[test]
    fn crossbreed_no_just_vigorous_when_already_at_max() {
        let mut z = z();
        z.hybrid = 100.0;
        z.crossbreed(10.0);
        assert!(!z.just_vigorous);
    }

    #[test]
    fn crossbreed_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.crossbreed(50.0);
        assert_eq!(z.hybrid, 0.0);
    }

    #[test]
    fn crossbreed_no_op_when_amount_zero() {
        let mut z = z();
        z.crossbreed(0.0);
        assert_eq!(z.hybrid, 0.0);
    }

    // --- revert ---

    #[test]
    fn revert_reduces_hybrid() {
        let mut z = z();
        z.hybrid = 60.0;
        z.revert(20.0);
        assert!((z.hybrid - 40.0).abs() < 1e-3);
    }

    #[test]
    fn revert_clamps_at_zero() {
        let mut z = z();
        z.hybrid = 30.0;
        z.revert(200.0);
        assert_eq!(z.hybrid, 0.0);
    }

    #[test]
    fn revert_fires_just_reverted_at_zero() {
        let mut z = z();
        z.hybrid = 30.0;
        z.revert(30.0);
        assert!(z.just_reverted);
    }

    #[test]
    fn revert_no_op_when_already_reverted() {
        let mut z = z();
        z.revert(10.0);
        assert!(!z.just_reverted);
    }

    #[test]
    fn revert_no_op_when_disabled() {
        let mut z = z();
        z.hybrid = 50.0;
        z.enabled = false;
        z.revert(50.0);
        assert!((z.hybrid - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_blends_hybrid() {
        let mut z = z(); // rate=7
        z.tick(1.0); // 0 + 7 = 7
        assert!((z.hybrid - 7.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_vigorous_on_blend_to_max() {
        let mut z = Zorse::new(100.0, 200.0);
        z.hybrid = 95.0;
        z.tick(1.0);
        assert!(z.just_vigorous);
        assert!(z.is_vigorous());
    }

    #[test]
    fn tick_no_blend_when_already_vigorous() {
        let mut z = z();
        z.hybrid = 100.0;
        z.tick(1.0);
        assert!(!z.just_vigorous);
    }

    #[test]
    fn tick_no_blend_when_rate_zero() {
        let mut z = Zorse::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.hybrid, 0.0);
    }

    #[test]
    fn tick_no_blend_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.hybrid, 0.0);
    }

    #[test]
    fn tick_clears_just_vigorous() {
        let mut z = Zorse::new(100.0, 200.0);
        z.hybrid = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_vigorous);
    }

    #[test]
    fn tick_clears_just_reverted() {
        let mut z = z();
        z.hybrid = 10.0;
        z.revert(10.0);
        z.tick(0.016);
        assert!(!z.just_reverted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=7
        z.tick(3.0); // 7*3 = 21
        assert!((z.hybrid - 21.0).abs() < 1e-3);
    }

    // --- is_vigorous / is_reverted ---

    #[test]
    fn is_vigorous_false_when_disabled() {
        let mut z = z();
        z.hybrid = 100.0;
        z.enabled = false;
        assert!(!z.is_vigorous());
    }

    #[test]
    fn is_reverted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_reverted());
    }

    // --- hybrid_fraction / effective_vigor ---

    #[test]
    fn hybrid_fraction_zero_when_reverted() {
        assert_eq!(z().hybrid_fraction(), 0.0);
    }

    #[test]
    fn hybrid_fraction_half_at_midpoint() {
        let mut z = z();
        z.hybrid = 50.0;
        assert!((z.hybrid_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_vigor_zero_when_reverted() {
        assert_eq!(z().effective_vigor(100.0), 0.0);
    }

    #[test]
    fn effective_vigor_scales_with_hybrid() {
        let mut z = z();
        z.hybrid = 85.0;
        assert!((z.effective_vigor(100.0) - 85.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vigor_zero_when_disabled() {
        let mut z = z();
        z.hybrid = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_vigor(100.0), 0.0);
    }
}
