use bevy_ecs::prelude::Component;

/// Slide-fastener closure tracker. `closure` builds via `pull(amount)` and
/// slides passively at `slide_rate` per second in `tick(dt)` or is opened
/// immediately via `unzip(amount)`.
///
/// Models garment-seal completion bars, bag-fastening progress gauges,
/// tent-door closure fill levels, wetsuit-zip integrity trackers, sleeping-bag
/// seal saturation indicators, pressure-suit fastening progress meters,
/// spacesuit-seam closure accumulators, tactical-vest closure state bars,
/// or any mechanic where a small metal slider travels along interlocking
/// teeth, pulling two edges together tooth by tooth into a seal so tight
/// that nothing leaks through — until the pull-tab snags and the whole
/// thing gapes wide open in the worst possible moment.
///
/// `pull(amount)` adds closure; fires `just_sealed` when first reaching
/// `max_closure`. No-op when disabled.
///
/// `unzip(amount)` reduces closure immediately; fires `just_open`
/// when reaching 0. No-op when disabled or already open.
///
/// `tick(dt)` clears both flags, then increases closure by
/// `slide_rate * dt` (capped at `max_closure`). Fires `just_sealed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_sealed()` returns `closure >= max_closure && enabled`.
///
/// `is_open()` returns `closure == 0.0` (not gated by `enabled`).
///
/// `closure_fraction()` returns `(closure / max_closure).clamp(0, 1)`.
///
/// `effective_seal(scale)` returns `scale * closure_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — slides at 5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zipper {
    pub closure: f32,
    pub max_closure: f32,
    pub slide_rate: f32,
    pub just_sealed: bool,
    pub just_open: bool,
    pub enabled: bool,
}

impl Zipper {
    pub fn new(max_closure: f32, slide_rate: f32) -> Self {
        Self {
            closure: 0.0,
            max_closure: max_closure.max(0.1),
            slide_rate: slide_rate.max(0.0),
            just_sealed: false,
            just_open: false,
            enabled: true,
        }
    }

    /// Add closure; fires `just_sealed` when first reaching max.
    /// No-op when disabled.
    pub fn pull(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.closure < self.max_closure;
        self.closure = (self.closure + amount).min(self.max_closure);
        if was_below && self.closure >= self.max_closure {
            self.just_sealed = true;
        }
    }

    /// Reduce closure; fires `just_open` when reaching 0.
    /// No-op when disabled or already open.
    pub fn unzip(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.closure <= 0.0 {
            return;
        }
        self.closure = (self.closure - amount).max(0.0);
        if self.closure <= 0.0 {
            self.just_open = true;
        }
    }

    /// Clear flags, then increase closure by `slide_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_sealed = false;
        self.just_open = false;
        if self.enabled && self.slide_rate > 0.0 && self.closure < self.max_closure {
            let was_below = self.closure < self.max_closure;
            self.closure = (self.closure + self.slide_rate * dt).min(self.max_closure);
            if was_below && self.closure >= self.max_closure {
                self.just_sealed = true;
            }
        }
    }

    /// `true` when closure is at maximum and component is enabled.
    pub fn is_sealed(&self) -> bool {
        self.closure >= self.max_closure && self.enabled
    }

    /// `true` when closure is 0 (not gated by `enabled`).
    pub fn is_open(&self) -> bool {
        self.closure == 0.0
    }

    /// Fraction of maximum closure [0.0, 1.0].
    pub fn closure_fraction(&self) -> f32 {
        (self.closure / self.max_closure).clamp(0.0, 1.0)
    }

    /// Returns `scale * closure_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_seal(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.closure_fraction()
    }
}

