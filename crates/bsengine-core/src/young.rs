use bevy_ecs::prelude::Component;

/// Monotonically-decreasing youth bonus. Starts full and ages passively each
/// tick; fires `just_matured` once when depleted. Models early-stage buffs,
/// freshness bonuses, or developmental windows that expire over time.
///
/// The output curve is the inverse of `Wiz` (which grows upward): Young
/// starts at 2× and decays to 1× as youth depletes. Unlike `Woo` or `Zest`
/// (both replenishable), Young's natural direction is always downward — only
/// `revitalize()` resets it, modeling deliberate intervention ("fountain of
/// youth" mechanics).
///
/// `tick(dt)` clears one-frame flags first, then if enabled and
/// `young_level > 0`: subtracts `age_rate * dt`, floors at 0. Fires
/// `just_matured` the first time `young_level` reaches 0. No-op (beyond
/// flag clear) when disabled.
///
/// `revitalize()` resets `young_level` to `max_young`. No-op when already
/// at max.
///
/// `is_young()` returns `young_level > 0.0 && enabled`.
///
/// `is_matured()` returns `young_level == 0.0 && enabled`.
///
/// `youth_fraction()` returns `(young_level / max_young).clamp(0.0, 1.0)`.
///
/// `effective_vitality(base)` returns `base * (1.0 + youth_fraction())` when
/// enabled — 2× when freshly young, 1× when fully matured; `base` when
/// disabled.
///
/// Default: `new(60.0, 1.0)` — full youth for 60 seconds at rate 1/s.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Young {
    /// Current youth level [0, max_young]. Starts at `max_young`.
    pub young_level: f32,
    /// Maximum (initial) youth. Clamped >= 1.0.
    pub max_young: f32,
    /// Aging rate in youth-units/second. Clamped >= 0.0.
    pub age_rate: f32,
    pub just_matured: bool,
    pub enabled: bool,
}

impl Young {
    pub fn new(max_young: f32, age_rate: f32) -> Self {
        let max_young = max_young.max(1.0);
        Self {
            young_level: max_young,
            max_young,
            age_rate: age_rate.max(0.0),
            just_matured: false,
            enabled: true,
        }
    }

    /// Reset youth to maximum. No-op when already full.
    pub fn revitalize(&mut self) {
        if self.young_level >= self.max_young {
            return;
        }
        self.young_level = self.max_young;
    }

    /// Advance one frame: clear flags, then age. Fires `just_matured` when
    /// reaching 0. No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_matured = false;

        if !self.enabled || self.young_level == 0.0 {
            return;
        }

        self.young_level = (self.young_level - self.age_rate * dt).max(0.0);
        if self.young_level == 0.0 {
            self.just_matured = true;
        }
    }

    /// `true` when youth remains and component is enabled.
    pub fn is_young(&self) -> bool {
        self.young_level > 0.0 && self.enabled
    }

    /// `true` when youth is fully depleted and component is enabled.
    pub fn is_matured(&self) -> bool {
        self.young_level == 0.0 && self.enabled
    }

    /// Youth remaining as a fraction of maximum [0.0, 1.0].
    pub fn youth_fraction(&self) -> f32 {
        (self.young_level / self.max_young).clamp(0.0, 1.0)
    }

    /// Scale `base` by remaining youth. Returns `base * (1.0 + youth_fraction())`
    /// when enabled — 2× at full youth, 1× at matured; `base` when disabled.
    pub fn effective_vitality(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.youth_fraction())
    }
}

