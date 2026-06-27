use bevy_ecs::prelude::Component;

/// Faunal-zone saturation tracker. `fauna` builds via `populate(amount)`
/// and accumulates passively at `colonize_rate` per second in `tick(dt)` or
/// is depleted via `extirpate(amount)`.
///
/// Models biological-zone saturation bars, fauna-density fill levels,
/// wildlife-population carrying-capacity meters, animal-habitat coverage
/// trackers, ecological niche-occupancy saturation gauges, vertebrate-range
/// fill levels, migratory-corridor density indicators, megafauna-presence
/// intensity bars, biodiversity saturation trackers, or any mechanic where
/// creatures slowly colonize every available niche in a region until the
/// habitat reaches ecological carrying capacity — right up until a hunting
/// pressure, disease outbreak, or habitat loss event extirpates the
/// population back toward functional extinction.
///
/// `populate(amount)` adds fauna; fires `just_saturated` when first
/// reaching `max_fauna`. No-op when disabled.
///
/// `extirpate(amount)` reduces fauna immediately; fires `just_empty`
/// when reaching 0. No-op when disabled or already empty.
///
/// `tick(dt)` clears both flags, then increases fauna by
/// `colonize_rate * dt` (capped at `max_fauna`). Fires `just_saturated`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_saturated()` returns `fauna >= max_fauna && enabled`.
///
/// `is_empty()` returns `fauna == 0.0` (not gated by `enabled`).
///
/// `fauna_fraction()` returns `(fauna / max_fauna).clamp(0, 1)`.
///
/// `effective_biodiversity(scale)` returns `scale * fauna_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — colonizes at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoosphere {
    pub fauna: f32,
    pub max_fauna: f32,
    pub colonize_rate: f32,
    pub just_saturated: bool,
    pub just_empty: bool,
    pub enabled: bool,
}

impl Zoosphere {
    pub fn new(max_fauna: f32, colonize_rate: f32) -> Self {
        Self {
            fauna: 0.0,
            max_fauna: max_fauna.max(0.1),
            colonize_rate: colonize_rate.max(0.0),
            just_saturated: false,
            just_empty: false,
            enabled: true,
        }
    }

    /// Add fauna; fires `just_saturated` when first reaching max.
    /// No-op when disabled.
    pub fn populate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.fauna < self.max_fauna;
        self.fauna = (self.fauna + amount).min(self.max_fauna);
        if was_below && self.fauna >= self.max_fauna {
            self.just_saturated = true;
        }
    }

    /// Reduce fauna; fires `just_empty` when reaching 0.
    /// No-op when disabled or already empty.
    pub fn extirpate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fauna <= 0.0 {
            return;
        }
        self.fauna = (self.fauna - amount).max(0.0);
        if self.fauna <= 0.0 {
            self.just_empty = true;
        }
    }

    /// Clear flags, then increase fauna by `colonize_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_saturated = false;
        self.just_empty = false;
        if self.enabled && self.colonize_rate > 0.0 && self.fauna < self.max_fauna {
            let was_below = self.fauna < self.max_fauna;
            self.fauna = (self.fauna + self.colonize_rate * dt).min(self.max_fauna);
            if was_below && self.fauna >= self.max_fauna {
                self.just_saturated = true;
            }
        }
    }

    /// `true` when fauna is at maximum and component is enabled.
    pub fn is_saturated(&self) -> bool {
        self.fauna >= self.max_fauna && self.enabled
    }

    /// `true` when fauna is 0 (not gated by `enabled`).
    pub fn is_empty(&self) -> bool {
        self.fauna == 0.0
    }

    /// Fraction of maximum fauna [0.0, 1.0].
    pub fn fauna_fraction(&self) -> f32 {
        (self.fauna / self.max_fauna).clamp(0.0, 1.0)
    }

    /// Returns `scale * fauna_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_biodiversity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.fauna_fraction()
    }
}

impl Default for Zoosphere {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoosphere {
        Zoosphere::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let z = z();
        assert_eq!(z.fauna, 0.0);
        assert!(z.is_empty());
        assert!(!z.is_saturated());
    }

