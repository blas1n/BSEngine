use bevy_ecs::prelude::Component;

/// Life-vitality tracker. `vitality` builds via `quicken(amount)` and
/// increases passively at `quicken_rate` per second in `tick(dt)` or is
/// diminished immediately via `diminish(amount)`.
///
/// Models life-force saturation bars, vital-energy fill levels,
/// bio-energy accumulation gauges, chi-flow intensity trackers,
/// animating-spirit capacity indicators, life-spark restoration meters,
/// cellular-vitality health bars, essence-of-life saturation fill levels,
/// élan-vital build-up trackers, or any mechanic where the raw animating
/// principle of biology saturates every cell of a living organism until it
/// is so full of vital energy that it practically hums — only for some
/// drain or shock to rob it of that precious spark and leave nothing
/// behind but chemistry.
///
/// `quicken(amount)` adds vitality; fires `just_vital` when first
/// reaching `max_vitality`. No-op when disabled.
///
/// `diminish(amount)` reduces vitality immediately; fires `just_dormant`
/// when reaching 0. No-op when disabled or already dormant.
///
/// `tick(dt)` clears both flags, then increases vitality by
/// `quicken_rate * dt` (capped at `max_vitality`). Fires `just_vital`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_vital()` returns `vitality >= max_vitality && enabled`.
///
/// `is_dormant()` returns `vitality == 0.0` (not gated by `enabled`).
///
/// `vitality_fraction()` returns `(vitality / max_vitality).clamp(0, 1)`.
///
/// `effective_vigor(scale)` returns `scale * vitality_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.5)` — quickens at 2.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoetic {
    pub vitality: f32,
    pub max_vitality: f32,
    pub quicken_rate: f32,
    pub just_vital: bool,
    pub just_dormant: bool,
    pub enabled: bool,
}

impl Zoetic {
    pub fn new(max_vitality: f32, quicken_rate: f32) -> Self {
        Self {
            vitality: 0.0,
            max_vitality: max_vitality.max(0.1),
            quicken_rate: quicken_rate.max(0.0),
            just_vital: false,
            just_dormant: false,
            enabled: true,
        }
    }

    /// Add vitality; fires `just_vital` when first reaching max.
    /// No-op when disabled.
    pub fn quicken(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.vitality < self.max_vitality;
        self.vitality = (self.vitality + amount).min(self.max_vitality);
        if was_below && self.vitality >= self.max_vitality {
            self.just_vital = true;
        }
    }

    /// Reduce vitality; fires `just_dormant` when reaching 0.
    /// No-op when disabled or already dormant.
    pub fn diminish(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.vitality <= 0.0 {
            return;
        }
        self.vitality = (self.vitality - amount).max(0.0);
        if self.vitality <= 0.0 {
            self.just_dormant = true;
        }
    }

    /// Clear flags, then increase vitality by `quicken_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_vital = false;
        self.just_dormant = false;
        if self.enabled && self.quicken_rate > 0.0 && self.vitality < self.max_vitality {
            let was_below = self.vitality < self.max_vitality;
            self.vitality = (self.vitality + self.quicken_rate * dt).min(self.max_vitality);
            if was_below && self.vitality >= self.max_vitality {
                self.just_vital = true;
            }
        }
    }

    /// `true` when vitality is at maximum and component is enabled.
    pub fn is_vital(&self) -> bool {
        self.vitality >= self.max_vitality && self.enabled
    }

    /// `true` when vitality is 0 (not gated by `enabled`).
    pub fn is_dormant(&self) -> bool {
        self.vitality == 0.0
    }

    /// Fraction of maximum vitality [0.0, 1.0].
    pub fn vitality_fraction(&self) -> f32 {
        (self.vitality / self.max_vitality).clamp(0.0, 1.0)
    }

    /// Returns `scale * vitality_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vigor(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.vitality_fraction()
    }
}