impl Default for Young {
    fn default() -> Self {
        Self::new(60.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Young {
        Young::new(10.0, 1.0) // max=10, age 1/s
    }

    #[test]
    fn new_starts_at_max() {
        let y = y();
        assert!((y.young_level - 10.0).abs() < 1e-5);
        assert!(!y.just_matured);
        assert!(y.is_young());
        assert!(!y.is_matured());
    }

    // --- tick ---

    #[test]
    fn tick_ages_passively() {
        let mut y = y(); // age_rate=1/s
        y.tick(3.0); // 10 - 3 = 7
        assert!((y.young_level - 7.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_matured_at_zero() {
        let mut y = y();
        y.tick(10.0); // exactly reaches 0
        assert!(y.just_matured);
        assert_eq!(y.young_level, 0.0);
    }

    #[test]
    fn tick_fires_just_matured_crossing_zero() {
        let mut y = y();
        y.tick(7.0); // 3.0
        y.tick(5.0); // crosses 0
        assert!(y.just_matured);
        assert_eq!(y.young_level, 0.0);
    }

    #[test]
    fn tick_just_matured_clears_next_frame() {
        let mut y = y();
        y.tick(10.0); // just_matured=true
        y.tick(0.016);
        assert!(!y.just_matured);
    }

    #[test]
    fn tick_no_op_when_already_matured() {
        let mut y = y();
        y.tick(10.0); // reach 0
        y.tick(10.0); // no-op
        assert_eq!(y.young_level, 0.0);
        assert!(!y.just_matured); // flags cleared, not re-fired
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.tick(10.0);
        assert!((y.young_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut y = y();
        y.just_matured = true;
        y.enabled = false;
        y.tick(0.016);
        assert!(!y.just_matured);
    }

    // --- revitalize ---

    #[test]
    fn revitalize_resets_to_max() {
        let mut y = y();
        y.tick(7.0); // 3.0
        y.revitalize();
        assert!((y.young_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn revitalize_from_fully_matured() {
        let mut y = y();
        y.tick(10.0); // 0.0
        y.revitalize();
        assert!((y.young_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn revitalize_no_op_when_already_full() {
        let mut y = y(); // starts at max
        y.revitalize();
        assert!((y.young_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn revitalize_allows_reaging() {
        let mut y = y();
        y.tick(10.0); // fully matured
        y.revitalize();
        y.tick(5.0); // age again
        assert!((y.young_level - 5.0).abs() < 1e-4);
    }

    // --- is_young / is_matured ---

    #[test]
    fn is_young_true_when_fresh() {
        let y = y();
        assert!(y.is_young());
    }

    #[test]
    fn is_young_false_when_matured() {
        let mut y = y();
        y.tick(10.0);
        assert!(!y.is_young());
    }

    #[test]
    fn is_young_false_when_disabled() {
        let y_disabled = {
            let mut y = y();
            y.enabled = false;
            y
        };
        assert!(!y_disabled.is_young());
    }

    #[test]
    fn is_matured_false_when_fresh() {
        let y = y();
        assert!(!y.is_matured());
    }

    #[test]
    fn is_matured_true_when_depleted() {
        let mut y = y();
        y.tick(10.0);
        assert!(y.is_matured());
    }

    #[test]
    fn is_matured_false_when_disabled() {
        let mut y = y();
        y.tick(10.0);
        y.enabled = false;
        assert!(!y.is_matured());
    }

    // --- youth_fraction ---

    #[test]
    fn youth_fraction_one_when_fresh() {
        let y = y();
        assert!((y.youth_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn youth_fraction_at_half() {
        let mut y = y(); // max=10
        y.tick(5.0); // 5/10=0.5
        assert!((y.youth_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn youth_fraction_zero_when_matured() {
        let mut y = y();
        y.tick(10.0);
        assert_eq!(y.youth_fraction(), 0.0);
    }

    // --- effective_vitality ---

    #[test]
    fn effective_vitality_doubled_when_fresh() {
        let y = y(); // fraction=1.0 → 100*(1+1)=200
        assert!((y.effective_vitality(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vitality_at_half_youth() {
        let mut y = y();
        y.tick(5.0); // fraction=0.5 → 100*(1+0.5)=150
        assert!((y.effective_vitality(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vitality_passthrough_when_matured() {
        let mut y = y();
        y.tick(10.0); // fraction=0 → 100*(1+0)=100
        assert!((y.effective_vitality(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vitality_passthrough_when_disabled() {
        let y_disabled = {
            let mut y = y();
            y.enabled = false;
            y
        };
        assert!((y_disabled.effective_vitality(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_young_clamped_to_one() {
        let y = Young::new(0.0, 1.0);
        assert!((y.max_young - 1.0).abs() < 1e-5);
        assert!((y.young_level - 1.0).abs() < 1e-5);
    }

    #[test]
    fn age_rate_clamped_to_zero() {
        let y = Young::new(10.0, -1.0);
        assert_eq!(y.age_rate, 0.0);
    }

    #[test]
    fn zero_age_rate_never_matures() {
        let mut y = Young::new(10.0, 0.0);
        y.tick(100.0);
        assert!((y.young_level - 10.0).abs() < 1e-4);
        assert!(!y.just_matured);
    }
}
