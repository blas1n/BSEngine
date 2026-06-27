use bevy_ecs::prelude::Component;

/// Mental-stillness tracker. `stillness` builds via `settle(amount)` and
/// deepens passively at `calm_rate` per second in `tick(dt)` or is broken
/// immediately via `stir(amount)`.
///
/// Models meditation-depth bars, concentration meters, tranquillity gauges,
/// NPC calm-state trackers, zen-puzzle completion bars, or any mechanic
/// where an entity gradually quiets its activity and reaches full serenity
/// when undisturbed long enough.
///
/// `settle(amount)` adds stillness; fires `just_still` when first reaching
/// `max_stillness`. No-op when disabled.
///
/// `stir(amount)` reduces stillness immediately; fires `just_agitated` when
/// reaching 0. No-op when disabled or already agitated.
///
/// `tick(dt)` clears both flags, then deepens stillness by `calm_rate * dt`
/// (capped at `max_stillness`). Fires `just_still` when first reaching max.
/// No-op when disabled or rate is 0.
///
/// `is_still()` returns `stillness >= max_stillness && enabled`.
///
/// `is_agitated()` returns `stillness == 0.0` (not gated by `enabled`).
///
/// `stillness_fraction()` returns `(stillness / max_stillness).clamp(0, 1)`.
///
/// `effective_clarity(scale)` returns `scale * stillness_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — calms at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zazen {
    pub stillness: f32,
    pub max_stillness: f32,
    pub calm_rate: f32,
    pub just_still: bool,
    pub just_agitated: bool,
    pub enabled: bool,
}

impl Zazen {
    pub fn new(max_stillness: f32, calm_rate: f32) -> Self {
        Self {
            stillness: 0.0,
            max_stillness: max_stillness.max(0.1),
            calm_rate: calm_rate.max(0.0),
            just_still: false,
            just_agitated: false,
            enabled: true,
        }
    }

    /// Add stillness; fires `just_still` when first reaching max.
    /// No-op when disabled.
    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.stillness < self.max_stillness;
        self.stillness = (self.stillness + amount).min(self.max_stillness);
        if was_below && self.stillness >= self.max_stillness {
            self.just_still = true;
        }
    }

    /// Reduce stillness; fires `just_agitated` when reaching 0.
    /// No-op when disabled or already agitated.
    pub fn stir(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.stillness <= 0.0 {
            return;
        }
        self.stillness = (self.stillness - amount).max(0.0);
        if self.stillness <= 0.0 {
            self.just_agitated = true;
        }
    }

    /// Clear flags, then deepen stillness by `calm_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_still = false;
        self.just_agitated = false;
        if self.enabled && self.calm_rate > 0.0 && self.stillness < self.max_stillness {
            let was_below = self.stillness < self.max_stillness;
            self.stillness = (self.stillness + self.calm_rate * dt).min(self.max_stillness);
            if was_below && self.stillness >= self.max_stillness {
                self.just_still = true;
            }
        }
    }

    /// `true` when stillness is at maximum and component is enabled.
    pub fn is_still(&self) -> bool {
        self.stillness >= self.max_stillness && self.enabled
    }

    /// `true` when stillness is 0 (not gated by `enabled`).
    pub fn is_agitated(&self) -> bool {
        self.stillness == 0.0
    }

    /// Fraction of maximum stillness [0.0, 1.0].
    pub fn stillness_fraction(&self) -> f32 {
        (self.stillness / self.max_stillness).clamp(0.0, 1.0)
    }

    /// Returns `scale * stillness_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_clarity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.stillness_fraction()
    }
}

impl Default for Zazen {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zazen {
        Zazen::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_agitated() {
        let z = z();
        assert_eq!(z.stillness, 0.0);
        assert!(z.is_agitated());
        assert!(!z.is_still());
    }

