use bevy_ecs::prelude::Component;

/// Burden-accumulation tracker. `burden` builds via `load(amount)` and
/// strains passively at `strain_rate` per second in `tick(dt)` or can be
/// shed immediately via `shed(amount)`.
///
/// Models encumbrance meters, carry-weight pressure, fatigue-from-load
/// systems, pack-animal capacity limits, over-stocking penalties, debt
/// accumulators, or any mechanic where a growing burden eventually
/// overwhelms the carrier.
///
/// `load(amount)` adds burden; fires `just_overwhelmed` when first reaching
/// `max_burden`. No-op when disabled.
///
/// `shed(amount)` reduces burden immediately; fires `just_unburdened` when
/// reaching 0. No-op when disabled or already unburdened.
///
/// `tick(dt)` clears both flags, then strains burden by
/// `strain_rate * dt` (capped at `max_burden`). Fires `just_overwhelmed`
/// when first reaching max via strain. No-op when disabled or rate is 0.
///
/// `is_overwhelmed()` returns `burden >= max_burden && enabled`.
///
/// `is_unburdened()` returns `burden == 0.0` (not gated by `enabled`).
///
/// `burden_fraction()` returns `(burden / max_burden).clamp(0, 1)`.
///
/// `effective_weight(scale)` returns `scale * burden_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 3.0)` — strains at 3 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zebu {
    pub burden: f32,
    pub max_burden: f32,
    pub strain_rate: f32,
    pub just_overwhelmed: bool,
    pub just_unburdened: bool,
    pub enabled: bool,
}

impl Zebu {
    pub fn new(max_burden: f32, strain_rate: f32) -> Self {
        Self {
            burden: 0.0,
            max_burden: max_burden.max(0.1),
            strain_rate: strain_rate.max(0.0),
            just_overwhelmed: false,
            just_unburdened: false,
            enabled: true,
        }
    }

    /// Add burden; fires `just_overwhelmed` when first reaching max.
    /// No-op when disabled.
    pub fn load(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.burden < self.max_burden;
        self.burden = (self.burden + amount).min(self.max_burden);
        if was_below && self.burden >= self.max_burden {
            self.just_overwhelmed = true;
        }
    }

    /// Reduce burden; fires `just_unburdened` when reaching 0.
    /// No-op when disabled or already unburdened.
    pub fn shed(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.burden <= 0.0 {
            return;
        }
        self.burden = (self.burden - amount).max(0.0);
        if self.burden <= 0.0 {
            self.just_unburdened = true;
        }
    }

    /// Clear flags, then strain burden by `strain_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_overwhelmed = false;
        self.just_unburdened = false;
        if self.enabled && self.strain_rate > 0.0 && self.burden < self.max_burden {
            let was_below = self.burden < self.max_burden;
            self.burden = (self.burden + self.strain_rate * dt).min(self.max_burden);
            if was_below && self.burden >= self.max_burden {
                self.just_overwhelmed = true;
            }
        }
    }

    /// `true` when burden is at maximum and component is enabled.
    pub fn is_overwhelmed(&self) -> bool {
        self.burden >= self.max_burden && self.enabled
    }

    /// `true` when burden is 0 (not gated by `enabled`).
    pub fn is_unburdened(&self) -> bool {
        self.burden == 0.0
    }

    /// Fraction of maximum burden [0.0, 1.0].
    pub fn burden_fraction(&self) -> f32 {
        (self.burden / self.max_burden).clamp(0.0, 1.0)
    }

    /// Returns `scale * burden_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_weight(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.burden_fraction()
    }
}

