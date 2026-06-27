use bevy_ecs::prelude::Component;

/// Tier-ascension tracker. `tier` builds via `ascend(amount)` and
/// rises passively at `rise_rate` per second in `tick(dt)` or
/// collapses immediately via `collapse(amount)`.
///
/// Models stepped-pyramid ritual-tier meters, Mesopotamian temple
/// elevation progress bars, ziggurat-level dedication accumulators,
/// ancient-architecture prestige gauges, sacred-mountain ascent
/// trackers, staged-construction completion indicators, monument-
/// tier fill levels, tiered-altar offering progress bars, or any
/// mechanic where steadily stacking ritual offerings on each
/// successive terrace elevates a civilization's standing until the
/// topmost shrine blazes like a beacon visible across the alluvial
/// plain.
///
/// `ascend(amount)` adds tier; fires `just_crowned` when first
/// reaching `max_tier`. No-op when disabled.
///
/// `collapse(amount)` reduces tier immediately; fires `just_razed`
/// when reaching 0. No-op when disabled or already razed.
///
/// `tick(dt)` clears both flags, then increases tier by
/// `rise_rate * dt` (capped at `max_tier`). Fires `just_crowned`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_crowned()` returns `tier >= max_tier && enabled`.
///
/// `is_razed()` returns `tier == 0.0` (not gated by `enabled`).
///
/// `tier_fraction()` returns `(tier / max_tier).clamp(0, 1)`.
///
/// `effective_prestige(scale)` returns `scale * tier_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — rises at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ziggurat {
    pub tier: f32,
    pub max_tier: f32,
    pub rise_rate: f32,
    pub just_crowned: bool,
    pub just_razed: bool,
    pub enabled: bool,
}

impl Ziggurat {
    pub fn new(max_tier: f32, rise_rate: f32) -> Self {
        Self {
            tier: 0.0,
            max_tier: max_tier.max(0.1),
            rise_rate: rise_rate.max(0.0),
            just_crowned: false,
            just_razed: false,
            enabled: true,
        }
    }

    /// Add tier; fires `just_crowned` when first reaching max.
    /// No-op when disabled.
    pub fn ascend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.tier < self.max_tier;
        self.tier = (self.tier + amount).min(self.max_tier);
        if was_below && self.tier >= self.max_tier {
            self.just_crowned = true;
        }
    }

    /// Reduce tier; fires `just_razed` when reaching 0.
    /// No-op when disabled or already razed.
    pub fn collapse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.tier <= 0.0 {
            return;
        }
        self.tier = (self.tier - amount).max(0.0);
        if self.tier <= 0.0 {
            self.just_razed = true;
        }
    }

    /// Clear flags, then increase tier by `rise_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_crowned = false;
        self.just_razed = false;
        if self.enabled && self.rise_rate > 0.0 && self.tier < self.max_tier {
            let was_below = self.tier < self.max_tier;
            self.tier = (self.tier + self.rise_rate * dt).min(self.max_tier);
            if was_below && self.tier >= self.max_tier {
                self.just_crowned = true;
            }
        }
    }

    /// `true` when tier is at maximum and component is enabled.
    pub fn is_crowned(&self) -> bool {
        self.tier >= self.max_tier && self.enabled
    }

    /// `true` when tier is 0 (not gated by `enabled`).
    pub fn is_razed(&self) -> bool {
        self.tier == 0.0
    }

    /// Fraction of maximum tier [0.0, 1.0].
    pub fn tier_fraction(&self) -> f32 {
        (self.tier / self.max_tier).clamp(0.0, 1.0)
    }

    /// Returns `scale * tier_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_prestige(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.tier_fraction()
    }
}

impl Default for Ziggurat {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Ziggurat {
        Ziggurat::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_razed() {
        let z = z();
        assert_eq!(z.tier, 0.0);
        assert!(z.is_razed());
        assert!(!z.is_crowned());
    }

