use bevy_ecs::prelude::Component;

/// Hybrid-vigor tracker. `vigor` builds via `hybridize(amount)` and
/// strengthens passively at `cross_rate` per second in `tick(dt)` or
/// weakens immediately via `weaken(amount)`.
///
/// Models yak-cattle hybrid crossbreeding meters, heterosis fill levels,
/// hybrid-vigor cultivation progress bars, livestock-crossbreed strength
/// accumulators, F1-generation advantage trackers, hardy-crossbreed
/// constitution gauges, genetic-diversity fitness indicators, crossbred-
/// species resilience meters, or any mechanic where controlled crossbreeding
/// between two robust species yields offspring stronger than either parent.
///
/// `hybridize(amount)` adds vigor; fires `just_thriving` when first
/// reaching `max_vigor`. No-op when disabled.
///
/// `weaken(amount)` reduces vigor immediately; fires `just_exhausted`
/// when reaching 0. No-op when disabled or already exhausted.
///
/// `tick(dt)` clears both flags, then increases vigor by
/// `cross_rate * dt` (capped at `max_vigor`). Fires `just_thriving`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_thriving()` returns `vigor >= max_vigor && enabled`.
///
/// `is_exhausted()` returns `vigor == 0.0` (not gated by `enabled`).
///
/// `vigor_fraction()` returns `(vigor / max_vigor).clamp(0, 1)`.
///
/// `effective_hybrid_strength(scale)` returns `scale * vigor_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.5)` — cross-breeds at 2.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zho {
    pub vigor: f32,
    pub max_vigor: f32,
    pub cross_rate: f32,
    pub just_thriving: bool,
    pub just_exhausted: bool,
    pub enabled: bool,
}

impl Zho {
    pub fn new(max_vigor: f32, cross_rate: f32) -> Self {
        Self {
            vigor: 0.0,
            max_vigor: max_vigor.max(0.1),
            cross_rate: cross_rate.max(0.0),
            just_thriving: false,
            just_exhausted: false,
            enabled: true,
        }
    }

    /// Add vigor; fires `just_thriving` when first reaching max.
    /// No-op when disabled.
    pub fn hybridize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.vigor < self.max_vigor;
        self.vigor = (self.vigor + amount).min(self.max_vigor);
        if was_below && self.vigor >= self.max_vigor {
            self.just_thriving = true;
        }
    }

    /// Reduce vigor; fires `just_exhausted` when reaching 0.
    /// No-op when disabled or already exhausted.
    pub fn weaken(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.vigor <= 0.0 {
            return;
        }
        self.vigor = (self.vigor - amount).max(0.0);
        if self.vigor <= 0.0 {
            self.just_exhausted = true;
        }
    }

    /// Clear flags, then increase vigor by `cross_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_thriving = false;
        self.just_exhausted = false;
        if self.enabled && self.cross_rate > 0.0 && self.vigor < self.max_vigor {
            let was_below = self.vigor < self.max_vigor;
            self.vigor = (self.vigor + self.cross_rate * dt).min(self.max_vigor);
            if was_below && self.vigor >= self.max_vigor {
                self.just_thriving = true;
            }
        }
    }

    /// `true` when vigor is at maximum and component is enabled.
    pub fn is_thriving(&self) -> bool {
        self.vigor >= self.max_vigor && self.enabled
    }

    /// `true` when vigor is 0 (not gated by `enabled`).
    pub fn is_exhausted(&self) -> bool {
        self.vigor == 0.0
    }

    /// Fraction of maximum vigor [0.0, 1.0].
    pub fn vigor_fraction(&self) -> f32 {
        (self.vigor / self.max_vigor).clamp(0.0, 1.0)
    }

    /// Returns `scale * vigor_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_hybrid_strength(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.vigor_fraction()
    }
}

