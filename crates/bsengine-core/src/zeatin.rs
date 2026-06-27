use bevy_ecs::prelude::Component;

/// Cell-division stimulus tracker. `division` builds via `stimulate(amount)`
/// and increases passively at `proliferate_rate` per second in `tick(dt)`
/// or is halted immediately via `arrest(amount)`.
///
/// Models plant-cytokinin build-up meters, cell-proliferation drive bars,
/// growth-hormone saturation accumulators, meristem-activity gauges,
/// tissue-regeneration signal trackers, wound-healing cascade fill levels,
/// embryonic-growth-factor intensity indicators, callus-induction progress
/// bars, lateral-bud-release signal meters, or any mechanic where a
/// biochemical signal steadily saturates receptors until every dormant
/// cell wakes up and divides — only for a growth-arresting compound to
/// shut the whole cascade down.
///
/// `stimulate(amount)` adds division; fires `just_proliferating` when
/// first reaching `max_division`. No-op when disabled.
///
/// `arrest(amount)` reduces division immediately; fires `just_arrested`
/// when reaching 0. No-op when disabled or already arrested.
///
/// `tick(dt)` clears both flags, then increases division by
/// `proliferate_rate * dt` (capped at `max_division`). Fires
/// `just_proliferating` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_proliferating()` returns `division >= max_division && enabled`.
///
/// `is_arrested()` returns `division == 0.0` (not gated by `enabled`).
///
/// `division_fraction()` returns `(division / max_division).clamp(0, 1)`.
///
/// `effective_growth(scale)` returns `scale * division_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.5)` — proliferates at 2.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeatin {
    pub division: f32,
    pub max_division: f32,
    pub proliferate_rate: f32,
    pub just_proliferating: bool,
    pub just_arrested: bool,
    pub enabled: bool,
}

impl Zeatin {
    pub fn new(max_division: f32, proliferate_rate: f32) -> Self {
        Self {
            division: 0.0,
            max_division: max_division.max(0.1),
            proliferate_rate: proliferate_rate.max(0.0),
            just_proliferating: false,
            just_arrested: false,
            enabled: true,
        }
    }

    /// Add division; fires `just_proliferating` when first reaching max.
    /// No-op when disabled.
    pub fn stimulate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.division < self.max_division;
        self.division = (self.division + amount).min(self.max_division);
        if was_below && self.division >= self.max_division {
            self.just_proliferating = true;
        }
    }

    /// Reduce division; fires `just_arrested` when reaching 0.
    /// No-op when disabled or already arrested.
    pub fn arrest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.division <= 0.0 {
            return;
        }
        self.division = (self.division - amount).max(0.0);
        if self.division <= 0.0 {
            self.just_arrested = true;
        }
    }

    /// Clear flags, then increase division by `proliferate_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_proliferating = false;
        self.just_arrested = false;
        if self.enabled && self.proliferate_rate > 0.0 && self.division < self.max_division {
            let was_below = self.division < self.max_division;
            self.division = (self.division + self.proliferate_rate * dt).min(self.max_division);
            if was_below && self.division >= self.max_division {
                self.just_proliferating = true;
            }
        }
    }

    /// `true` when division is at maximum and component is enabled.
    pub fn is_proliferating(&self) -> bool {
        self.division >= self.max_division && self.enabled
    }

    /// `true` when division is 0 (not gated by `enabled`).
    pub fn is_arrested(&self) -> bool {
        self.division == 0.0
    }

    /// Fraction of maximum division [0.0, 1.0].
    pub fn division_fraction(&self) -> f32 {
        (self.division / self.max_division).clamp(0.0, 1.0)
    }

    /// Returns `scale * division_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_growth(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.division_fraction()
    }
}

