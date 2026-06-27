use bevy_ecs::prelude::Component;

/// Bloom-cycle tracker. `petals` builds via `unfurl(amount)` and opens
/// passively at `bloom_rate` per second in `tick(dt)` or wilts immediately
/// via `wilt(amount)`.
///
/// Models flower-petal unfurl fill levels, garden-bed flowering intensity
/// gauges, blossom-saturation accumulation bars, ornamental-bloom cycle
/// trackers, daisy-chain progress indicators, cut-flower freshness meters,
/// pollinator-attraction fill levels, botanical-display health bars,
/// horticultural-peak saturation trackers, or any mechanic where a
/// cheerful disc of brightly coloured ray florets opens petal by petal
/// in relentless calendar-defying optimism until drought or early frost
/// turns the whole affair brown and brittle in a single afternoon.
///
/// `unfurl(amount)` adds petals; fires `just_blooming` when first
/// reaching `max_petals`. No-op when disabled.
///
/// `wilt(amount)` reduces petals immediately; fires `just_wilted`
/// when reaching 0. No-op when disabled or already wilted.
///
/// `tick(dt)` clears both flags, then increases petals by
/// `bloom_rate * dt` (capped at `max_petals`). Fires `just_blooming`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_blooming()` returns `petals >= max_petals && enabled`.
///
/// `is_wilted()` returns `petals == 0.0` (not gated by `enabled`).
///
/// `petal_fraction()` returns `(petals / max_petals).clamp(0, 1)`.
///
/// `effective_fragrance(scale)` returns `scale * petal_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — blooms at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zinnia {
    pub petals: f32,
    pub max_petals: f32,
    pub bloom_rate: f32,
    pub just_blooming: bool,
    pub just_wilted: bool,
    pub enabled: bool,
}

impl Zinnia {
    pub fn new(max_petals: f32, bloom_rate: f32) -> Self {
        Self {
            petals: 0.0,
            max_petals: max_petals.max(0.1),
            bloom_rate: bloom_rate.max(0.0),
            just_blooming: false,
            just_wilted: false,
            enabled: true,
        }
    }

    /// Add petals; fires `just_blooming` when first reaching max.
    /// No-op when disabled.
    pub fn unfurl(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.petals < self.max_petals;
        self.petals = (self.petals + amount).min(self.max_petals);
        if was_below && self.petals >= self.max_petals {
            self.just_blooming = true;
        }
    }

    /// Reduce petals; fires `just_wilted` when reaching 0.
    /// No-op when disabled or already wilted.
    pub fn wilt(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.petals <= 0.0 {
            return;
        }
        self.petals = (self.petals - amount).max(0.0);
        if self.petals <= 0.0 {
            self.just_wilted = true;
        }
    }

    /// Clear flags, then increase petals by `bloom_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_blooming = false;
        self.just_wilted = false;
        if self.enabled && self.bloom_rate > 0.0 && self.petals < self.max_petals {
            let was_below = self.petals < self.max_petals;
            self.petals = (self.petals + self.bloom_rate * dt).min(self.max_petals);
            if was_below && self.petals >= self.max_petals {
                self.just_blooming = true;
            }
        }
    }

    /// `true` when petals are at maximum and component is enabled.
    pub fn is_blooming(&self) -> bool {
        self.petals >= self.max_petals && self.enabled
    }

    /// `true` when petals are 0 (not gated by `enabled`).
    pub fn is_wilted(&self) -> bool {
        self.petals == 0.0
    }

    /// Fraction of maximum petals [0.0, 1.0].
    pub fn petal_fraction(&self) -> f32 {
        (self.petals / self.max_petals).clamp(0.0, 1.0)
    }

    /// Returns `scale * petal_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_fragrance(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.petal_fraction()
    }
}

impl Default for Zinnia {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zinnia {
        Zinnia::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_wilted() {
        let z = z();
        assert_eq!(z.petals, 0.0);
        assert!(z.is_wilted());
        assert!(!z.is_blooming());
    }

