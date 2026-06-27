use bevy_ecs::prelude::Component;

/// Citrus-zest scraper tracker. `scraped` builds via `grate(amount)` and
/// accumulates passively at `grate_rate` per second in `tick(dt)` or is
/// discarded immediately via `discard(amount)`.
///
/// Models kitchen-prep progress bars, citrus-oil extraction fill levels,
/// aromatic-compound harvesting trackers, rind-layer scraping gauges,
/// flavor-concentration accumulators, culinary-craft intensity indicators,
/// ingredient-preparation completion meters, chef-skill throughput bars,
/// micro-plane efficiency trackers, or any mechanic where patient, rhythmic
/// scraping strips a thin fragrant layer from the outer surface of something
/// bright and waxy until every last volatile oil has been captured before
/// the white pith below grows bitter.
///
/// `grate(amount)` adds scraped; fires `just_zested` when first reaching
/// `max_scraped`. No-op when disabled.
///
/// `discard(amount)` reduces scraped immediately; fires `just_depleted`
/// when reaching 0. No-op when disabled or already depleted.
///
/// `tick(dt)` clears both flags, then increases scraped by
/// `grate_rate * dt` (capped at `max_scraped`). Fires `just_zested`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_zested()` returns `scraped >= max_scraped && enabled`.
///
/// `is_depleted()` returns `scraped == 0.0` (not gated by `enabled`).
///
/// `scraped_fraction()` returns `(scraped / max_scraped).clamp(0, 1)`.
///
/// `effective_aroma(scale)` returns `scale * scraped_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — grates at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zester {
    pub scraped: f32,
    pub max_scraped: f32,
    pub grate_rate: f32,
    pub just_zested: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Zester {
    pub fn new(max_scraped: f32, grate_rate: f32) -> Self {
        Self {
            scraped: 0.0,
            max_scraped: max_scraped.max(0.1),
            grate_rate: grate_rate.max(0.0),
            just_zested: false,
            just_depleted: false,
            enabled: true,
        }
    }

    /// Add scraped; fires `just_zested` when first reaching max.
    /// No-op when disabled.
    pub fn grate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.scraped < self.max_scraped;
        self.scraped = (self.scraped + amount).min(self.max_scraped);
        if was_below && self.scraped >= self.max_scraped {
            self.just_zested = true;
        }
    }

    /// Reduce scraped; fires `just_depleted` when reaching 0.
    /// No-op when disabled or already depleted.
    pub fn discard(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.scraped <= 0.0 {
            return;
        }
        self.scraped = (self.scraped - amount).max(0.0);
        if self.scraped <= 0.0 {
            self.just_depleted = true;
        }
    }

    /// Clear flags, then increase scraped by `grate_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_zested = false;
        self.just_depleted = false;
        if self.enabled && self.grate_rate > 0.0 && self.scraped < self.max_scraped {
            let was_below = self.scraped < self.max_scraped;
            self.scraped = (self.scraped + self.grate_rate * dt).min(self.max_scraped);
            if was_below && self.scraped >= self.max_scraped {
                self.just_zested = true;
            }
        }
    }

    /// `true` when scraped is at maximum and component is enabled.
    pub fn is_zested(&self) -> bool {
        self.scraped >= self.max_scraped && self.enabled
    }

    /// `true` when scraped is 0 (not gated by `enabled`).
    pub fn is_depleted(&self) -> bool {
        self.scraped == 0.0
    }

    /// Fraction of maximum scraped [0.0, 1.0].
    pub fn scraped_fraction(&self) -> f32 {
        (self.scraped / self.max_scraped).clamp(0.0, 1.0)
    }

    /// Returns `scale * scraped_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_aroma(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.scraped_fraction()
    }
}

impl Default for Zester {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zester {
        Zester::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_depleted() {
        let z = z();
        assert_eq!(z.scraped, 0.0);
        assert!(z.is_depleted());
        assert!(!z.is_zested());
    }