impl Default for Zeatin {
    fn default() -> Self {
        Self::new(100.0, 2.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeatin {
        Zeatin::new(100.0, 2.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_arrested() {
        let z = z();
        assert_eq!(z.division, 0.0);
        assert!(z.is_arrested());
        assert!(!z.is_proliferating());
    }

    #[test]
    fn new_clamps_max_division() {
        let z = Zeatin::new(-5.0, 2.5);
        assert!((z.max_division - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_proliferate_rate() {
        let z = Zeatin::new(100.0, -3.0);
        assert_eq!(z.proliferate_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeatin::default();
        assert!((z.max_division - 100.0).abs() < 1e-5);
        assert!((z.proliferate_rate - 2.5).abs() < 1e-5);
    }

    // --- stimulate ---

    #[test]
    fn stimulate_adds_division() {
        let mut z = z();
        z.stimulate(40.0);
        assert!((z.division - 40.0).abs() < 1e-3);
    }

    #[test]
    fn stimulate_clamps_at_max() {
        let mut z = z();
        z.stimulate(200.0);
        assert!((z.division - 100.0).abs() < 1e-3);
    }

    #[test]
    fn stimulate_fires_just_proliferating_at_max() {
        let mut z = z();
        z.stimulate(100.0);
        assert!(z.just_proliferating);
        assert!(z.is_proliferating());
    }

    #[test]
    fn stimulate_no_just_proliferating_when_already_at_max() {
        let mut z = z();
        z.division = 100.0;
        z.stimulate(10.0);
        assert!(!z.just_proliferating);
    }

    #[test]
    fn stimulate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.stimulate(50.0);
        assert_eq!(z.division, 0.0);
    }

    #[test]
    fn stimulate_no_op_when_amount_zero() {
        let mut z = z();
        z.stimulate(0.0);
        assert_eq!(z.division, 0.0);
    }

    // --- arrest ---

    #[test]
    fn arrest_reduces_division() {
        let mut z = z();
        z.division = 60.0;
        z.arrest(20.0);
        assert!((z.division - 40.0).abs() < 1e-3);
    }

    #[test]
    fn arrest_clamps_at_zero() {
        let mut z = z();
        z.division = 30.0;
        z.arrest(200.0);
        assert_eq!(z.division, 0.0);
    }

    #[test]
    fn arrest_fires_just_arrested_at_zero() {
        let mut z = z();
        z.division = 30.0;
        z.arrest(30.0);
        assert!(z.just_arrested);
    }

    #[test]
    fn arrest_no_op_when_already_arrested() {
        let mut z = z();
        z.arrest(10.0);
        assert!(!z.just_arrested);
    }

    #[test]
    fn arrest_no_op_when_disabled() {
        let mut z = z();
        z.division = 50.0;
        z.enabled = false;
        z.arrest(50.0);
        assert!((z.division - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_proliferates_division() {
        let mut z = z(); // rate=2.5
        z.tick(2.0); // 0 + 2.5*2 = 5
        assert!((z.division - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_proliferating_on_growth_to_max() {
        let mut z = Zeatin::new(100.0, 200.0);
        z.division = 95.0;
        z.tick(1.0);
        assert!(z.just_proliferating);
        assert!(z.is_proliferating());
    }

    #[test]
    fn tick_no_growth_when_already_proliferating() {
        let mut z = z();
        z.division = 100.0;
        z.tick(1.0);
        assert!(!z.just_proliferating);
    }

    #[test]
    fn tick_no_growth_when_rate_zero() {
        let mut z = Zeatin::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.division, 0.0);
    }

    #[test]
    fn tick_no_growth_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.division, 0.0);
    }

    #[test]
    fn tick_clears_just_proliferating() {
        let mut z = Zeatin::new(100.0, 200.0);
        z.division = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_proliferating);
    }

    #[test]
    fn tick_clears_just_arrested() {
        let mut z = z();
        z.division = 10.0;
        z.arrest(10.0);
        z.tick(0.016);
        assert!(!z.just_arrested);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2.5
        z.tick(4.0); // 2.5*4 = 10
        assert!((z.division - 10.0).abs() < 1e-3);
    }

    // --- is_proliferating / is_arrested ---

    #[test]
    fn is_proliferating_false_when_disabled() {
        let mut z = z();
        z.division = 100.0;
        z.enabled = false;
        assert!(!z.is_proliferating());
    }

    #[test]
    fn is_arrested_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_arrested());
    }

    // --- division_fraction / effective_growth ---

    #[test]
    fn division_fraction_zero_when_arrested() {
        assert_eq!(z().division_fraction(), 0.0);
    }

    #[test]
    fn division_fraction_half_at_midpoint() {
        let mut z = z();
        z.division = 50.0;
        assert!((z.division_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_growth_zero_when_arrested() {
        assert_eq!(z().effective_growth(100.0), 0.0);
    }

    #[test]
    fn effective_growth_scales_with_division() {
        let mut z = z();
        z.division = 75.0;
        assert!((z.effective_growth(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_growth_zero_when_disabled() {
        let mut z = z();
        z.division = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_growth(100.0), 0.0);
    }
}
