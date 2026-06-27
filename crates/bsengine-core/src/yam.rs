use bevy_ecs::prelude::Component;

/// Periodic resource-yield generator with harvestable accumulation. Models
/// farms, mines, generators, or any entity that passively produces a
/// resource over time up to a capped store, which a caller then harvests
/// via `harvest()`. Distinct from `Fuel` (which *depletes*), `Regen`
/// (which heals a specific stat), and `Zest` (which clamps without a
/// harvest action).
///
/// `tick(dt)` adds `yield_rate * dt` to `yield_stored`, capping at
/// `yield_cap`. Fires `just_capped` the first time `yield_stored` reaches
/// `yield_cap`. No-op (beyond flag clear) when disabled.
///
/// `harvest()` returns the current `yield_stored` and resets it to 0.
/// Returns 0.0 when disabled or empty. No flags.
///
/// `harvest_min(min_yield)` returns the stored amount and resets only if
/// `yield_stored >= min_yield`; otherwise returns 0.0 (partial harvest
/// rejected). Useful for recipes that require a minimum batch.
///
/// `is_ready(min_yield)` returns `yield_stored >= min_yield && enabled`.
///
/// `fill_fraction()` returns `(yield_stored / yield_cap).clamp(0.0, 1.0)`.
///
/// `effective_output(base)` returns `base * fill_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(10.0, 1.0)` — cap 10, rate 1/s.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yam {
    /// Passively generated per second. Clamped >= 0.0.
    pub yield_rate: f32,
    /// Current accumulated yield [0, yield_cap].
    pub yield_stored: f32,
    /// Maximum storable yield before overflow. Clamped >= 0.01.
    pub yield_cap: f32,
    pub just_capped: bool,
    pub enabled: bool,
}

impl Yam {
    pub fn new(yield_cap: f32, yield_rate: f32) -> Self {
        Self {
            yield_rate: yield_rate.max(0.0),
            yield_stored: 0.0,
            yield_cap: yield_cap.max(0.01),
            just_capped: false,
            enabled: true,
        }
    }

    /// Advance one frame: clear flags, then accumulate yield. Fires
    /// `just_capped` on first reaching `yield_cap`. No-op (beyond flag
    /// clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_capped = false;

        if !self.enabled {
            return;
        }

        let was_below_cap = self.yield_stored < self.yield_cap;
        self.yield_stored = (self.yield_stored + self.yield_rate * dt).min(self.yield_cap);
        if was_below_cap && self.yield_stored >= self.yield_cap {
            self.just_capped = true;
        }
    }

    /// Take all stored yield. Returns current `yield_stored` and resets to
    /// 0. Returns 0.0 when disabled.
    pub fn harvest(&mut self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let amount = self.yield_stored;
        self.yield_stored = 0.0;
        amount
    }

    /// Take stored yield only if `yield_stored >= min_yield`. Returns the
    /// stored amount and resets on success; returns 0.0 on partial batch
    /// or when disabled.
    pub fn harvest_min(&mut self, min_yield: f32) -> f32 {
        if !self.enabled || self.yield_stored < min_yield {
            return 0.0;
        }
        let amount = self.yield_stored;
        self.yield_stored = 0.0;
        amount
    }

    /// `true` when `yield_stored >= min_yield` and enabled.
    pub fn is_ready(&self, min_yield: f32) -> bool {
        self.yield_stored >= min_yield && self.enabled
    }

    /// Current yield as a fraction of cap [0.0, 1.0].
    pub fn fill_fraction(&self) -> f32 {
        (self.yield_stored / self.yield_cap).clamp(0.0, 1.0)
    }

    /// Scale `base` by current fill. Returns `base * fill_fraction()` when
    /// enabled; `0.0` when disabled.
    pub fn effective_output(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.fill_fraction()
    }
}

