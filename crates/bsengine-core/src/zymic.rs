use bevy_ecs::prelude::Component;

/// Fermentation-progress tracker. `ferment` builds via
/// `activate(amount)` and intensifies passively at `enzyme_rate`
/// per second in `tick(dt)` or is exhausted immediately via
/// `exhaust(amount)`.
///
/// Models brew-batch fermentation progress bars, enzyme-activity
/// saturation trackers, yeast-colony build-up meters, wort-sugar
/// conversion fill levels, zymatic-reaction progress indicators,
/// dough-leavening accumulation gauges, kombucha-culture activity
/// bars, kimchi-fermentation intensity trackers, sourdough-starter
/// vigor meters, or any mechanic where invisible microbial labor
/// quietly transforms raw substrate into something complex and alive
/// over hours or days — right until the culture spends its last
/// available nutrient and the bubbling foam goes still.
///
/// `activate(amount)` adds ferment; fires `just_brewed` when first
/// reaching `max_ferment`. No-op when disabled.
///
/// `exhaust(amount)` reduces ferment immediately; fires `just_spent`
/// when reaching 0. No-op when disabled or already spent.
///
/// `tick(dt)` clears both flags, then increases ferment by
/// `enzyme_rate * dt` (capped at `max_ferment`). Fires `just_brewed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_brewed()` returns `ferment >= max_ferment && enabled`.
///
/// `is_spent()` returns `ferment == 0.0` (not gated by `enabled`).
///
/// `ferment_fraction()` returns `(ferment / max_ferment).clamp(0, 1)`.
///
/// `effective_yield(scale)` returns `scale * ferment_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — enzymes work at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zymic {
    pub ferment: f32,
    pub max_ferment: f32,
    pub enzyme_rate: f32,
    pub just_brewed: bool,
    pub just_spent: bool,
    pub enabled: bool,
}

impl Zymic {
    pub fn new(max_ferment: f32, enzyme_rate: f32) -> Self {
        Self {
            ferment: 0.0,
            max_ferment: max_ferment.max(0.1),
            enzyme_rate: enzyme_rate.max(0.0),
            just_brewed: false,
            just_spent: false,
            enabled: true,
        }
    }

    /// Add ferment; fires `just_brewed` when first reaching max.
    /// No-op when disabled.
    pub fn activate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.ferment < self.max_ferment;
        self.ferment = (self.ferment + amount).min(self.max_ferment);
        if was_below && self.ferment >= self.max_ferment {
            self.just_brewed = true;
        }
    }

    /// Reduce ferment; fires `just_spent` when reaching 0.
    /// No-op when disabled or already spent.
    pub fn exhaust(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.ferment <= 0.0 {
            return;
        }
        self.ferment = (self.ferment - amount).max(0.0);
        if self.ferment <= 0.0 {
            self.just_spent = true;
        }
    }

    /// Clear flags, then increase ferment by `enzyme_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_brewed = false;
        self.just_spent = false;
        if self.enabled && self.enzyme_rate > 0.0 && self.ferment < self.max_ferment {
            let was_below = self.ferment < self.max_ferment;
            self.ferment = (self.ferment + self.enzyme_rate * dt).min(self.max_ferment);
            if was_below && self.ferment >= self.max_ferment {
                self.just_brewed = true;
            }
        }
    }

    /// `true` when ferment is at maximum and component is enabled.
    pub fn is_brewed(&self) -> bool {
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

impl Default for Zymic {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zymic {
        Zymic::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_spent() {
        let z = z();
        assert_eq!(z.ferment, 0.0);
        assert!(z.is_spent());
        assert!(!z.is_brewed());
    }

    #[test]
    fn new_clamps_max_ferment() {
        let z = Zymic::new(-5.0, 1.5);
        assert!((z.max_ferment - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_enzyme_rate() {
        let z = Zymic::new(100.0, -1.5);
        assert_eq!(z.enzyme_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zymic::default();
        assert!((z.max_ferment - 100.0).abs() < 1e-5);
        assert!((z.enzyme_rate - 1.5).abs() < 1e-5);
    }

    // --- activate ---

    #[test]
    fn activate_adds_ferment() {
        let mut z = z();
        z.activate(40.0);
        assert!((z.ferment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn activate_clamps_at_max() {
        let mut z = z();
        z.activate(200.0);
        assert!((z.ferment - 100.0).abs() < 1e-3);
    }

    #[test]
    fn activate_fires_just_brewed_at_max() {
        let mut z = z();
        z.activate(100.0);
        assert!(z.just_brewed);
        assert!(z.is_brewed());
    }

    #[test]
    fn activate_no_just_brewed_when_already_at_max() {
        let mut z = z();
        z.ferment = 100.0;
        z.activate(10.0);
        assert!(!z.just_brewed);
    }

    #[test]
    fn activate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.activate(50.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn activate_no_op_when_amount_zero() {
        let mut z = z();
        z.activate(0.0);
        assert_eq!(z.ferment, 0.0);
    }

    // --- exhaust ---

    #[test]
    fn exhaust_reduces_ferment() {
        let mut z = z();
        z.ferment = 60.0;
        z.exhaust(20.0);
        assert!((z.ferment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn exhaust_clamps_at_zero() {
        let mut z = z();
        z.ferment = 30.0;
        z.exhaust(200.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn exhaust_fires_just_spent_at_zero() {
        let mut z = z();
        z.ferment = 30.0;
        z.exhaust(30.0);
        assert!(z.just_spent);
    }

    #[test]
    fn exhaust_no_op_when_already_spent() {
        let mut z = z();
        z.exhaust(10.0);
        assert!(!z.just_spent);
    }

    #[test]
    fn exhaust_no_op_when_disabled() {
        let mut z = z();
        z.ferment = 50.0;
        z.enabled = false;
        z.exhaust(50.0);
        assert!((z.ferment - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_ferment() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.ferment - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_brewed_on_build_to_max() {
        let mut z = Zymic::new(100.0, 200.0);
        z.ferment = 95.0;
        z.tick(1.0);
        assert!(z.just_brewed);
        assert!(z.is_brewed());
    }

    #[test]
    fn tick_no_build_when_already_brewed() {
        let mut z = z();
        z.ferment = 100.0;
        z.tick(1.0);
        assert!(!z.just_brewed);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut z = Zymic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.ferment, 0.0);
    }

    #[test]
    fn tick_clears_just_brewed() {
        let mut z = Zymic::new(100.0, 200.0);
        z.ferment = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_brewed);
    }

    #[test]
    fn tick_clears_just_spent() {
        let mut z = z();
        z.ferment = 10.0;
        z.exhaust(10.0);
        z.tick(0.016);
        assert!(!z.just_spent);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.ferment - 9.0).abs() < 1e-3);
    }

    // --- is_brewed / is_spent ---

    #[test]
    fn is_brewed_false_when_disabled() {
        let mut z = z();
        z.ferment = 100.0;
        z.enabled = false;
        assert!(!z.is_brewed());
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
