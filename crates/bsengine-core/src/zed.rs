use bevy_ecs::prelude::Component;

/// Wind-down / shutdown-phase tracker. `fatigue` builds via `weary(amount)`
/// and creeps up passively at `wind_down_rate` per second in `tick(dt)` or
/// is immediately reversed via `rally(amount)`.
///
/// Models a final-phase meter, shutdown countdown, engine-wind-down
/// tracker, elder-character exhaustion bar, end-of-level decay mechanic,
/// power-cell-depletion indicator, or any system where an entity
/// inevitably grinds to a halt unless actively counter-acted.
///
/// `weary(amount)` adds fatigue; fires `just_spent` when first reaching
/// `max_fatigue`. No-op when disabled.
///
/// `rally(amount)` reduces fatigue immediately; fires `just_rallied` when
/// reaching 0. No-op when disabled or already rallied.
///
/// `tick(dt)` clears both flags, then increases fatigue by
/// `wind_down_rate * dt` (capped at `max_fatigue`). Fires `just_spent`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_spent()` returns `fatigue >= max_fatigue && enabled`.
///
/// `is_rallied()` returns `fatigue == 0.0` (not gated by `enabled`).
///
/// `fatigue_fraction()` returns `(fatigue / max_fatigue).clamp(0, 1)`.
///
/// `effective_drag(scale)` returns `scale * fatigue_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — winds down at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zed {
    pub fatigue: f32,
    pub max_fatigue: f32,
    pub wind_down_rate: f32,
    pub just_spent: bool,
    pub just_rallied: bool,
    pub enabled: bool,
}

impl Zed {
    pub fn new(max_fatigue: f32, wind_down_rate: f32) -> Self {
        Self {
            fatigue: 0.0,
            max_fatigue: max_fatigue.max(0.1),
            wind_down_rate: wind_down_rate.max(0.0),
            just_spent: false,
            just_rallied: false,
            enabled: true,
        }
    }

    /// Add fatigue; fires `just_spent` when first reaching max.
    /// No-op when disabled.
    pub fn weary(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.fatigue < self.max_fatigue;
        self.fatigue = (self.fatigue + amount).min(self.max_fatigue);
        if was_below && self.fatigue >= self.max_fatigue {
            self.just_spent = true;
        }
    }

    /// Reduce fatigue; fires `just_rallied` when reaching 0.
    /// No-op when disabled or already rallied.
    pub fn rally(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fatigue <= 0.0 {
            return;
        }
        self.fatigue = (self.fatigue - amount).max(0.0);
        if self.fatigue <= 0.0 {
            self.just_rallied = true;
        }
    }

    /// Clear flags, then increase fatigue by `wind_down_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_spent = false;
        self.just_rallied = false;
        if self.enabled && self.wind_down_rate > 0.0 && self.fatigue < self.max_fatigue {
            let was_below = self.fatigue < self.max_fatigue;
            self.fatigue = (self.fatigue + self.wind_down_rate * dt).min(self.max_fatigue);
            if was_below && self.fatigue >= self.max_fatigue {
                self.just_spent = true;
            }
        }
    }

    /// `true` when fatigue is at maximum and component is enabled.
    pub fn is_spent(&self) -> bool {
        self.fatigue >= self.max_fatigue && self.enabled
    }

    /// `true` when fatigue is 0 (not gated by `enabled`).
    pub fn is_rallied(&self) -> bool {
        self.fatigue == 0.0
    }

    /// Fraction of maximum fatigue [0.0, 1.0].
    pub fn fatigue_fraction(&self) -> f32 {
        (self.fatigue / self.max_fatigue).clamp(0.0, 1.0)
    }

    /// Returns `scale * fatigue_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_drag(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.fatigue_fraction()
    }
}

impl Default for Zed {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zed {
        Zed::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_rallied() {
        let z = z();
        assert_eq!(z.fatigue, 0.0);
        assert!(z.is_rallied());
        assert!(!z.is_spent());
    }

