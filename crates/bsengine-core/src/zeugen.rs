use bevy_ecs::prelude::Component;

/// Geological-prominence tracker. `prominence` builds via `uplift(amount)`
/// and rises passively at `denude_rate` per second in `tick(dt)` or
/// is worn down immediately via `erode(amount)`.
///
/// Models inselberg-formation progress bars, table-mountain prominence
/// accumulators, differential-weathering intensity gauges, mesa-capping
/// hardness fill levels, tor-formation resistance trackers, butte-survival
/// strength bars, shield-rock durability meters, pediment-erosion stage
/// indicators, residual-hill distinction trackers, or any mechanic where
/// alternating layers of hard and soft rock slowly raise a prominent
/// landform above a stripped plain until a new erosion cycle grinds it
/// flat again.
///
/// `uplift(amount)` adds prominence; fires `just_prominent` when first
/// reaching `max_prominence`. No-op when disabled.
///
/// `erode(amount)` reduces prominence immediately; fires `just_eroded`
/// when reaching 0. No-op when disabled or already eroded.
///
/// `tick(dt)` clears both flags, then increases prominence by
/// `denude_rate * dt` (capped at `max_prominence`). Fires
/// `just_prominent` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_prominent()` returns `prominence >= max_prominence && enabled`.
///
/// `is_eroded()` returns `prominence == 0.0` (not gated by `enabled`).
///
/// `prominence_fraction()` returns `(prominence / max_prominence).clamp(0, 1)`.
///
/// `effective_resistance(scale)` returns `scale * prominence_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 0.5)` — uplifts at 0.5 units/sec (geology is slow).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeugen {
    pub prominence: f32,
    pub max_prominence: f32,
    pub denude_rate: f32,
    pub just_prominent: bool,
    pub just_eroded: bool,
    pub enabled: bool,
}

impl Zeugen {
    pub fn new(max_prominence: f32, denude_rate: f32) -> Self {
        Self {
            prominence: 0.0,
            max_prominence: max_prominence.max(0.1),
            denude_rate: denude_rate.max(0.0),
            just_prominent: false,
            just_eroded: false,
            enabled: true,
        }
    }

    /// Add prominence; fires `just_prominent` when first reaching max.
    /// No-op when disabled.
    pub fn uplift(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.prominence < self.max_prominence;
        self.prominence = (self.prominence + amount).min(self.max_prominence);
        if was_below && self.prominence >= self.max_prominence {
            self.just_prominent = true;
        }
    }

    /// Reduce prominence; fires `just_eroded` when reaching 0.
    /// No-op when disabled or already eroded.
    pub fn erode(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.prominence <= 0.0 {
            return;
        }
        self.prominence = (self.prominence - amount).max(0.0);
        if self.prominence <= 0.0 {
            self.just_eroded = true;
        }
    }

    /// Clear flags, then increase prominence by `denude_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_prominent = false;
        self.just_eroded = false;
        if self.enabled && self.denude_rate > 0.0 && self.prominence < self.max_prominence {
            let was_below = self.prominence < self.max_prominence;
            self.prominence = (self.prominence + self.denude_rate * dt).min(self.max_prominence);
            if was_below && self.prominence >= self.max_prominence {
                self.just_prominent = true;
            }
        }
    }

    /// `true` when prominence is at maximum and component is enabled.
    pub fn is_prominent(&self) -> bool {
        self.prominence >= self.max_prominence && self.enabled
    }

    /// `true` when prominence is 0 (not gated by `enabled`).
    pub fn is_eroded(&self) -> bool {
        self.prominence == 0.0
    }

    /// Fraction of maximum prominence [0.0, 1.0].
    pub fn prominence_fraction(&self) -> f32 {
        (self.prominence / self.max_prominence).clamp(0.0, 1.0)
    }

    /// Returns `scale * prominence_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_resistance(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.prominence_fraction()
    }
}

