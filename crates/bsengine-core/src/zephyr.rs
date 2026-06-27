use bevy_ecs::prelude::Component;

/// Wind-gust tracker. `gust` builds via `breathe(amount)` and calms
/// passively at `calm_rate` per second in `tick(dt)` or immediately via
/// `lull(amount)`.
///
/// Models wind gusts, breeze intensity, air-current meters, aerodynamic
/// forces, or any mechanic where an active surge dissipates over time
/// unless re-energised.
///
/// `breathe(amount)` adds to `gust`; fires `just_surged` when first
/// reaching `max_gust`. No-op when disabled.
///
/// `lull(amount)` reduces `gust` immediately; fires `just_stilled` when
/// reaching 0. No-op when disabled or already still.
///
/// `tick(dt)` clears both flags, then calms `gust` by `calm_rate * dt`
/// (floored at 0). Fires `just_stilled` when reaching 0 via calm. No-op
/// when disabled or rate is 0.
///
/// `is_surging()` returns `gust >= max_gust && enabled`.
///
/// `is_still()` returns `gust == 0.0` (not gated by `enabled`).
///
/// `gust_fraction()` returns `(gust / max_gust).clamp(0, 1)`.
///
/// `effective_gale(scale)` returns `scale * gust_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(100.0, 8.0)` — calms at 8 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zephyr {
    pub gust: f32,
    pub max_gust: f32,
    pub calm_rate: f32,
    pub just_surged: bool,
    pub just_stilled: bool,
    pub enabled: bool,
}

impl Zephyr {
    pub fn new(max_gust: f32, calm_rate: f32) -> Self {
        Self {
            gust: 0.0,
            max_gust: max_gust.max(0.1),
            calm_rate: calm_rate.max(0.0),
            just_surged: false,
            just_stilled: false,
            enabled: true,
        }
    }

    /// Add gust; fires `just_surged` when first reaching max.
    /// No-op when disabled.
    pub fn breathe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.gust < self.max_gust;
        self.gust = (self.gust + amount).min(self.max_gust);
        if was_below && self.gust >= self.max_gust {
            self.just_surged = true;
        }
    }

    /// Reduce gust; fires `just_stilled` when reaching 0.
    /// No-op when disabled or already still.
    pub fn lull(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.gust <= 0.0 {
            return;
        }
        self.gust = (self.gust - amount).max(0.0);
        if self.gust <= 0.0 {
            self.just_stilled = true;
        }
    }

    /// Clear flags, then calm gust by `calm_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_surged = false;
        self.just_stilled = false;
        if self.enabled && self.calm_rate > 0.0 && self.gust > 0.0 {
            self.gust = (self.gust - self.calm_rate * dt).max(0.0);
            if self.gust <= 0.0 {
                self.just_stilled = true;
            }
        }
    }

    /// `true` when gust is at maximum and component is enabled.
    pub fn is_surging(&self) -> bool {
        self.gust >= self.max_gust && self.enabled
    }

    /// `true` when gust is 0 (not gated by `enabled`).
    pub fn is_still(&self) -> bool {
        self.gust == 0.0
    }

    /// Fraction of maximum gust [0.0, 1.0].
    pub fn gust_fraction(&self) -> f32 {
        (self.gust / self.max_gust).clamp(0.0, 1.0)
    }

    /// Returns `scale * gust_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_gale(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.gust_fraction()
    }
}