impl Default for Zipper {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zipper {
        Zipper::new(100.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_open() {
        let z = z();
        assert_eq!(z.closure, 0.0);
        assert!(z.is_open());
        assert!(!z.is_sealed());
    }

    #[test]
    fn new_clamps_max_closure() {
        let z = Zipper::new(-5.0, 5.0);
        assert!((z.max_closure - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_slide_rate() {
        let z = Zipper::new(100.0, -3.0);
        assert_eq!(z.slide_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zipper::default();
        assert!((z.max_closure - 100.0).abs() < 1e-5);
        assert!((z.slide_rate - 5.0).abs() < 1e-5);
    }

    // --- pull ---

    #[test]
    fn pull_adds_closure() {
        let mut z = z();
        z.pull(40.0);
        assert!((z.closure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn pull_clamps_at_max() {
        let mut z = z();
        z.pull(200.0);
        assert!((z.closure - 100.0).abs() < 1e-3);
    }

    #[test]
    fn pull_fires_just_sealed_at_max() {
        let mut z = z();
        z.pull(100.0);
        assert!(z.just_sealed);
        assert!(z.is_sealed());
    }

    #[test]
    fn pull_no_just_sealed_when_already_at_max() {
        let mut z = z();
        z.closure = 100.0;
        z.pull(10.0);
        assert!(!z.just_sealed);
    }

    #[test]
    fn pull_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.pull(50.0);
        assert_eq!(z.closure, 0.0);
    }

    #[test]
    fn pull_no_op_when_amount_zero() {
        let mut z = z();
        z.pull(0.0);
        assert_eq!(z.closure, 0.0);
    }

    // --- unzip ---

    #[test]
    fn unzip_reduces_closure() {
        let mut z = z();
        z.closure = 60.0;
        z.unzip(20.0);
        assert!((z.closure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn unzip_clamps_at_zero() {
        let mut z = z();
        z.closure = 30.0;
        z.unzip(200.0);
        assert_eq!(z.closure, 0.0);
    }

    #[test]
    fn unzip_fires_just_open_at_zero() {
        let mut z = z();
        z.closure = 30.0;
        z.unzip(30.0);
        assert!(z.just_open);
    }

    #[test]
    fn unzip_no_op_when_already_open() {
        let mut z = z();
        z.unzip(10.0);
        assert!(!z.just_open);
    }

    #[test]
    fn unzip_no_op_when_disabled() {
        let mut z = z();
        z.closure = 50.0;
        z.enabled = false;
        z.unzip(50.0);
        assert!((z.closure - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_slides_closure() {
        let mut z = z(); // rate=5
        z.tick(2.0); // 0 + 5*2 = 10
        assert!((z.closure - 10.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_sealed_on_slide_to_max() {
        let mut z = Zipper::new(100.0, 200.0);
        z.closure = 95.0;
        z.tick(1.0);
        assert!(z.just_sealed);
        assert!(z.is_sealed());
    }

    #[test]
    fn tick_no_slide_when_already_sealed() {
        let mut z = z();
        z.closure = 100.0;
        z.tick(1.0);
        assert!(!z.just_sealed);
    }

    #[test]
    fn tick_no_slide_when_rate_zero() {
        let mut z = Zipper::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.closure, 0.0);
    }

    #[test]
    fn tick_no_slide_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.closure, 0.0);
    }

    #[test]
    fn tick_clears_just_sealed() {
        let mut z = Zipper::new(100.0, 200.0);
        z.closure = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_sealed);
    }

    #[test]
    fn tick_clears_just_open() {
        let mut z = z();
        z.closure = 10.0;
        z.unzip(10.0);
        z.tick(0.016);
        assert!(!z.just_open);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=5
        z.tick(4.0); // 5*4 = 20
        assert!((z.closure - 20.0).abs() < 1e-3);
    }

    // --- is_sealed / is_open ---

    #[test]
    fn is_sealed_false_when_disabled() {
        let mut z = z();
        z.closure = 100.0;
        z.enabled = false;
        assert!(!z.is_sealed());
    }

    #[test]
    fn is_open_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_open());
    }

    // --- closure_fraction / effective_seal ---

    #[test]
    fn closure_fraction_zero_when_open() {
        assert_eq!(z().closure_fraction(), 0.0);
    }

    #[test]
    fn closure_fraction_half_at_midpoint() {
        let mut z = z();
        z.closure = 50.0;
        assert!((z.closure_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_seal_zero_when_open() {
        assert_eq!(z().effective_seal(100.0), 0.0);
    }

    #[test]
    fn effective_seal_scales_with_closure() {
        let mut z = z();
        z.closure = 75.0;
        assert!((z.effective_seal(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_seal_zero_when_disabled() {
        let mut z = z();
        z.closure = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_seal(100.0), 0.0);
    }
}