    #[test]
    fn new_clamps_max_fatigue() {
        let z = Zed::new(-5.0, 2.0);
        assert!((z.max_fatigue - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_wind_down_rate() {
        let z = Zed::new(100.0, -3.0);
        assert_eq!(z.wind_down_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zed::default();
        assert!((z.max_fatigue - 100.0).abs() < 1e-5);
        assert!((z.wind_down_rate - 2.0).abs() < 1e-5);
    }

    // --- weary ---

    #[test]
    fn weary_adds_fatigue() {
        let mut z = z();
        z.weary(40.0);
        assert!((z.fatigue - 40.0).abs() < 1e-3);
    }

    #[test]
    fn weary_clamps_at_max() {
        let mut z = z();
        z.weary(200.0);
        assert!((z.fatigue - 100.0).abs() < 1e-3);
    }

    #[test]
    fn weary_fires_just_spent_at_max() {
        let mut z = z();
        z.weary(100.0);
        assert!(z.just_spent);
        assert!(z.is_spent());
    }

    #[test]
    fn weary_no_just_spent_when_already_at_max() {
        let mut z = z();
        z.fatigue = 100.0;
        z.weary(10.0);
        assert!(!z.just_spent);
    }

    #[test]
    fn weary_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.weary(50.0);
        assert_eq!(z.fatigue, 0.0);
    }

    #[test]
    fn weary_no_op_when_amount_zero() {
        let mut z = z();
        z.weary(0.0);
        assert_eq!(z.fatigue, 0.0);
    }

    // --- rally ---

    #[test]
    fn rally_reduces_fatigue() {
        let mut z = z();
        z.fatigue = 60.0;
        z.rally(20.0);
        assert!((z.fatigue - 40.0).abs() < 1e-3);
    }

    #[test]
    fn rally_clamps_at_zero() {
        let mut z = z();
        z.fatigue = 30.0;
        z.rally(200.0);
        assert_eq!(z.fatigue, 0.0);
    }

    #[test]
    fn rally_fires_just_rallied_at_zero() {
        let mut z = z();
        z.fatigue = 30.0;
        z.rally(30.0);
        assert!(z.just_rallied);
    }

    #[test]
    fn rally_no_op_when_already_rallied() {
        let mut z = z();
        z.rally(10.0);
        assert!(!z.just_rallied);
    }

    #[test]
    fn rally_no_op_when_disabled() {
        let mut z = z();
        z.fatigue = 50.0;
        z.enabled = false;
        z.rally(50.0);
        assert!((z.fatigue - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_winds_down_fatigue() {
        let mut z = z(); // rate=2
        z.tick(1.0); // 0 + 2 = 2
        assert!((z.fatigue - 2.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_spent_on_wind_down_to_max() {
        let mut z = Zed::new(100.0, 200.0);
        z.fatigue = 95.0;
        z.tick(1.0);
        assert!(z.just_spent);
        assert!(z.is_spent());
    }

    #[test]
    fn tick_no_wind_down_when_already_spent() {
        let mut z = z();
        z.fatigue = 100.0;
        z.tick(1.0);
        assert!(!z.just_spent);
    }

    #[test]
    fn tick_no_wind_down_when_rate_zero() {
        let mut z = Zed::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.fatigue, 0.0);
    }

    #[test]
    fn tick_no_wind_down_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.fatigue, 0.0);
    }

    #[test]
    fn tick_clears_just_spent() {
        let mut z = Zed::new(100.0, 200.0);
        z.fatigue = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_spent);
    }

    #[test]
    fn tick_clears_just_rallied() {
        let mut z = z();
        z.fatigue = 10.0;
        z.rally(10.0);
        z.tick(0.016);
        assert!(!z.just_rallied);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.fatigue - 10.0).abs() < 1e-3);
    }

    // --- is_spent / is_rallied ---

    #[test]
    fn is_spent_false_when_disabled() {
        let mut z = z();
        z.fatigue = 100.0;
        z.enabled = false;
        assert!(!z.is_spent());
    }

    #[test]
    fn is_rallied_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_rallied());
    }

    // --- fatigue_fraction / effective_drag ---

    #[test]
    fn fatigue_fraction_zero_when_rallied() {
        assert_eq!(z().fatigue_fraction(), 0.0);
    }

    #[test]
    fn fatigue_fraction_half_at_midpoint() {
        let mut z = z();
        z.fatigue = 50.0;
        assert!((z.fatigue_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_drag_zero_when_rallied() {
        assert_eq!(z().effective_drag(100.0), 0.0);
    }

    #[test]
    fn effective_drag_scales_with_fatigue() {
        let mut z = z();
        z.fatigue = 70.0;
        assert!((z.effective_drag(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_drag_zero_when_disabled() {
        let mut z = z();
        z.fatigue = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_drag(100.0), 0.0);
    }
}
