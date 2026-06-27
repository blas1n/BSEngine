use bevy_ecs::prelude::Component;

/// Heat-saturation tracker. `heat_level` builds via `warm(amount)` and
/// rises passively at `simmer_rate` per second in `tick(dt)` or drops
/// immediately via `cool(amount)`.
///
/// Models cooking-progress bars, forge-temperature gauges, lava-exposure
/// meters, heat-buildup in machinery, magma-proximity dangers, or any
/// mechanic where something heats up over time and reaches a critical
/// scorching point.
///
/// `warm(amount)` adds heat; fires `just_scorching` when first reaching
/// `max_heat`. No-op when disabled.
///
/// `cool(amount)` reduces heat immediately; fires `just_raw` when reaching
/// 0. No-op when disabled or already raw.
///
/// `tick(dt)` clears both flags, then simmers heat by `simmer_rate * dt`
/// (capped at `max_heat`). Fires `just_scorching` when first reaching max.
/// No-op when disabled or rate is 0.
///
/// `is_scorching()` returns `heat_level >= max_heat && enabled`.
///
/// `is_raw()` returns `heat_level == 0.0` (not gated by `enabled`).
///
/// `heat_fraction()` returns `(heat_level / max_heat).clamp(0, 1)`.
///
/// `effective_temperature(scale)` returns `scale * heat_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — simmers at 5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ziti {
    pub heat_level: f32,
    pub max_heat: f32,
    pub simmer_rate: f32,
    pub just_scorching: bool,
    pub just_raw: bool,
    pub enabled: bool,
}

impl Ziti {
    pub fn new(max_heat: f32, simmer_rate: f32) -> Self {
        Self {
            heat_level: 0.0,
            max_heat: max_heat.max(0.1),
            simmer_rate: simmer_rate.max(0.0),
            just_scorching: false,
            just_raw: false,
            enabled: true,
        }
    }

    /// Add heat; fires `just_scorching` when first reaching max.
    /// No-op when disabled.
    pub fn warm(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.heat_level < self.max_heat;
        self.heat_level = (self.heat_level + amount).min(self.max_heat);
        if was_below && self.heat_level >= self.max_heat {
            self.just_scorching = true;
        }
    }

    /// Reduce heat; fires `just_raw` when reaching 0.
    /// No-op when disabled or already raw.
    pub fn cool(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.heat_level <= 0.0 {
            return;
        }
        self.heat_level = (self.heat_level - amount).max(0.0);
        if self.heat_level <= 0.0 {
            self.just_raw = true;
        }
    }

    /// Clear flags, then simmer heat by `simmer_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_scorching = false;
        self.just_raw = false;
        if self.enabled && self.simmer_rate > 0.0 && self.heat_level < self.max_heat {
            let was_below = self.heat_level < self.max_heat;
            self.heat_level = (self.heat_level + self.simmer_rate * dt).min(self.max_heat);
            if was_below && self.heat_level >= self.max_heat {
                self.just_scorching = true;
            }
        }
    }

    /// `true` when heat is at maximum and component is enabled.
    pub fn is_scorching(&self) -> bool {
        self.heat_level >= self.max_heat && self.enabled
    }

    /// `true` when heat is 0 (not gated by `enabled`).
    pub fn is_raw(&self) -> bool {
        self.heat_level == 0.0
    }

    /// Fraction of maximum heat [0.0, 1.0].
    pub fn heat_fraction(&self) -> f32 {
        (self.heat_level / self.max_heat).clamp(0.0, 1.0)
    }

    /// Returns `scale * heat_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_temperature(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.heat_fraction()
    }
}

