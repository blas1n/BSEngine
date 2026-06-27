use bevy_ecs::prelude::Component;

/// Stripe-pattern saturation tracker. `striping` builds via `mark(amount)`
/// and intensifies passively at `pattern_rate` per second in `tick(dt)` or
/// is bleached via `bleach(amount)`.
///
/// Models zebra-stripe pattern-coverage bars, hide-banding saturation
/// gauges, territorial-stripe marking fill levels, pelage-pattern density
/// trackers, contrasting-coloration saturation meters, camouflage-band
/// build-up indicators, skin-marking stripe accumulation bars, warning-
/// coloration intensity gauges, biometric-stripe distinctiveness fill
/// levels, or any mechanic where alternating high-contrast bands build
/// across a surface until every millimetre carries a definitive dark
/// stripe — or a bleaching event strips the pattern back to uniform grey.
///
/// `mark(amount)` adds striping; fires `just_banded` when first
/// reaching `max_striping`. No-op when disabled.
///
/// `bleach(amount)` reduces striping immediately; fires `just_blank`
/// when reaching 0. No-op when disabled or already blank.
///
/// `tick(dt)` clears both flags, then increases striping by
/// `pattern_rate * dt` (capped at `max_striping`). Fires `just_banded`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_banded()` returns `striping >= max_striping && enabled`.
///
/// `is_blank()` returns `striping == 0.0` (not gated by `enabled`).
///
/// `striping_fraction()` returns `(striping / max_striping).clamp(0, 1)`.
///
/// `effective_contrast(scale)` returns `scale * striping_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — patterns at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zebrine {
    pub striping: f32,
    pub max_striping: f32,
    pub pattern_rate: f32,
    pub just_banded: bool,
    pub just_blank: bool,
    pub enabled: bool,
}

impl Zebrine {
    pub fn new(max_striping: f32, pattern_rate: f32) -> Self {
        Self {
            striping: 0.0,
            max_striping: max_striping.max(0.1),
            pattern_rate: pattern_rate.max(0.0),
            just_banded: false,
            just_blank: false,
            enabled: true,
        }
    }

    /// Add striping; fires `just_banded` when first reaching max.
    /// No-op when disabled.
    pub fn mark(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.striping < self.max_striping;
        self.striping = (self.striping + amount).min(self.max_striping);
        if was_below && self.striping >= self.max_striping {
            self.just_banded = true;
        }
    }

    /// Reduce striping; fires `just_blank` when reaching 0.
    /// No-op when disabled or already blank.
    pub fn bleach(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.striping <= 0.0 {
            return;
        }
        self.striping = (self.striping - amount).max(0.0);
        if self.striping <= 0.0 {
            self.just_blank = true;
        }
    }

    /// Clear flags, then increase striping by `pattern_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_banded = false;
        self.just_blank = false;
        if self.enabled && self.pattern_rate > 0.0 && self.striping < self.max_striping {
            let was_below = self.striping < self.max_striping;
            self.striping = (self.striping + self.pattern_rate * dt).min(self.max_striping);
            if was_below && self.striping >= self.max_striping {
                self.just_banded = true;
            }
        }
    }

    /// `true` when striping is at maximum and component is enabled.
    pub fn is_banded(&self) -> bool {
        self.striping >= self.max_striping && self.enabled
    }

    /// `true` when striping is 0 (not gated by `enabled`).
    pub fn is_blank(&self) -> bool {
        self.striping == 0.0
    }

    /// Fraction of maximum striping [0.0, 1.0].
    pub fn striping_fraction(&self) -> f32 {
        (self.striping / self.max_striping).clamp(0.0, 1.0)
    }

    /// Returns `scale * striping_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_contrast(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.striping_fraction()
    }
}

impl Default for Zebrine {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zebrine {
        Zebrine::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_blank() {
        let z = z();
        assert_eq!(z.striping, 0.0);
        assert!(z.is_blank());
        assert!(!z.is_banded());
    }