impl Default for Zeugen {
    fn default() -> Self {
        Self::new(100.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeugen {
        Zeugen::new(100.0, 0.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_eroded() {
        let z = z();
        assert_eq!(z.prominence, 0.0);
        assert!(z.is_eroded());
        assert!(!z.is_prominent());
    }

    #[test]
    fn new_clamps_max_prominence() {
        let z = Zeugen::new(-5.0, 0.5);
        assert!((z.max_prominence - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_denude_rate() {
        let z = Zeugen::new(100.0, -3.0);
        assert_eq!(z.denude_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeugen::default();
        assert!((z.max_prominence - 100.0).abs() < 1e-5);
        assert!((z.denude_rate - 0.5).abs() < 1e-5);
    }

    // --- uplift ---

    #[test]
    fn uplift_adds_prominence() {
        let mut z = z();
        z.uplift(40.0);
        assert!((z.prominence - 40.0).abs() < 1e-3);
    }

    #[test]
    fn uplift_clamps_at_max() {
        let mut z = z();
        z.uplift(200.0);
        assert!((z.prominence - 100.0).abs() < 1e-3);
    }

    #[test]
    fn uplift_fires_just_prominent_at_max() {
        let mut z = z();
        z.uplift(100.0);
        assert!(z.just_prominent);
        assert!(z.is_prominent());
    }

    #[test]
    fn uplift_no_just_prominent_when_already_at_max() {
        let mut z = z();
        z.prominence = 100.0;
        z.uplift(10.0);
        assert!(!z.just_prominent);
    }

    #[test]
    fn uplift_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.uplift(50.0);
        assert_eq!(z.prominence, 0.0);
    }

    #[test]
    fn uplift_no_op_when_amount_zero() {
        let mut z = z();
        z.uplift(0.0);
        assert_eq!(z.prominence, 0.0);
    }

    // --- erode ---

    #[test]
    fn erode_reduces_prominence() {
        let mut z = z();
        z.prominence = 60.0;
        z.erode(20.0);
        assert!((z.prominence - 40.0).abs() < 1e-3);
    }

    #[test]
    fn erode_clamps_at_zero() {
        let mut z = z();
        z.prominence = 30.0;
        z.erode(200.0);
        assert_eq!(z.prominence, 0.0);
    }

    #[test]
    fn erode_fires_just_eroded_at_zero() {
        let mut z = z();
        z.prominence = 30.0;
        z.erode(30.0);
        assert!(z.just_eroded);
    }

    #[test]
    fn erode_no_op_when_already_eroded() {
        let mut z = z();
        z.erode(10.0);
        assert!(!z.just_eroded);
    }

    #[test]
    fn erode_no_op_when_disabled() {
        let mut z = z();
        z.prominence = 50.0;
        z.enabled = false;
        z.erode(50.0);
        assert!((z.prominence - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_uplifts_prominence() {
        let mut z = z(); // rate=0.5
        z.tick(4.0); // 0 + 0.5*4 = 2
        assert!((z.prominence - 2.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_prominent_on_uplift_to_max() {
        let mut z = Zeugen::new(100.0, 200.0);
        z.prominence = 95.0;
        z.tick(1.0);
        assert!(z.just_prominent);
        assert!(z.is_prominent());
    }

    #[test]
    fn tick_no_uplift_when_already_prominent() {
        let mut z = z();
        z.prominence = 100.0;
        z.tick(1.0);
        assert!(!z.just_prominent);
    }

    #[test]
    fn tick_no_uplift_when_rate_zero() {
        let mut z = Zeugen::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.prominence, 0.0);
    }

    #[test]
    fn tick_no_uplift_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.prominence, 0.0);
    }

    #[test]
    fn tick_clears_just_prominent() {
        let mut z = Zeugen::new(100.0, 200.0);
        z.prominence = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_prominent);
    }

    #[test]
    fn tick_clears_just_eroded() {
        let mut z = z();
        z.prominence = 10.0;
        z.erode(10.0);
        z.tick(0.016);
        assert!(!z.just_eroded);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=0.5
        z.tick(10.0); // 0.5*10 = 5
        assert!((z.prominence - 5.0).abs() < 1e-3);
    }

    // --- is_prominent / is_eroded ---

    #[test]
    fn is_prominent_false_when_disabled() {
        let mut z = z();
        z.prominence = 100.0;
        z.enabled = false;
        assert!(!z.is_prominent());
    }

    #[test]
    fn is_eroded_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_eroded());
    }

    // --- prominence_fraction / effective_resistance ---

    #[test]
    fn prominence_fraction_zero_when_eroded() {
        assert_eq!(z().prominence_fraction(), 0.0);
    }

    #[test]
    fn prominence_fraction_half_at_midpoint() {
        let mut z = z();
        z.prominence = 50.0;
        assert!((z.prominence_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_resistance_zero_when_eroded() {
        assert_eq!(z().effective_resistance(100.0), 0.0);
    }

    #[test]
    fn effective_resistance_scales_with_prominence() {
        let mut z = z();
        z.prominence = 75.0;
        assert!((z.effective_resistance(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_resistance_zero_when_disabled() {
        let mut z = z();
        z.prominence = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_resistance(100.0), 0.0);
    }
}