    #[test]
    fn new_clamps_max_scraped() {
        let z = Zester::new(-5.0, 4.0);
        assert!((z.max_scraped - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_grate_rate() {
        let z = Zester::new(100.0, -3.0);
        assert_eq!(z.grate_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zester::default();
        assert!((z.max_scraped - 100.0).abs() < 1e-5);
        assert!((z.grate_rate - 4.0).abs() < 1e-5);
    }

    // --- grate ---

    #[test]
    fn grate_adds_scraped() {
        let mut z = z();
        z.grate(40.0);
        assert!((z.scraped - 40.0).abs() < 1e-3);
    }

    #[test]
    fn grate_clamps_at_max() {
        let mut z = z();
        z.grate(200.0);
        assert!((z.scraped - 100.0).abs() < 1e-3);
    }

    #[test]
    fn grate_fires_just_zested_at_max() {
        let mut z = z();
        z.grate(100.0);
        assert!(z.just_zested);
        assert!(z.is_zested());
    }

    #[test]
    fn grate_no_just_zested_when_already_at_max() {
        let mut z = z();
        z.scraped = 100.0;
        z.grate(10.0);
        assert!(!z.just_zested);
    }

    #[test]
    fn grate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.grate(50.0);
        assert_eq!(z.scraped, 0.0);
    }

    #[test]
    fn grate_no_op_when_amount_zero() {
        let mut z = z();
        z.grate(0.0);
        assert_eq!(z.scraped, 0.0);
    }

    // --- discard ---

    #[test]
    fn discard_reduces_scraped() {
        let mut z = z();
        z.scraped = 60.0;
        z.discard(20.0);
        assert!((z.scraped - 40.0).abs() < 1e-3);
    }

    #[test]
    fn discard_clamps_at_zero() {
        let mut z = z();
        z.scraped = 30.0;
        z.discard(200.0);
        assert_eq!(z.scraped, 0.0);
    }

    #[test]
    fn discard_fires_just_depleted_at_zero() {
        let mut z = z();
        z.scraped = 30.0;
        z.discard(30.0);
        assert!(z.just_depleted);
    }

    #[test]
    fn discard_no_op_when_already_depleted() {
        let mut z = z();
        z.discard(10.0);
        assert!(!z.just_depleted);
    }

    #[test]
    fn discard_no_op_when_disabled() {
        let mut z = z();
        z.scraped = 50.0;
        z.enabled = false;
        z.discard(50.0);
        assert!((z.scraped - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_grates_scraped() {
        let mut z = z(); // rate=4
        z.tick(2.0); // 0 + 4*2 = 8
        assert!((z.scraped - 8.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_zested_on_grate_to_max() {
        let mut z = Zester::new(100.0, 200.0);
        z.scraped = 95.0;
        z.tick(1.0);
        assert!(z.just_zested);
        assert!(z.is_zested());
    }

    #[test]
    fn tick_no_grate_when_already_zested() {
        let mut z = z();
        z.scraped = 100.0;
        z.tick(1.0);
        assert!(!z.just_zested);
    }

    #[test]
    fn tick_no_grate_when_rate_zero() {
        let mut z = Zester::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.scraped, 0.0);
    }

    #[test]
    fn tick_no_grate_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.scraped, 0.0);
    }

    #[test]
    fn tick_clears_just_zested() {
        let mut z = Zester::new(100.0, 200.0);
        z.scraped = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_zested);
    }

    #[test]
    fn tick_clears_just_depleted() {
        let mut z = z();
        z.scraped = 10.0;
        z.discard(10.0);
        z.tick(0.016);
        assert!(!z.just_depleted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=4
        z.tick(3.0); // 4*3 = 12
        assert!((z.scraped - 12.0).abs() < 1e-3);
    }

    // --- is_zested / is_depleted ---

    #[test]
    fn is_zested_false_when_disabled() {
        let mut z = z();
        z.scraped = 100.0;
        z.enabled = false;
        assert!(!z.is_zested());
    }

    #[test]
    fn is_depleted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_depleted());
    }

    // --- scraped_fraction / effective_aroma ---

    #[test]
    fn scraped_fraction_zero_when_depleted() {
        assert_eq!(z().scraped_fraction(), 0.0);
    }

    #[test]
    fn scraped_fraction_half_at_midpoint() {
        let mut z = z();
        z.scraped = 50.0;
        assert!((z.scraped_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_aroma_zero_when_depleted() {
        assert_eq!(z().effective_aroma(100.0), 0.0);
    }

    #[test]
    fn effective_aroma_scales_with_scraped() {
        let mut z = z();
        z.scraped = 75.0;
        assert!((z.effective_aroma(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_aroma_zero_when_disabled() {
        let mut z = z();
        z.scraped = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_aroma(100.0), 0.0);
    }
}