    #[test]
    fn new_clamps_max_tier() {
        let z = Ziggurat::new(-5.0, 1.0);
        assert!((z.max_tier - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_rise_rate() {
        let z = Ziggurat::new(100.0, -3.0);
        assert_eq!(z.rise_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Ziggurat::default();
        assert!((z.max_tier - 100.0).abs() < 1e-5);
        assert!((z.rise_rate - 1.0).abs() < 1e-5);
    }

    // --- ascend ---

    #[test]
    fn ascend_adds_tier() {
        let mut z = z();
        z.ascend(40.0);
        assert!((z.tier - 40.0).abs() < 1e-3);
    }

    #[test]
    fn ascend_clamps_at_max() {
        let mut z = z();
        z.ascend(200.0);
        assert!((z.tier - 100.0).abs() < 1e-3);
    }

    #[test]
    fn ascend_fires_just_crowned_at_max() {
        let mut z = z();
        z.ascend(100.0);
        assert!(z.just_crowned);
        assert!(z.is_crowned());
    }

    #[test]
    fn ascend_no_just_crowned_when_already_at_max() {
        let mut z = z();
        z.tier = 100.0;
        z.ascend(10.0);
        assert!(!z.just_crowned);
    }

    #[test]
    fn ascend_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.ascend(50.0);
        assert_eq!(z.tier, 0.0);
    }

    #[test]
    fn ascend_no_op_when_amount_zero() {
        let mut z = z();
        z.ascend(0.0);
        assert_eq!(z.tier, 0.0);
    }

    // --- collapse ---

    #[test]
    fn collapse_reduces_tier() {
        let mut z = z();
        z.tier = 60.0;
        z.collapse(20.0);
        assert!((z.tier - 40.0).abs() < 1e-3);
    }

    #[test]
    fn collapse_clamps_at_zero() {
        let mut z = z();
        z.tier = 30.0;
        z.collapse(200.0);
        assert_eq!(z.tier, 0.0);
    }

    #[test]
    fn collapse_fires_just_razed_at_zero() {
        let mut z = z();
        z.tier = 30.0;
        z.collapse(30.0);
        assert!(z.just_razed);
    }

    #[test]
    fn collapse_no_op_when_already_razed() {
        let mut z = z();
        z.collapse(10.0);
        assert!(!z.just_razed);
    }

    #[test]
    fn collapse_no_op_when_disabled() {
        let mut z = z();
        z.tier = 50.0;
        z.enabled = false;
        z.collapse(50.0);
        assert!((z.tier - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_rises_tier() {
        let mut z = z(); // rate=1
        z.tick(3.0); // 0 + 1*3 = 3
        assert!((z.tier - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_crowned_on_rise_to_max() {
        let mut z = Ziggurat::new(100.0, 200.0);
        z.tier = 95.0;
        z.tick(1.0);
        assert!(z.just_crowned);
        assert!(z.is_crowned());
    }

    #[test]
    fn tick_no_rise_when_already_crowned() {
        let mut z = z();
        z.tier = 100.0;
        z.tick(1.0);
        assert!(!z.just_crowned);
    }

    #[test]
    fn tick_no_rise_when_rate_zero() {
        let mut z = Ziggurat::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.tier, 0.0);
    }

    #[test]
    fn tick_no_rise_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.tier, 0.0);
    }

    #[test]
    fn tick_clears_just_crowned() {
        let mut z = Ziggurat::new(100.0, 200.0);
        z.tier = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_crowned);
    }

    #[test]
    fn tick_clears_just_razed() {
        let mut z = z();
        z.tier = 10.0;
        z.collapse(10.0);
        z.tick(0.016);
        assert!(!z.just_razed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(7.0); // 1*7 = 7
        assert!((z.tier - 7.0).abs() < 1e-3);
    }

    // --- is_crowned / is_razed ---

    #[test]
    fn is_crowned_false_when_disabled() {
        let mut z = z();
        z.tier = 100.0;
        z.enabled = false;
        assert!(!z.is_crowned());
    }

    #[test]
    fn is_razed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_razed());
    }

    // --- tier_fraction / effective_prestige ---

    #[test]
    fn tier_fraction_zero_when_razed() {
        assert_eq!(z().tier_fraction(), 0.0);
    }

    #[test]
    fn tier_fraction_half_at_midpoint() {
        let mut z = z();
        z.tier = 50.0;
        assert!((z.tier_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_prestige_zero_when_razed() {
        assert_eq!(z().effective_prestige(100.0), 0.0);
    }

    #[test]
    fn effective_prestige_scales_with_tier() {
        let mut z = z();
        z.tier = 80.0;
        assert!((z.effective_prestige(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_prestige_zero_when_disabled() {
        let mut z = z();
        z.tier = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_prestige(100.0), 0.0);
    }
}