impl Default for Zephyr {
    fn default() -> Self {
        Self::new(100.0, 8.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zephyr {
        Zephyr::new(100.0, 8.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_still() {
        let z = z();
        assert_eq!(z.gust, 0.0);
        assert!(z.is_still());
        assert!(!z.is_surging());
    }

    #[test]
    fn new_clamps_max_gust() {
        let z = Zephyr::new(-5.0, 8.0);
        assert!((z.max_gust - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_calm_rate() {
        let z = Zephyr::new(100.0, -3.0);
        assert_eq!(z.calm_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zephyr::default();
        assert!((z.max_gust - 100.0).abs() < 1e-5);
        assert!((z.calm_rate - 8.0).abs() < 1e-5);
    }

    // --- breathe ---

    #[test]
    fn breathe_adds_gust() {
        let mut z = z();
        z.breathe(40.0);
        assert!((z.gust - 40.0).abs() < 1e-3);
    }

    #[test]
    fn breathe_clamps_at_max() {
        let mut z = z();
        z.breathe(200.0);
        assert!((z.gust - 100.0).abs() < 1e-3);
    }

    #[test]
    fn breathe_fires_just_surged_at_max() {
        let mut z = z();
        z.breathe(100.0);
        assert!(z.just_surged);
        assert!(z.is_surging());
    }

    #[test]
    fn breathe_no_just_surged_when_already_at_max() {
        let mut z = z();
        z.gust = 100.0;
        z.breathe(10.0);
        assert!(!z.just_surged);
    }

    #[test]
    fn breathe_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.breathe(50.0);
        assert_eq!(z.gust, 0.0);
    }

    #[test]
    fn breathe_no_op_when_amount_zero() {
        let mut z = z();
        z.breathe(0.0);
        assert_eq!(z.gust, 0.0);
    }

    // --- lull ---

    #[test]
    fn lull_reduces_gust() {
        let mut z = z();
        z.gust = 60.0;
        z.lull(20.0);
        assert!((z.gust - 40.0).abs() < 1e-3);
    }

    #[test]
    fn lull_clamps_at_zero() {
        let mut z = z();
        z.gust = 30.0;
        z.lull(200.0);
        assert_eq!(z.gust, 0.0);
    }

    #[test]
    fn lull_fires_just_stilled_at_zero() {
        let mut z = z();
        z.gust = 30.0;
        z.lull(30.0);
        assert!(z.just_stilled);
    }

    #[test]
    fn lull_no_op_when_already_still() {
        let mut z = z();
        z.lull(10.0);
        assert!(!z.just_stilled);
    }

    #[test]
    fn lull_no_op_when_disabled() {
        let mut z = z();
        z.gust = 50.0;
        z.enabled = false;
        z.lull(50.0);
        assert!((z.gust - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_calms_gust() {
        let mut z = z(); // calm=8
        z.gust = 60.0;
        z.tick(1.0); // 60 - 8 = 52
        assert!((z.gust - 52.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_stilled_on_calm_to_zero() {
        let mut z = Zephyr::new(100.0, 200.0);
        z.gust = 5.0;
        z.tick(1.0);
        assert!(z.just_stilled);
        assert!(z.is_still());
    }

    #[test]
    fn tick_no_calm_when_already_still() {
        let mut z = z();
        z.tick(10.0);
        assert!(!z.just_stilled);
    }

    #[test]
    fn tick_no_calm_when_rate_zero() {
        let mut z = Zephyr::new(100.0, 0.0);
        z.gust = 50.0;
        z.tick(100.0);
        assert!((z.gust - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_calm_when_disabled() {
        let mut z = z();
        z.gust = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.gust - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_surged() {
        let mut z = z();
        z.breathe(100.0);
        z.tick(0.016);
        assert!(!z.just_surged);
    }

    #[test]
    fn tick_clears_just_stilled() {
        let mut z = Zephyr::new(100.0, 200.0);
        z.gust = 5.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_stilled);
    }

    #[test]
    fn tick_scales_calm_with_dt() {
        let mut z = z(); // calm=8
        z.gust = 100.0;
        z.tick(2.0); // 100 - 8*2 = 84
        assert!((z.gust - 84.0).abs() < 1e-3);
    }

    // --- is_surging / is_still ---

    #[test]
    fn is_surging_false_when_disabled() {
        let mut z = z();
        z.gust = 100.0;
        z.enabled = false;
        assert!(!z.is_surging());
    }

    #[test]
    fn is_still_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_still());
    }

    // --- gust_fraction / effective_gale ---

    #[test]
    fn gust_fraction_zero_when_still() {
        assert_eq!(z().gust_fraction(), 0.0);
    }

    #[test]
    fn gust_fraction_half_at_midpoint() {
        let mut z = z();
        z.gust = 50.0;
        assert!((z.gust_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_gale_zero_when_still() {
        assert_eq!(z().effective_gale(100.0), 0.0);
    }

    #[test]
    fn effective_gale_scales_with_gust() {
        let mut z = z();
        z.gust = 75.0;
        assert!((z.effective_gale(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_gale_zero_when_disabled() {
        let mut z = z();
        z.gust = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_gale(100.0), 0.0);
    }
}
