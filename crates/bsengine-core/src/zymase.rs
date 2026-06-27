use bevy_ecs::prelude::Component;

/// Yeast-enzyme fermentation tracker. `ferment` builds via `inoculate(amount)`
/// and catalyses passively at `catalyze_rate` per second in `tick(dt)` or is
/// denatured immediately via `denature(amount)`.
///
/// Models brewery-fermentation progress bars, sourdough-culture activation
/// fill levels, bioreactor-yield accumulation gauges, kombucha-culture
/// health trackers, wine-yeast saturation indicators, alcohol-conversion
/// efficiency meters, dough-rise intensity bars, ferment-tank capacity
/// trackers, culinary-culture inoculation progress meters, or any mechanic
/// where a microscopic collection of enzymatic proteins slowly converts
/// simple sugars into carbon dioxide and ethanol through chemistry so ancient
/// and unremarkable that it happened accidentally in every abandoned piece
/// of wet grain that was ever left in a jar.
///
/// `inoculate(amount)` adds ferment; fires `just_active` when first
/// reaching `max_ferment`. No-op when disabled.
///
/// `denature(amount)` reduces ferment immediately; fires `just_spent`
/// when reaching 0. No-op when disabled or already spent.
///
/// `tick(dt)` clears both flags, then increases ferment by
/// `catalyze_rate * dt` (capped at `max_ferment`). Fires `just_active`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_active()` returns `ferment >= max_ferment && enabled`.
///
/// `is_spent()` returns `ferment == 0.0` (not gated by `enabled`).
///
/// `ferment_fraction()` returns `(ferment / max_ferment).clamp(0, 1)`.
///
/// `effective_yield(scale)` returns `scale * ferment_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — catalyses at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zymase {
    pub ferment: f32,
    pub max_ferment: f32,
    pub catalyze_rate: f32,
    pub just_active: bool,
    pub just_spent: bool,
    pub enabled: bool,
}

impl Zymase {
    pub fn new(max_ferment: f32, catalyze_rate: f32) -> Self {
        Self {
            ferment: 0.0,
            max_ferment: max_ferment.max(0.1),
            catalyze_rate: catalyze_rate.max(0.0),
            just_active: false,
            just_spent: false,
            enabled: true,
        }
    }

    /// Add ferment; fires `just_active` when first reaching max.
    /// No-op when disabled.
    pub fn inoculate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.ferment < self.max_ferment;
        self.ferment = (self.ferment + amount).min(self.max_ferment);
        if was_below && self.ferment >= self.max_ferment {
            self.just_active = true;
        }
    }

    /// Reduce ferment; fires `just_spent` when reaching 0.
    /// No-op when disabled or already spent.
    pub fn denature(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.ferment <= 0.0 {
            return;
        }
        self.ferment = (self.ferment - amount).max(0.0);
        if self.ferment <= 0.0 {
            self.just_spent = true;
        }
    }

    /// Clear flags, then increase ferment by `catalyze_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_active = false;
        self.just_spent = false;
        if self.enabled && self.catalyze_rate > 0.0 && self.ferment < self.max_ferment {
            let was_below = self.ferment < self.max_ferment;
            self.ferment = (self.ferment + self.catalyze_rate * dt).min(self.max_ferment);
            if was_below && self.ferment >= self.max_ferment {
                self.just_active = true;
            }
        }
    }

    /// `true` when ferment is at maximum and component is enabled.
    pub fn is_active(&self) -> bool {
        self.ferment >= self.max_ferment && self.enabled
    }

    /// `true` when ferment is 0 (not gated by `enabled`).
    pub fn is_spent(&self) -> bool {
        self.ferment == 0.0
    }

    /// Fraction of maximum ferment [0.0, 1.0].
    pub fn ferment_fraction(&self) -> f32 {
        (self.ferment / self.max_ferment).clamp(0.0, 1.0)
    }

    /// Returns `scale * ferment_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_yield(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.ferment_fraction()
    }
}

impl Default for Zymase {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zymase {
        Zymase::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_spent() {
        let z = z();
        assert_eq!(z.ferment, 0.0);
        assert!(z.is_spent());
        assert!(!z.is_active());
    }