impl Default for Zoetic {
    fn default() -> Self {
        Self::new(100.0, 2.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoetic {
        Zoetic::new(100.0, 2.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dormant() {
        let z = z();
        assert_eq!(z.vitality, 0.0);
        assert!(z.is_dormant());
        assert!(!z.is_vital());
    }

    #[test]
    fn new_clamps_max_vitality() {
        let z = Zoetic::new(-5.0, 2.5);
        assert!((z.max_vitality - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_quicken_rate() {
        let z = Zoetic::new(100.0, -3.0);
        assert_eq!(z.quicken_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoetic::default();
        assert!((z.max_vitality - 100.0).abs() < 1e-5);
        assert!((z.quicken_rate - 2.5).abs() < 1e-5);
    }

    // --- quicken ---

    #[test]
    fn quicken_adds_vitality() {
        let mut z = z();
        z.quicken(40.0);
        assert!((z.vitality - 40.0).abs() < 1e-3);
    }

    #[test]
    fn quicken_clamps_at_max() {
        let mut z = z();
        z.quicken(200.0);
        assert!((z.vitality - 100.0).abs() < 1e-3);
    }

    #[test]
    fn quicken_fires_just_vital_at_max() {
        let mut z = z();
        z.quicken(100.0);
        assert!(z.just_vital);
        assert!(z.is_vital());
    }

    #[test]
    fn quicken_no_just_vital_when_already_at_max() {
        let mut z = z();
        z.vitality = 100.0;
        z.quicken(10.0);
        assert!(!z.just_vital);
    }

    #[test]
    fn quicken_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.quicken(50.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn quicken_no_op_when_amount_zero() {
        let mut z = z();
        z.quicken(0.0);
        assert_eq!(z.vitality, 0.0);
    }

    // --- diminish ---

    #[test]
    fn diminish_reduces_vitality() {
        let mut z = z();
        z.vitality = 60.0;
        z.diminish(20.0);
        assert!((z.vitality - 40.0).abs() < 1e-3);
    }

    #[test]
    fn diminish_clamps_at_zero() {
        let mut z = z();
        z.vitality = 30.0;
        z.diminish(200.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn diminish_fires_just_dormant_at_zero() {
        let mut z = z();
        z.vitality = 30.0;
        z.diminish(30.0);
        assert!(z.just_dormant);
    }

    #[test]
    fn diminish_no_op_when_already_dormant() {
        let mut z = z();
        z.diminish(10.0);
        assert!(!z.just_dormant);
    }

    #[test]
    fn diminish_no_op_when_disabled() {
        let mut z = z();
        z.vitality = 50.0;
        z.enabled = false;
        z.diminish(50.0);
        assert!((z.vitality - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_quickens_vitality() {
        let mut z = z(); // rate=2.5
        z.tick(2.0); // 0 + 2.5*2 = 5
        assert!((z.vitality - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_vital_on_quicken_to_max() {
        let mut z = Zoetic::new(100.0, 200.0);
        z.vitality = 95.0;
        z.tick(1.0);
        assert!(z.just_vital);
        assert!(z.is_vital());
    }

    #[test]
    fn tick_no_quicken_when_already_vital() {
        let mut z = z();
        z.vitality = 100.0;
        z.tick(1.0);
        assert!(!z.just_vital);
    }

    #[test]
    fn tick_no_quicken_when_rate_zero() {
        let mut z = Zoetic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn tick_no_quicken_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn tick_clears_just_vital() {
        let mut z = Zoetic::new(100.0, 200.0);
        z.vitality = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_vital);
    }

    #[test]
    fn tick_clears_just_dormant() {
        let mut z = z();
        z.vitality = 10.0;
        z.diminish(10.0);
        z.tick(0.016);
        assert!(!z.just_dormant);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2.5
        z.tick(4.0); // 2.5*4 = 10
        assert!((z.vitality - 10.0).abs() < 1e-3);
    }

    // --- is_vital / is_dormant ---

    #[test]
    fn is_vital_false_when_disabled() {
        let mut z = z();
        z.vitality = 100.0;
        z.enabled = false;
        assert!(!z.is_vital());
    }

    #[test]
    fn is_dormant_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_dormant());
    }

    // --- vitality_fraction / effective_vigor ---

    #[test]
    fn vitality_fraction_zero_when_dormant() {
        assert_eq!(z().vitality_fraction(), 0.0);
    }

    #[test]
    fn vitality_fraction_half_at_midpoint() {
        let mut z = z();
        z.vitality = 50.0;
        assert!((z.vitality_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_vigor_zero_when_dormant() {
        assert_eq!(z().effective_vigor(100.0), 0.0);
    }

    #[test]
    fn effective_vigor_scales_with_vitality() {
        let mut z = z();
        z.vitality = 75.0;
        assert!((z.effective_vigor(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vigor_zero_when_disabled() {
        let mut z = z();
        z.vitality = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_vigor(100.0), 0.0);
    }
}
