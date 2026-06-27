use bevy_ecs::prelude::Component;

/// Cycad-frond unfurling progress tracker. `frond` builds via
/// `unfurl(amount)` and extends passively at `grow_rate` per second
/// in `tick(dt)` or curls back immediately via `curl(amount)`.
///
/// Models tropical-cycad frond-extension fill levels, ancient-plant
/// canopy-coverage trackers, palm-like cycad growth saturation bars,
/// Carboniferous-era fern-frond unfurling gauges, garden-cycad
/// specimen maturity fill levels, botanical-preserve cycad-health
/// intensity meters, tropical-understory frond-density build-up
/// indicators, living-fossil specimen vitality bars, or any mechanic
/// where one of the planet's oldest surviving plant genera slowly
/// unfurls its stiff, pinnate fronds arc by arc until every leaflet
/// catches the dappled forest light — only for a drought or frost
/// to send each frond curling back into a tight protective crozier.
///
/// `unfurl(amount)` adds frond; fires `just_spread` when first
/// reaching `max_frond`. No-op when disabled.
///
/// `curl(amount)` reduces frond immediately; fires `just_crozier`
/// when reaching 0. No-op when disabled or already crozier.
///
/// `tick(dt)` clears both flags, then increases frond by
/// `grow_rate * dt` (capped at `max_frond`). Fires `just_spread`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_spread()` returns `frond >= max_frond && enabled`.
///
/// `is_crozier()` returns `frond == 0.0` (not gated by `enabled`).
///
/// `frond_fraction()` returns `(frond / max_frond).clamp(0, 1)`.
///
/// `effective_canopy(scale)` returns `scale * frond_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — grows at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zamia {
    pub frond: f32,
    pub max_frond: f32,
    pub grow_rate: f32,
    pub just_spread: bool,
    pub just_crozier: bool,
    pub enabled: bool,
}

impl Zamia {
    pub fn new(max_frond: f32, grow_rate: f32) -> Self {
        Self {
            frond: 0.0,
            max_frond: max_frond.max(0.1),
            grow_rate: grow_rate.max(0.0),
            just_spread: false,
            just_crozier: false,
            enabled: true,
        }
    }

    /// Add frond; fires `just_spread` when first reaching max.
    /// No-op when disabled.
    pub fn unfurl(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.frond < self.max_frond;
        self.frond = (self.frond + amount).min(self.max_frond);
        if was_below && self.frond >= self.max_frond {
            self.just_spread = true;
        }
    }

    /// Reduce frond; fires `just_crozier` when reaching 0.
    /// No-op when disabled or already crozier.
    pub fn curl(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.frond <= 0.0 {
            return;
        }
        self.frond = (self.frond - amount).max(0.0);
        if self.frond <= 0.0 {
            self.just_crozier = true;
        }
    }

    /// Clear flags, then increase frond by `grow_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_spread = false;
        self.just_crozier = false;
        if self.enabled && self.grow_rate > 0.0 && self.frond < self.max_frond {
            let was_below = self.frond < self.max_frond;
            self.frond = (self.frond + self.grow_rate * dt).min(self.max_frond);
            if was_below && self.frond >= self.max_frond {
                self.just_spread = true;
            }
        }
    }

    /// `true` when frond is at maximum and component is enabled.
    pub fn is_spread(&self) -> bool {
        self.frond >= self.max_frond && self.enabled
    }

    /// `true` when frond is 0 (not gated by `enabled`).
    pub fn is_crozier(&self) -> bool {
        self.frond == 0.0
    }

    /// Fraction of maximum frond [0.0, 1.0].
    pub fn frond_fraction(&self) -> f32 {
        (self.frond / self.max_frond).clamp(0.0, 1.0)
    }

    /// Returns `scale * frond_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_canopy(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.frond_fraction()
    }
}

impl Default for Zamia {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zamia {
        Zamia::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_crozier() {
        let z = z();
        assert_eq!(z.frond, 0.0);
        assert!(z.is_crozier());
        assert!(!z.is_spread());
    }