impl Default for Ziti {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Ziti {
        Ziti::new(100.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_raw() {
        let z = z();
        assert_eq!(z.heat_level, 0.0);
        assert!(z.is_raw());
        assert!(!z.is_scorching());
    }

    #[test]
    fn new_clamps_max_heat() {
        let z = Ziti::new(-5.0, 5.0);
        assert!((z.max_heat - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_simmer_rate() {
        let z = Ziti::new(100.0, -3.0);
        assert_eq!(z.simmer_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Ziti::default();
        assert!((z.max_heat - 100.0).abs() < 1e-5);
        assert!((z.simmer_rate - 5.0).abs() < 1e-5);
    }

    // --- warm ---

    #[test]
    fn warm_adds_heat() {
        let mut z = z();
        z.warm(40.0);
        assert!((z.heat_level - 40.0).abs() < 1e-3);
    }

    #[test]
    fn warm_clamps_at_max() {
        let mut z = z();
        z.warm(200.0);
        assert!((z.heat_level - 100.0).abs() < 1e-3);
    }

    #[test]
    fn warm_fires_just_scorching_at_max() {
        let mut z = z();
        z.warm(100.0);
        assert!(z.just_scorching);
        assert!(z.is_scorching());
    }

    #[test]
    fn warm_no_just_scorching_when_already_at_max() {
        let mut z = z();
        z.heat_level = 100.0;
        z.warm(10.0);
        assert!(!z.just_scorching);
    }

    #[test]
    fn warm_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.warm(50.0);
        assert_eq!(z.heat_level, 0.0);
    }

    #[test]
    fn warm_no_op_when_amount_zero() {
        let mut z = z();
        z.warm(0.0);
        assert_eq!(z.heat_level, 0.0);
    }

    // --- cool ---

    #[test]
    fn cool_reduces_heat() {
        let mut z = z();
        z.heat_level = 60.0;
        z.cool(20.0);
        assert!((z.heat_level - 40.0).abs() < 1e-3);
    }

    #[test]
    fn cool_clamps_at_zero() {
        let mut z = z();
        z.heat_level = 30.0;
        z.cool(200.0);
        assert_eq!(z.heat_level, 0.0);
    }

    #[test]
    fn cool_fires_just_raw_at_zero() {
        let mut z = z();
        z.heat_level = 30.0;
        z.cool(30.0);
        assert!(z.just_raw);
    }

    #[test]
    fn cool_no_op_when_already_raw() {
        let mut z = z();
        z.cool(10.0);
        assert!(!z.just_raw);
    }

    #[test]
    fn cool_no_op_when_disabled() {
        let mut z = z();
        z.heat_level = 50.0;
        z.enabled = false;
        z.cool(50.0);
        assert!((z.heat_level - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_simmers_heat() {
        let mut z = z(); // simmer=5
        z.tick(1.0); // 0 + 5 = 5
        assert!((z.heat_level - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_scorching_on_simmer_to_max() {
        let mut z = Ziti::new(100.0, 200.0);
        z.heat_level = 95.0;
        z.tick(1.0);
        assert!(z.just_scorching);
        assert!(z.is_scorching());
    }

    #[test]
    fn tick_no_simmer_when_already_at_max() {
        let mut z = z();
        z.heat_level = 100.0;
        z.tick(1.0);
        assert!(!z.just_scorching);
    }

    #[test]
    fn tick_no_simmer_when_rate_zero() {
        let mut z = Ziti::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.heat_level, 0.0);
    }

    #[test]
    fn tick_no_simmer_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.heat_level, 0.0);
    }

    #[test]
    fn tick_clears_just_scorching() {
        let mut z = Ziti::new(100.0, 200.0);
        z.heat_level = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_scorching);
    }

    #[test]
    fn tick_clears_just_raw() {
        let mut z = z();
        z.heat_level = 10.0;
        z.cool(10.0);
        z.tick(0.016);
        assert!(!z.just_raw);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // simmer=5
        z.tick(6.0); // 5*6 = 30
        assert!((z.heat_level - 30.0).abs() < 1e-3);
    }

    // --- is_scorching / is_raw ---

    #[test]
    fn is_scorching_false_when_disabled() {
        let mut z = z();
        z.heat_level = 100.0;
        z.enabled = false;
        assert!(!z.is_scorching());
    }

    #[test]
    fn is_raw_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_raw());
    }

    // --- heat_fraction / effective_temperature ---

    #[test]
    fn heat_fraction_zero_when_raw() {
        assert_eq!(z().heat_fraction(), 0.0);
    }

    #[test]
    fn heat_fraction_half_at_midpoint() {
        let mut z = z();
        z.heat_level = 50.0;
        assert!((z.heat_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_temperature_zero_when_raw() {
        assert_eq!(z().effective_temperature(100.0), 0.0);
    }

    #[test]
    fn effective_temperature_scales_with_heat() {
        let mut z = z();
        z.heat_level = 70.0;
        assert!((z.effective_temperature(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_temperature_zero_when_disabled() {
        let mut z = z();
        z.heat_level = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_temperature(100.0), 0.0);
    }
}