impl Default for Yam {
    fn default() -> Self {
        Self::new(10.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yam {
        Yam::new(10.0, 1.0) // cap=10, rate=1/s
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let y = y();
        assert_eq!(y.yield_stored, 0.0);
        assert!((y.yield_cap - 10.0).abs() < 1e-5);
        assert!((y.yield_rate - 1.0).abs() < 1e-5);
        assert!(!y.just_capped);
    }

    #[test]
    fn yield_cap_clamped_to_min() {
        let y = Yam::new(0.0, 1.0);
        assert!((y.yield_cap - 0.01).abs() < 1e-6);
    }

    #[test]
    fn yield_rate_clamped_to_zero() {
        let y = Yam::new(10.0, -1.0);
        assert_eq!(y.yield_rate, 0.0);
    }

    // --- tick: accumulation ---

    #[test]
    fn tick_accumulates_yield() {
        let mut y = y(); // rate=1/s
        y.tick(3.0);
        assert!((y.yield_stored - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_yield_cap() {
        let mut y = y(); // cap=10
        y.tick(15.0); // would overshoot
        assert!((y.yield_stored - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_capped_on_reaching_cap() {
        let mut y = y(); // cap=10
        y.tick(10.0); // exactly hits cap
        assert!(y.just_capped);
    }

    #[test]
    fn tick_fires_just_capped_crossing_cap() {
        let mut y = y();
        y.tick(7.0);
        y.tick(5.0); // crosses cap
        assert!(y.just_capped);
    }

    #[test]
    fn tick_does_not_refire_just_capped_when_already_capped() {
        let mut y = y();
        y.tick(10.0); // capped
        y.tick(1.0); // still at cap
        assert!(!y.just_capped); // flag cleared by tick
    }

    #[test]
    fn tick_just_capped_clears_next_frame() {
        let mut y = y();
        y.tick(10.0);
        y.tick(0.016);
        assert!(!y.just_capped);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.tick(5.0);
        assert_eq!(y.yield_stored, 0.0);
    }

    #[test]
    fn tick_clears_flags_when_disabled() {
        let mut y = y();
        y.just_capped = true;
        y.enabled = false;
        y.tick(0.016);
        assert!(!y.just_capped);
    }

    // --- harvest ---

    #[test]
    fn harvest_returns_stored_amount() {
        let mut y = y();
        y.tick(4.0);
        let amount = y.harvest();
        assert!((amount - 4.0).abs() < 1e-4);
    }

    #[test]
    fn harvest_resets_stored_to_zero() {
        let mut y = y();
        y.tick(4.0);
        y.harvest();
        assert_eq!(y.yield_stored, 0.0);
    }

    #[test]
    fn harvest_returns_zero_when_empty() {
        let mut y = y();
        let amount = y.harvest();
        assert_eq!(amount, 0.0);
    }

    #[test]
    fn harvest_returns_zero_when_disabled() {
        let mut y = y();
        y.tick(5.0);
        y.enabled = false;
        let amount = y.harvest();
        assert_eq!(amount, 0.0);
        assert!((y.yield_stored - 5.0).abs() < 1e-4); // unchanged
    }

    #[test]
    fn harvest_allows_re_accumulation() {
        let mut y = y();
        y.tick(5.0);
        y.harvest();
        y.tick(3.0);
        assert!((y.yield_stored - 3.0).abs() < 1e-4);
    }

    // --- harvest_min ---

    #[test]
    fn harvest_min_returns_amount_when_sufficient() {
        let mut y = y();
        y.tick(6.0);
        let amount = y.harvest_min(5.0);
        assert!((amount - 6.0).abs() < 1e-4);
        assert_eq!(y.yield_stored, 0.0);
    }

    #[test]
    fn harvest_min_rejects_partial_batch() {
        let mut y = y();
        y.tick(3.0);
        let amount = y.harvest_min(5.0); // not enough
        assert_eq!(amount, 0.0);
        assert!((y.yield_stored - 3.0).abs() < 1e-4); // preserved
    }

    #[test]
    fn harvest_min_rejects_when_disabled() {
        let mut y = y();
        y.tick(8.0);
        y.enabled = false;
        let amount = y.harvest_min(1.0);
        assert_eq!(amount, 0.0);
    }

    // --- is_ready ---

    #[test]
    fn is_ready_false_when_empty() {
        let y = y();
        assert!(!y.is_ready(1.0));
    }

    #[test]
    fn is_ready_true_when_sufficient() {
        let mut y = y();
        y.tick(5.0);
        assert!(y.is_ready(5.0));
    }

    #[test]
    fn is_ready_false_below_min() {
        let mut y = y();
        y.tick(3.0);
        assert!(!y.is_ready(5.0));
    }

    #[test]
    fn is_ready_false_when_disabled() {
        let mut y = y();
        y.tick(5.0);
        y.enabled = false;
        assert!(!y.is_ready(1.0));
    }

    // --- fill_fraction ---

    #[test]
    fn fill_fraction_zero_when_empty() {
        let y = y();
        assert_eq!(y.fill_fraction(), 0.0);
    }

    #[test]
    fn fill_fraction_half_at_midpoint() {
        let mut y = y(); // cap=10
        y.tick(5.0);
        assert!((y.fill_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn fill_fraction_one_when_full() {
        let mut y = y();
        y.tick(10.0);
        assert!((y.fill_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_output ---

    #[test]
    fn effective_output_zero_when_empty() {
        let y = y();
        assert_eq!(y.effective_output(100.0), 0.0);
    }

    #[test]
    fn effective_output_at_half_fill() {
        let mut y = y();
        y.tick(5.0); // fraction=0.5 → 100*0.5=50
        assert!((y.effective_output(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_full_at_cap() {
        let mut y = y();
        y.tick(10.0); // fraction=1.0 → 100*1=100
        assert!((y.effective_output(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_output_zero_when_disabled() {
        let mut y = y();
        y.tick(5.0);
        y.enabled = false;
        assert_eq!(y.effective_output(100.0), 0.0);
    }

    // --- full harvest cycle ---

    #[test]
    fn accumulate_harvest_recycle() {
        let mut y = Yam::new(5.0, 1.0);
        y.tick(5.0); // full
        assert!(y.just_capped);
        let a = y.harvest();
        assert!((a - 5.0).abs() < 1e-4);
        y.tick(2.5); // half full again
        assert!((y.fill_fraction() - 0.5).abs() < 1e-4);
        let b = y.harvest_min(3.0); // 2.5 < 3.0, rejected
        assert_eq!(b, 0.0);
        y.tick(1.0); // now 3.5
        let c = y.harvest_min(3.0); // 3.5 >= 3.0, accepted
        assert!((c - 3.5).abs() < 1e-4);
    }
}