    #[test]
    fn new_clamps_max_petals() {
        let z = Zinnia::new(-5.0, 2.0);
        assert!((z.max_petals - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_bloom_rate() {
        let z = Zinnia::new(100.0, -2.0);
        assert_eq!(z.bloom_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zinnia::default();
        assert!((z.max_petals - 100.0).abs() < 1e-5);
        assert!((z.bloom_rate - 2.0).abs() < 1e-5);
    }

    // --- unfurl ---

    #[test]
    fn unfurl_adds_petals() {
        let mut z = z();
        z.unfurl(40.0);
        assert!((z.petals - 40.0).abs() < 1e-3);
    }

    #[test]
    fn unfurl_clamps_at_max() {
        let mut z = z();
        z.unfurl(200.0);
        assert!((z.petals - 100.0).abs() < 1e-3);
    }

    #[test]
    fn unfurl_fires_just_blooming_at_max() {
        let mut z = z();
        z.unfurl(100.0);
        assert!(z.just_blooming);
        assert!(z.is_blooming());
    }

    #[test]
    fn unfurl_no_just_blooming_when_already_at_max() {
        let mut z = z();
        z.petals = 100.0;
        z.unfurl(10.0);
        assert!(!z.just_blooming);
    }

    #[test]
    fn unfurl_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.unfurl(50.0);
        assert_eq!(z.petals, 0.0);
    }

    #[test]
    fn unfurl_no_op_when_amount_zero() {
        let mut z = z();
        z.unfurl(0.0);
        assert_eq!(z.petals, 0.0);
    }

    // --- wilt ---

    #[test]
    fn wilt_reduces_petals() {
        let mut z = z();
        z.petals = 60.0;
        z.wilt(20.0);
        assert!((z.petals - 40.0).abs() < 1e-3);
    }

    #[test]
    fn wilt_clamps_at_zero() {
        let mut z = z();
        z.petals = 30.0;
        z.wilt(200.0);
        assert_eq!(z.petals, 0.0);
    }

    #[test]
    fn wilt_fires_just_wilted_at_zero() {
        let mut z = z();
        z.petals = 30.0;
        z.wilt(30.0);
        assert!(z.just_wilted);
    }

    #[test]
    fn wilt_no_op_when_already_wilted() {
        let mut z = z();
        z.wilt(10.0);
        assert!(!z.just_wilted);
    }

    #[test]
    fn wilt_no_op_when_disabled() {
        let mut z = z();
        z.petals = 50.0;
        z.enabled = false;
        z.wilt(50.0);
        assert!((z.petals - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_blooms_petals() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.petals - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_blooming_on_unfurl_to_max() {
        let mut z = Zinnia::new(100.0, 200.0);
        z.petals = 95.0;
        z.tick(1.0);
        assert!(z.just_blooming);
        assert!(z.is_blooming());
    }

    #[test]
    fn tick_no_bloom_when_already_blooming() {
        let mut z = z();
        z.petals = 100.0;
        z.tick(1.0);
        assert!(!z.just_blooming);
    }

    #[test]
    fn tick_no_bloom_when_rate_zero() {
        let mut z = Zinnia::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.petals, 0.0);
    }

    #[test]
    fn tick_no_bloom_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.petals, 0.0);
    }

    #[test]
    fn tick_clears_just_blooming() {
        let mut z = Zinnia::new(100.0, 200.0);
        z.petals = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_blooming);
    }

    #[test]
    fn tick_clears_just_wilted() {
        let mut z = z();
        z.petals = 10.0;
        z.wilt(10.0);
        z.tick(0.016);
        assert!(!z.just_wilted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.petals - 10.0).abs() < 1e-3);
    }

    // --- is_blooming / is_wilted ---

    #[test]
    fn is_blooming_false_when_disabled() {
        let mut z = z();
        z.petals = 100.0;
        z.enabled = false;
        assert!(!z.is_blooming());
    }

    #[test]
    fn is_wilted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_wilted());
    }

    // --- petal_fraction / effective_fragrance ---

    #[test]
    fn petal_fraction_zero_when_wilted() {
        assert_eq!(z().petal_fraction(), 0.0);
    }

    #[test]
    fn petal_fraction_half_at_midpoint() {
        let mut z = z();
        z.petals = 50.0;
        assert!((z.petal_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_fragrance_zero_when_wilted() {
        assert_eq!(z().effective_fragrance(100.0), 0.0);
    }

    #[test]
    fn effective_fragrance_scales_with_petals() {
        let mut z = z();
        z.petals = 75.0;
        assert!((z.effective_fragrance(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_fragrance_zero_when_disabled() {
        let mut z = z();
        z.petals = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_fragrance(100.0), 0.0);
    }
}