    #[test]
    fn new_clamps_max_fauna() {
        let z = Zoosphere::new(-5.0, 1.0);
        assert!((z.max_fauna - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_colonize_rate() {
        let z = Zoosphere::new(100.0, -1.0);
        assert_eq!(z.colonize_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoosphere::default();
        assert!((z.max_fauna - 100.0).abs() < 1e-5);
        assert!((z.colonize_rate - 1.0).abs() < 1e-5);
    }

    // --- populate ---

    #[test]
    fn populate_adds_fauna() {
        let mut z = z();
        z.populate(40.0);
        assert!((z.fauna - 40.0).abs() < 1e-3);
    }

    #[test]
    fn populate_clamps_at_max() {
        let mut z = z();
        z.populate(200.0);
        assert!((z.fauna - 100.0).abs() < 1e-3);
    }

    #[test]
    fn populate_fires_just_saturated_at_max() {
        let mut z = z();
        z.populate(100.0);
        assert!(z.just_saturated);
        assert!(z.is_saturated());
    }

    #[test]
    fn populate_no_just_saturated_when_already_at_max() {
        let mut z = z();
        z.fauna = 100.0;
        z.populate(10.0);
        assert!(!z.just_saturated);
    }

    #[test]
    fn populate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.populate(50.0);
        assert_eq!(z.fauna, 0.0);
    }

    #[test]
    fn populate_no_op_when_amount_zero() {
        let mut z = z();
        z.populate(0.0);
        assert_eq!(z.fauna, 0.0);
    }

    // --- extirpate ---

    #[test]
    fn extirpate_reduces_fauna() {
        let mut z = z();
        z.fauna = 60.0;
        z.extirpate(20.0);
        assert!((z.fauna - 40.0).abs() < 1e-3);
    }

    #[test]
    fn extirpate_clamps_at_zero() {
        let mut z = z();
        z.fauna = 30.0;
        z.extirpate(200.0);
        assert_eq!(z.fauna, 0.0);
    }

    #[test]
    fn extirpate_fires_just_empty_at_zero() {
        let mut z = z();
        z.fauna = 30.0;
        z.extirpate(30.0);
        assert!(z.just_empty);
    }

    #[test]
    fn extirpate_no_op_when_already_empty() {
        let mut z = z();
        z.extirpate(10.0);
        assert!(!z.just_empty);
    }

    #[test]
    fn extirpate_no_op_when_disabled() {
        let mut z = z();
        z.fauna = 50.0;
        z.enabled = false;
        z.extirpate(50.0);
        assert!((z.fauna - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_colonizes_fauna() {
        let mut z = z(); // rate=1
        z.tick(7.0); // 0 + 1*7 = 7
        assert!((z.fauna - 7.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_saturated_on_colonize_to_max() {
        let mut z = Zoosphere::new(100.0, 200.0);
        z.fauna = 95.0;
        z.tick(1.0);
        assert!(z.just_saturated);
        assert!(z.is_saturated());
    }

    #[test]
    fn tick_no_colonize_when_already_saturated() {
        let mut z = z();
        z.fauna = 100.0;
        z.tick(1.0);
        assert!(!z.just_saturated);
    }

    #[test]
    fn tick_no_colonize_when_rate_zero() {
        let mut z = Zoosphere::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.fauna, 0.0);
    }

    #[test]
    fn tick_no_colonize_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.fauna, 0.0);
    }

    #[test]
    fn tick_clears_just_saturated() {
        let mut z = Zoosphere::new(100.0, 200.0);
        z.fauna = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_saturated);
    }

    #[test]
    fn tick_clears_just_empty() {
        let mut z = z();
        z.fauna = 10.0;
        z.extirpate(10.0);
        z.tick(0.016);
        assert!(!z.just_empty);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(9.0); // 1*9 = 9
        assert!((z.fauna - 9.0).abs() < 1e-3);
    }

    // --- is_saturated / is_empty ---

    #[test]
    fn is_saturated_false_when_disabled() {
        let mut z = z();
        z.fauna = 100.0;
        z.enabled = false;
        assert!(!z.is_saturated());
    }

    #[test]
    fn is_empty_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_empty());
    }

    // --- fauna_fraction / effective_biodiversity ---

    #[test]
    fn fauna_fraction_zero_when_empty() {
        assert_eq!(z().fauna_fraction(), 0.0);
    }

    #[test]
    fn fauna_fraction_half_at_midpoint() {
        let mut z = z();
        z.fauna = 50.0;
        assert!((z.fauna_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_biodiversity_zero_when_empty() {
        assert_eq!(z().effective_biodiversity(100.0), 0.0);
    }

    #[test]
    fn effective_biodiversity_scales_with_fauna() {
        let mut z = z();
        z.fauna = 75.0;
        assert!((z.effective_biodiversity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_biodiversity_zero_when_disabled() {
        let mut z = z();
        z.fauna = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_biodiversity(100.0), 0.0);
    }
}