impl Default for Zebu {
    fn default() -> Self {
        Self::new(100.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zebu {
        Zebu::new(100.0, 3.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_unburdened() {
        let z = z();
        assert_eq!(z.burden, 0.0);
        assert!(z.is_unburdened());
        assert!(!z.is_overwhelmed());
    }

    #[test]
    fn new_clamps_max_burden() {
        let z = Zebu::new(-5.0, 3.0);
        assert!((z.max_burden - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_strain_rate() {
        let z = Zebu::new(100.0, -3.0);
        assert_eq!(z.strain_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zebu::default();
        assert!((z.max_burden - 100.0).abs() < 1e-5);
        assert!((z.strain_rate - 3.0).abs() < 1e-5);
    }

    // --- load ---

    #[test]
    fn load_adds_burden() {
        let mut z = z();
        z.load(40.0);
        assert!((z.burden - 40.0).abs() < 1e-3);
    }

    #[test]
    fn load_clamps_at_max() {
        let mut z = z();
        z.load(200.0);
        assert!((z.burden - 100.0).abs() < 1e-3);
    }

    #[test]
    fn load_fires_just_overwhelmed_at_max() {
        let mut z = z();
        z.load(100.0);
        assert!(z.just_overwhelmed);
        assert!(z.is_overwhelmed());
    }

    #[test]
    fn load_no_just_overwhelmed_when_already_at_max() {
        let mut z = z();
        z.burden = 100.0;
        z.load(10.0);
        assert!(!z.just_overwhelmed);
    }

    #[test]
    fn load_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.load(50.0);
        assert_eq!(z.burden, 0.0);
    }

    #[test]
    fn load_no_op_when_amount_zero() {
        let mut z = z();
        z.load(0.0);
        assert_eq!(z.burden, 0.0);
    }

    // --- shed ---

    #[test]
    fn shed_reduces_burden() {
        let mut z = z();
        z.burden = 60.0;
        z.shed(20.0);
        assert!((z.burden - 40.0).abs() < 1e-3);
    }

    #[test]
    fn shed_clamps_at_zero() {
        let mut z = z();
        z.burden = 30.0;
        z.shed(200.0);
        assert_eq!(z.burden, 0.0);
    }

    #[test]
    fn shed_fires_just_unburdened_at_zero() {
        let mut z = z();
        z.burden = 30.0;
        z.shed(30.0);
        assert!(z.just_unburdened);
    }

    #[test]
    fn shed_no_op_when_already_unburdened() {
        let mut z = z();
        z.shed(10.0);
        assert!(!z.just_unburdened);
    }

    #[test]
    fn shed_no_op_when_disabled() {
        let mut z = z();
        z.burden = 50.0;
        z.enabled = false;
        z.shed(50.0);
        assert!((z.burden - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_strains_burden() {
        let mut z = z(); // strain=3
        z.burden = 50.0;
        z.tick(1.0); // 50 + 3 = 53
        assert!((z.burden - 53.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_overwhelmed_on_strain_to_max() {
        let mut z = Zebu::new(100.0, 200.0);
        z.burden = 95.0;
        z.tick(1.0);
        assert!(z.just_overwhelmed);
        assert!(z.is_overwhelmed());
    }

    #[test]
    fn tick_no_strain_when_already_overwhelmed() {
        let mut z = z();
        z.burden = 100.0;
        z.tick(1.0);
        assert!(!z.just_overwhelmed);
    }

    #[test]
    fn tick_no_strain_when_rate_zero() {
        let mut z = Zebu::new(100.0, 0.0);
        z.burden = 50.0;
        z.tick(100.0);
        assert!((z.burden - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_strain_when_disabled() {
        let mut z = z();
        z.burden = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.burden - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_overwhelmed() {
        let mut z = Zebu::new(100.0, 200.0);
        z.burden = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_overwhelmed);
    }

    #[test]
    fn tick_clears_just_unburdened() {
        let mut z = z();
        z.burden = 10.0;
        z.shed(10.0);
        z.tick(0.016);
        assert!(!z.just_unburdened);
    }

    #[test]
    fn tick_scales_strain_with_dt() {
        let mut z = z(); // strain=3
        z.tick(5.0); // 0 + 3*5 = 15
        assert!((z.burden - 15.0).abs() < 1e-3);
    }

    // --- is_overwhelmed / is_unburdened ---

    #[test]
    fn is_overwhelmed_false_when_disabled() {
        let mut z = z();
        z.burden = 100.0;
        z.enabled = false;
        assert!(!z.is_overwhelmed());
    }

    #[test]
    fn is_unburdened_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_unburdened());
    }

    // --- burden_fraction / effective_weight ---

    #[test]
    fn burden_fraction_zero_when_unburdened() {
        assert_eq!(z().burden_fraction(), 0.0);
    }

    #[test]
    fn burden_fraction_half_at_midpoint() {
        let mut z = z();
        z.burden = 50.0;
        assert!((z.burden_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_weight_zero_when_unburdened() {
        assert_eq!(z().effective_weight(100.0), 0.0);
    }

    #[test]
    fn effective_weight_scales_with_burden() {
        let mut z = z();
        z.burden = 60.0;
        assert!((z.effective_weight(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_weight_zero_when_disabled() {
        let mut z = z();
        z.burden = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_weight(100.0), 0.0);
    }
}