impl Default for Zho {
    fn default() -> Self {
        Self::new(100.0, 2.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zho {
        Zho::new(100.0, 2.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_exhausted() {
        let z = z();
        assert_eq!(z.vigor, 0.0);
        assert!(z.is_exhausted());
        assert!(!z.is_thriving());
    }

    #[test]
    fn new_clamps_max_vigor() {
        let z = Zho::new(-5.0, 2.5);
        assert!((z.max_vigor - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_cross_rate() {
        let z = Zho::new(100.0, -3.0);
        assert_eq!(z.cross_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zho::default();
        assert!((z.max_vigor - 100.0).abs() < 1e-5);
        assert!((z.cross_rate - 2.5).abs() < 1e-5);
    }

    // --- hybridize ---

    #[test]
    fn hybridize_adds_vigor() {
        let mut z = z();
        z.hybridize(40.0);
        assert!((z.vigor - 40.0).abs() < 1e-3);
    }

    #[test]
    fn hybridize_clamps_at_max() {
        let mut z = z();
        z.hybridize(200.0);
        assert!((z.vigor - 100.0).abs() < 1e-3);
    }

    #[test]
    fn hybridize_fires_just_thriving_at_max() {
        let mut z = z();
        z.hybridize(100.0);
        assert!(z.just_thriving);
        assert!(z.is_thriving());
    }

    #[test]
    fn hybridize_no_just_thriving_when_already_at_max() {
        let mut z = z();
        z.vigor = 100.0;
        z.hybridize(10.0);
        assert!(!z.just_thriving);
    }

    #[test]
    fn hybridize_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.hybridize(50.0);
        assert_eq!(z.vigor, 0.0);
    }

    #[test]
    fn hybridize_no_op_when_amount_zero() {
        let mut z = z();
        z.hybridize(0.0);
        assert_eq!(z.vigor, 0.0);
    }

    // --- weaken ---

    #[test]
    fn weaken_reduces_vigor() {
        let mut z = z();
        z.vigor = 60.0;
        z.weaken(20.0);
        assert!((z.vigor - 40.0).abs() < 1e-3);
    }

    #[test]
    fn weaken_clamps_at_zero() {
        let mut z = z();
        z.vigor = 30.0;
        z.weaken(200.0);
        assert_eq!(z.vigor, 0.0);
    }

    #[test]
    fn weaken_fires_just_exhausted_at_zero() {
        let mut z = z();
        z.vigor = 30.0;
        z.weaken(30.0);
        assert!(z.just_exhausted);
    }

    #[test]
    fn weaken_no_op_when_already_exhausted() {
        let mut z = z();
        z.weaken(10.0);
        assert!(!z.just_exhausted);
    }

    #[test]
    fn weaken_no_op_when_disabled() {
        let mut z = z();
        z.vigor = 50.0;
        z.enabled = false;
        z.weaken(50.0);
        assert!((z.vigor - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_increases_vigor() {
        let mut z = z(); // rate=2.5
        z.tick(2.0); // 0 + 2.5*2 = 5
        assert!((z.vigor - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_thriving_on_cross_to_max() {
        let mut z = Zho::new(100.0, 200.0);
        z.vigor = 95.0;
        z.tick(1.0);
        assert!(z.just_thriving);
        assert!(z.is_thriving());
    }

    #[test]
    fn tick_no_cross_when_already_thriving() {
        let mut z = z();
        z.vigor = 100.0;
        z.tick(1.0);
        assert!(!z.just_thriving);
    }

    #[test]
    fn tick_no_cross_when_rate_zero() {
        let mut z = Zho::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.vigor, 0.0);
    }

    #[test]
    fn tick_no_cross_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.vigor, 0.0);
    }

    #[test]
    fn tick_clears_just_thriving() {
        let mut z = Zho::new(100.0, 200.0);
        z.vigor = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_thriving);
    }

    #[test]
    fn tick_clears_just_exhausted() {
        let mut z = z();
        z.vigor = 10.0;
        z.weaken(10.0);
        z.tick(0.016);
        assert!(!z.just_exhausted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2.5
        z.tick(4.0); // 2.5*4 = 10
        assert!((z.vigor - 10.0).abs() < 1e-3);
    }

    // --- is_thriving / is_exhausted ---

    #[test]
    fn is_thriving_false_when_disabled() {
        let mut z = z();
        z.vigor = 100.0;
        z.enabled = false;
        assert!(!z.is_thriving());
    }

    #[test]
    fn is_exhausted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_exhausted());
    }

    // --- vigor_fraction / effective_hybrid_strength ---

    #[test]
    fn vigor_fraction_zero_when_exhausted() {
        assert_eq!(z().vigor_fraction(), 0.0);
    }

    #[test]
    fn vigor_fraction_half_at_midpoint() {
        let mut z = z();
        z.vigor = 50.0;
        assert!((z.vigor_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_hybrid_strength_zero_when_exhausted() {
        assert_eq!(z().effective_hybrid_strength(100.0), 0.0);
    }

    #[test]
    fn effective_hybrid_strength_scales_with_vigor() {
        let mut z = z();
        z.vigor = 80.0;
        assert!((z.effective_hybrid_strength(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_hybrid_strength_zero_when_disabled() {
        let mut z = z();
        z.vigor = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_hybrid_strength(100.0), 0.0);
    }
}