    #[test]
    fn new_clamps_max_stillness() {
        let z = Zazen::new(-5.0, 4.0);
        assert!((z.max_stillness - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_calm_rate() {
        let z = Zazen::new(100.0, -3.0);
        assert_eq!(z.calm_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zazen::default();
        assert!((z.max_stillness - 100.0).abs() < 1e-5);
        assert!((z.calm_rate - 4.0).abs() < 1e-5);
    }

    // --- settle ---

    #[test]
    fn settle_adds_stillness() {
        let mut z = z();
        z.settle(40.0);
        assert!((z.stillness - 40.0).abs() < 1e-3);
    }

    #[test]
    fn settle_clamps_at_max() {
        let mut z = z();
        z.settle(200.0);
        assert!((z.stillness - 100.0).abs() < 1e-3);
    }

    #[test]
    fn settle_fires_just_still_at_max() {
        let mut z = z();
        z.settle(100.0);
        assert!(z.just_still);
        assert!(z.is_still());
    }

    #[test]
    fn settle_no_just_still_when_already_at_max() {
        let mut z = z();
        z.stillness = 100.0;
        z.settle(10.0);
        assert!(!z.just_still);
    }

    #[test]
    fn settle_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.settle(50.0);
        assert_eq!(z.stillness, 0.0);
    }

    #[test]
    fn settle_no_op_when_amount_zero() {
        let mut z = z();
        z.settle(0.0);
        assert_eq!(z.stillness, 0.0);
    }

    // --- stir ---

    #[test]
    fn stir_reduces_stillness() {
        let mut z = z();
        z.stillness = 60.0;
        z.stir(20.0);
        assert!((z.stillness - 40.0).abs() < 1e-3);
    }

    #[test]
    fn stir_clamps_at_zero() {
        let mut z = z();
        z.stillness = 30.0;
        z.stir(200.0);
        assert_eq!(z.stillness, 0.0);
    }

    #[test]
    fn stir_fires_just_agitated_at_zero() {
        let mut z = z();
        z.stillness = 30.0;
        z.stir(30.0);
        assert!(z.just_agitated);
    }

    #[test]
    fn stir_no_op_when_already_agitated() {
        let mut z = z();
        z.stir(10.0);
        assert!(!z.just_agitated);
    }

    #[test]
    fn stir_no_op_when_disabled() {
        let mut z = z();
        z.stillness = 50.0;
        z.enabled = false;
        z.stir(50.0);
        assert!((z.stillness - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_calms_stillness() {
        let mut z = z(); // calm=4
        z.tick(1.0); // 0 + 4 = 4
        assert!((z.stillness - 4.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_still_on_calm_to_max() {
        let mut z = Zazen::new(100.0, 200.0);
        z.stillness = 95.0;
        z.tick(1.0);
        assert!(z.just_still);
        assert!(z.is_still());
    }

    #[test]
    fn tick_no_calm_when_already_at_max() {
        let mut z = z();
        z.stillness = 100.0;
        z.tick(1.0);
        assert!(!z.just_still);
    }

    #[test]
    fn tick_no_calm_when_rate_zero() {
        let mut z = Zazen::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.stillness, 0.0);
    }

    #[test]
    fn tick_no_calm_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.stillness, 0.0);
    }

    #[test]
    fn tick_clears_just_still() {
        let mut z = Zazen::new(100.0, 200.0);
        z.stillness = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_still);
    }

    #[test]
    fn tick_clears_just_agitated() {
        let mut z = z();
        z.stillness = 10.0;
        z.stir(10.0);
        z.tick(0.016);
        assert!(!z.just_agitated);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // calm=4
        z.tick(5.0); // 4*5 = 20
        assert!((z.stillness - 20.0).abs() < 1e-3);
    }

    // --- is_still / is_agitated ---

    #[test]
    fn is_still_false_when_disabled() {
        let mut z = z();
        z.stillness = 100.0;
        z.enabled = false;
        assert!(!z.is_still());
    }

    #[test]
    fn is_agitated_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_agitated());
    }

    // --- stillness_fraction / effective_clarity ---

    #[test]
    fn stillness_fraction_zero_when_agitated() {
        assert_eq!(z().stillness_fraction(), 0.0);
    }

    #[test]
    fn stillness_fraction_half_at_midpoint() {
        let mut z = z();
        z.stillness = 50.0;
        assert!((z.stillness_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_clarity_zero_when_agitated() {
        assert_eq!(z().effective_clarity(100.0), 0.0);
    }

    #[test]
    fn effective_clarity_scales_with_stillness() {
        let mut z = z();
        z.stillness = 60.0;
        assert!((z.effective_clarity(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_clarity_zero_when_disabled() {
        let mut z = z();
        z.stillness = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_clarity(100.0), 0.0);
    }
}