    #[test]
    fn new_clamps_max_frond() {
        let z = Zamia::new(-5.0, 1.0);
        assert!((z.max_frond - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_grow_rate() {
        let z = Zamia::new(100.0, -1.0);
        assert_eq!(z.grow_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zamia::default();
        assert!((z.max_frond - 100.0).abs() < 1e-5);
        assert!((z.grow_rate - 1.0).abs() < 1e-5);
    }

    // --- unfurl ---

    #[test]
    fn unfurl_adds_frond() {
        let mut z = z();
        z.unfurl(40.0);
        assert!((z.frond - 40.0).abs() < 1e-3);
    }

    #[test]
    fn unfurl_clamps_at_max() {
        let mut z = z();
        z.unfurl(200.0);
        assert!((z.frond - 100.0).abs() < 1e-3);
    }

    #[test]
    fn unfurl_fires_just_spread_at_max() {
        let mut z = z();
        z.unfurl(100.0);
        assert!(z.just_spread);
        assert!(z.is_spread());
    }

    #[test]
    fn unfurl_no_just_spread_when_already_at_max() {
        let mut z = z();
        z.frond = 100.0;
        z.unfurl(10.0);
        assert!(!z.just_spread);
    }

    #[test]
    fn unfurl_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.unfurl(50.0);
        assert_eq!(z.frond, 0.0);
    }

    #[test]
    fn unfurl_no_op_when_amount_zero() {
        let mut z = z();
        z.unfurl(0.0);
        assert_eq!(z.frond, 0.0);
    }

    // --- curl ---

    #[test]
    fn curl_reduces_frond() {
        let mut z = z();
        z.frond = 60.0;
        z.curl(20.0);
        assert!((z.frond - 40.0).abs() < 1e-3);
    }

    #[test]
    fn curl_clamps_at_zero() {
        let mut z = z();
        z.frond = 30.0;
        z.curl(200.0);
        assert_eq!(z.frond, 0.0);
    }

    #[test]
    fn curl_fires_just_crozier_at_zero() {
        let mut z = z();
        z.frond = 30.0;
        z.curl(30.0);
        assert!(z.just_crozier);
    }

    #[test]
    fn curl_no_op_when_already_crozier() {
        let mut z = z();
        z.curl(10.0);
        assert!(!z.just_crozier);
    }

    #[test]
    fn curl_no_op_when_disabled() {
        let mut z = z();
        z.frond = 50.0;
        z.enabled = false;
        z.curl(50.0);
        assert!((z.frond - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_grows_frond() {
        let mut z = z(); // rate=1
        z.tick(5.0); // 0 + 1*5 = 5
        assert!((z.frond - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_spread_on_grow_to_max() {
        let mut z = Zamia::new(100.0, 200.0);
        z.frond = 95.0;
        z.tick(1.0);
        assert!(z.just_spread);
        assert!(z.is_spread());
    }

    #[test]
    fn tick_no_grow_when_already_spread() {
        let mut z = z();
        z.frond = 100.0;
        z.tick(1.0);
        assert!(!z.just_spread);
    }

    #[test]
    fn tick_no_grow_when_rate_zero() {
        let mut z = Zamia::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.frond, 0.0);
    }

    #[test]
    fn tick_no_grow_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.frond, 0.0);
    }

    #[test]
    fn tick_clears_just_spread() {
        let mut z = Zamia::new(100.0, 200.0);
        z.frond = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_spread);
    }

    #[test]
    fn tick_clears_just_crozier() {
        let mut z = z();
        z.frond = 10.0;
        z.curl(10.0);
        z.tick(0.016);
        assert!(!z.just_crozier);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(9.0); // 1*9 = 9
        assert!((z.frond - 9.0).abs() < 1e-3);
    }

    // --- is_spread / is_crozier ---

    #[test]
    fn is_spread_false_when_disabled() {
        let mut z = z();
        z.frond = 100.0;
        z.enabled = false;
        assert!(!z.is_spread());
    }

    #[test]
    fn is_crozier_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_crozier());
    }

    // --- frond_fraction / effective_canopy ---

    #[test]
    fn frond_fraction_zero_when_crozier() {
        assert_eq!(z().frond_fraction(), 0.0);
    }

    #[test]
    fn frond_fraction_half_at_midpoint() {
        let mut z = z();
        z.frond = 50.0;
        assert!((z.frond_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_canopy_zero_when_crozier() {
        assert_eq!(z().effective_canopy(100.0), 0.0);
    }

    #[test]
    fn effective_canopy_scales_with_frond() {
        let mut z = z();
        z.frond = 75.0;
        assert!((z.effective_canopy(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_canopy_zero_when_disabled() {
        let mut z = z();
        z.frond = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_canopy(100.0), 0.0);
    }
}