    #[test]
    fn new_clamps_max_ferment() {
        let z = Zymase::new(-5.0, 1.5);
        assert!((z.max_ferment - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_catalyze_rate() {
        let z = Zymase::new(100.0, -3.0);
        assert_eq!(z.catalyze_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zymase::default();
        assert!((z.max_ferment - 100.0).abs() < 1e-5);
        assert!((z.catalyze_rate - 1.5).abs() < 1e-5);
    }

    // --- inoculate ---

    #[test]
    fn inoculate_adds_ferment() {
        let mut z = z();
        z.inoculate(40.0);
        assert!((z.ferment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn inoculate_clamps_at_max() {
        let mut z = z();
        z.inoculate(200.0);
        assert!((z.ferment - 100.0).abs() < 1e-3);
    }

    #[test]
    fn inoculate_fires_just_active_at_max() {
        let mut z = z();
        z.inoculate(100.0);
        assert!(z.just_active);
        assert!(z.is_active());
    }

    #[test]
    fn inoculate_no_just_active_when_already_at_max() {
        let mut z = z();
        z.ferment = 100.0;
        z.inoculate(10.0);
        assert!(!z.just_active);
    }

    #[test]
    fn inoculate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.inoculate(50.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn inoculate_no_op_when_amount_zero() {
        let mut z = z();
        z.inoculate(0.0);
        assert_eq!(z.ferment, 0.0);
    }

    // --- denature ---

    #[test]
    fn denature_reduces_ferment() {
        let mut z = z();
        z.ferment = 60.0;
        z.denature(20.0);
        assert!((z.ferment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn denature_clamps_at_zero() {
        let mut z = z();
        z.ferment = 30.0;
        z.denature(200.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn denature_fires_just_spent_at_zero() {
        let mut z = z();
        z.ferment = 30.0;
        z.denature(30.0);
        assert!(z.just_spent);
    }

    #[test]
    fn denature_no_op_when_already_spent() {
        let mut z = z();
        z.denature(10.0);
        assert!(!z.just_spent);
    }

    #[test]
    fn denature_no_op_when_disabled() {
        let mut z = z();
        z.ferment = 50.0;
        z.enabled = false;
        z.denature(50.0);
        assert!((z.ferment - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_catalyses_ferment() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.ferment - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_active_on_catalyze_to_max() {
        let mut z = Zymase::new(100.0, 200.0);
        z.ferment = 95.0;
        z.tick(1.0);
        assert!(z.just_active);
        assert!(z.is_active());
    }

    #[test]
    fn tick_no_catalyze_when_already_active() {
        let mut z = z();
        z.ferment = 100.0;
        z.tick(1.0);
        assert!(!z.just_active);
    }

    #[test]
    fn tick_no_catalyze_when_rate_zero() {
        let mut z = Zymase::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn tick_no_catalyze_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn tick_clears_just_active() {
        let mut z = Zymase::new(100.0, 200.0);
        z.ferment = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_active);
    }

    #[test]
    fn tick_clears_just_spent() {
        let mut z = z();
        z.ferment = 10.0;
        z.denature(10.0);
        z.tick(0.016);
        assert!(!z.just_spent);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(2.0); // 1.5*2 = 3
        assert!((z.ferment - 3.0).abs() < 1e-3);
    }

    // --- is_active / is_spent ---

    #[test]
    fn is_active_false_when_disabled() {
        let mut z = z();
        z.ferment = 100.0;
        z.enabled = false;
        assert!(!z.is_active());
    }

    #[test]
    fn is_spent_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_spent());
    }

    // --- ferment_fraction / effective_yield ---

    #[test]
    fn ferment_fraction_zero_when_spent() {
        assert_eq!(z().ferment_fraction(), 0.0);
    }

    #[test]
    fn ferment_fraction_half_at_midpoint() {
        let mut z = z();
        z.ferment = 50.0;
        assert!((z.ferment_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_yield_zero_when_spent() {
        assert_eq!(z().effective_yield(100.0), 0.0);
    }

    #[test]
    fn effective_yield_scales_with_ferment() {
        let mut z = z();
        z.ferment = 75.0;
        assert!((z.effective_yield(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_yield_zero_when_disabled() {
        let mut z = z();
        z.ferment = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_yield(100.0), 0.0);
    }
}