    #[test]
    fn new_clamps_max_striping() {
        let z = Zebrine::new(-5.0, 2.0);
        assert!((z.max_striping - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_pattern_rate() {
        let z = Zebrine::new(100.0, -2.0);
        assert_eq!(z.pattern_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zebrine::default();
        assert!((z.max_striping - 100.0).abs() < 1e-5);
        assert!((z.pattern_rate - 2.0).abs() < 1e-5);
    }

    // --- mark ---

    #[test]
    fn mark_adds_striping() {
        let mut z = z();
        z.mark(40.0);
        assert!((z.striping - 40.0).abs() < 1e-3);
    }

    #[test]
    fn mark_clamps_at_max() {
        let mut z = z();
        z.mark(200.0);
        assert!((z.striping - 100.0).abs() < 1e-3);
    }

    #[test]
    fn mark_fires_just_banded_at_max() {
        let mut z = z();
        z.mark(100.0);
        assert!(z.just_banded);
        assert!(z.is_banded());
    }

    #[test]
    fn mark_no_just_banded_when_already_at_max() {
        let mut z = z();
        z.striping = 100.0;
        z.mark(10.0);
        assert!(!z.just_banded);
    }

    #[test]
    fn mark_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.mark(50.0);
        assert_eq!(z.striping, 0.0);
    }

    #[test]
    fn mark_no_op_when_amount_zero() {
        let mut z = z();
        z.mark(0.0);
        assert_eq!(z.striping, 0.0);
    }

    // --- bleach ---

    #[test]
    fn bleach_reduces_striping() {
        let mut z = z();
        z.striping = 60.0;
        z.bleach(20.0);
        assert!((z.striping - 40.0).abs() < 1e-3);
    }

    #[test]
    fn bleach_clamps_at_zero() {
        let mut z = z();
        z.striping = 30.0;
        z.bleach(200.0);
        assert_eq!(z.striping, 0.0);
    }

    #[test]
    fn bleach_fires_just_blank_at_zero() {
        let mut z = z();
        z.striping = 30.0;
        z.bleach(30.0);
        assert!(z.just_blank);
    }

    #[test]
    fn bleach_no_op_when_already_blank() {
        let mut z = z();
        z.bleach(10.0);
        assert!(!z.just_blank);
    }

    #[test]
    fn bleach_no_op_when_disabled() {
        let mut z = z();
        z.striping = 50.0;
        z.enabled = false;
        z.bleach(50.0);
        assert!((z.striping - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_patterns_striping() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.striping - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_banded_on_pattern_to_max() {
        let mut z = Zebrine::new(100.0, 200.0);
        z.striping = 95.0;
        z.tick(1.0);
        assert!(z.just_banded);
        assert!(z.is_banded());
    }

    #[test]
    fn tick_no_pattern_when_already_banded() {
        let mut z = z();
        z.striping = 100.0;
        z.tick(1.0);
        assert!(!z.just_banded);
    }

    #[test]
    fn tick_no_pattern_when_rate_zero() {
        let mut z = Zebrine::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.striping, 0.0);
    }

    #[test]
    fn tick_no_pattern_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.striping, 0.0);
    }

    #[test]
    fn tick_clears_just_banded() {
        let mut z = Zebrine::new(100.0, 200.0);
        z.striping = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_banded);
    }

    #[test]
    fn tick_clears_just_blank() {
        let mut z = z();
        z.striping = 10.0;
        z.bleach(10.0);
        z.tick(0.016);
        assert!(!z.just_blank);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.striping - 10.0).abs() < 1e-3);
    }

    // --- is_banded / is_blank ---

    #[test]
    fn is_banded_false_when_disabled() {
        let mut z = z();
        z.striping = 100.0;
        z.enabled = false;
        assert!(!z.is_banded());
    }

    #[test]
    fn is_blank_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_blank());
    }

    // --- striping_fraction / effective_contrast ---

    #[test]
    fn striping_fraction_zero_when_blank() {
        assert_eq!(z().striping_fraction(), 0.0);
    }

    #[test]
    fn striping_fraction_half_at_midpoint() {
        let mut z = z();
        z.striping = 50.0;
        assert!((z.striping_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_contrast_zero_when_blank() {
        assert_eq!(z().effective_contrast(100.0), 0.0);
    }

    #[test]
    fn effective_contrast_scales_with_striping() {
        let mut z = z();
        z.striping = 75.0;
        assert!((z.effective_contrast(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_contrast_zero_when_disabled() {
        let mut z = z();
        z.striping = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_contrast(100.0), 0.0);
    }
}
