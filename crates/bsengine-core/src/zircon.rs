use bevy_ecs::prelude::Component;

/// Gemstone-hardness tracker. `hardness` builds via `crystallize(amount)`
/// and grows passively at `harden_rate` per second in `tick(dt)` or
/// is fractured immediately via `fracture(amount)`.
///
/// Models zircon-crystal hardness meters, gemstone-clarity accumulation
/// bars, mineral-lattice integrity fill levels, gem-cutting readiness
/// gauges, crystal-growth progress trackers, jewel-hardness saturation
/// indicators, refractive-index build-up meters, lapidary-specimen
/// quality accumulators, or any mechanic where steady crystallization
/// produces a flawless gem hard enough to scratch glass.
///
/// `crystallize(amount)` adds hardness; fires `just_flawless` when first
/// reaching `max_hardness`. No-op when disabled.
///
/// `fracture(amount)` reduces hardness immediately; fires `just_fractured`
/// when reaching 0. No-op when disabled or already fractured.
///
/// `tick(dt)` clears both flags, then increases hardness by
/// `harden_rate * dt` (capped at `max_hardness`). Fires `just_flawless`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_flawless()` returns `hardness >= max_hardness && enabled`.
///
/// `is_fractured()` returns `hardness == 0.0` (not gated by `enabled`).
///
/// `hardness_fraction()` returns `(hardness / max_hardness).clamp(0, 1)`.
///
/// `effective_lustre(scale)` returns `scale * hardness_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — hardens at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zircon {
    pub hardness: f32,
    pub max_hardness: f32,
    pub harden_rate: f32,
    pub just_flawless: bool,
    pub just_fractured: bool,
    pub enabled: bool,
}

impl Zircon {
    pub fn new(max_hardness: f32, harden_rate: f32) -> Self {
        Self {
            hardness: 0.0,
            max_hardness: max_hardness.max(0.1),
            harden_rate: harden_rate.max(0.0),
            just_flawless: false,
            just_fractured: false,
            enabled: true,
        }
    }

    /// Add hardness; fires `just_flawless` when first reaching max.
    /// No-op when disabled.
    pub fn crystallize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.hardness < self.max_hardness;
        self.hardness = (self.hardness + amount).min(self.max_hardness);
        if was_below && self.hardness >= self.max_hardness {
            self.just_flawless = true;
        }
    }

    /// Reduce hardness; fires `just_fractured` when reaching 0.
    /// No-op when disabled or already fractured.
    pub fn fracture(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.hardness <= 0.0 {
            return;
        }
        self.hardness = (self.hardness - amount).max(0.0);
        if self.hardness <= 0.0 {
            self.just_fractured = true;
        }
    }

    /// Clear flags, then increase hardness by `harden_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_flawless = false;
        self.just_fractured = false;
        if self.enabled && self.harden_rate > 0.0 && self.hardness < self.max_hardness {
            let was_below = self.hardness < self.max_hardness;
            self.hardness = (self.hardness + self.harden_rate * dt).min(self.max_hardness);
            if was_below && self.hardness >= self.max_hardness {
                self.just_flawless = true;
            }
        }
    }

    /// `true` when hardness is at maximum and component is enabled.
    pub fn is_flawless(&self) -> bool {
        self.hardness >= self.max_hardness && self.enabled
    }

    /// `true` when hardness is 0 (not gated by `enabled`).
    pub fn is_fractured(&self) -> bool {
        self.hardness == 0.0
    }

    /// Fraction of maximum hardness [0.0, 1.0].
    pub fn hardness_fraction(&self) -> f32 {
        (self.hardness / self.max_hardness).clamp(0.0, 1.0)
    }

    /// Returns `scale * hardness_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_lustre(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.hardness_fraction()
    }
}

impl Default for Zircon {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zircon {
        Zircon::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_fractured() {
        let z = z();
        assert_eq!(z.hardness, 0.0);
        assert!(z.is_fractured());
        assert!(!z.is_flawless());
    }

    #[test]
    fn new_clamps_max_hardness() {
        let z = Zircon::new(-5.0, 2.0);
        assert!((z.max_hardness - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_harden_rate() {
        let z = Zircon::new(100.0, -3.0);
        assert_eq!(z.harden_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zircon::default();
        assert!((z.max_hardness - 100.0).abs() < 1e-5);
        assert!((z.harden_rate - 2.0).abs() < 1e-5);
    }

    // --- crystallize ---

    #[test]
    fn crystallize_adds_hardness() {
        let mut z = z();
        z.crystallize(40.0);
        assert!((z.hardness - 40.0).abs() < 1e-3);
    }

    #[test]
    fn crystallize_clamps_at_max() {
        let mut z = z();
        z.crystallize(200.0);
        assert!((z.hardness - 100.0).abs() < 1e-3);
    }

    #[test]
    fn crystallize_fires_just_flawless_at_max() {
        let mut z = z();
        z.crystallize(100.0);
        assert!(z.just_flawless);
        assert!(z.is_flawless());
    }

    #[test]
    fn crystallize_no_just_flawless_when_already_at_max() {
        let mut z = z();
        z.hardness = 100.0;
        z.crystallize(10.0);
        assert!(!z.just_flawless);
    }

    #[test]
    fn crystallize_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.crystallize(50.0);
        assert_eq!(z.hardness, 0.0);
    }

    #[test]
    fn crystallize_no_op_when_amount_zero() {
        let mut z = z();
        z.crystallize(0.0);
        assert_eq!(z.hardness, 0.0);
    }

    // --- fracture ---

    #[test]
    fn fracture_reduces_hardness() {
        let mut z = z();
        z.hardness = 60.0;
        z.fracture(20.0);
        assert!((z.hardness - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fracture_clamps_at_zero() {
        let mut z = z();
        z.hardness = 30.0;
        z.fracture(200.0);
        assert_eq!(z.hardness, 0.0);
    }

    #[test]
    fn fracture_fires_just_fractured_at_zero() {
        let mut z = z();
        z.hardness = 30.0;
        z.fracture(30.0);
        assert!(z.just_fractured);
    }

    #[test]
    fn fracture_no_op_when_already_fractured() {
        let mut z = z();
        z.fracture(10.0);
        assert!(!z.just_fractured);
    }

    #[test]
    fn fracture_no_op_when_disabled() {
        let mut z = z();
        z.hardness = 50.0;
        z.enabled = false;
        z.fracture(50.0);
        assert!((z.hardness - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_hardens() {
        let mut z = z(); // rate=2
        z.tick(1.0); // 0 + 2 = 2
        assert!((z.hardness - 2.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_flawless_on_harden_to_max() {
        let mut z = Zircon::new(100.0, 200.0);
        z.hardness = 95.0;
        z.tick(1.0);
        assert!(z.just_flawless);
        assert!(z.is_flawless());
    }

    #[test]
    fn tick_no_harden_when_already_flawless() {
        let mut z = z();
        z.hardness = 100.0;
        z.tick(1.0);
        assert!(!z.just_flawless);
    }

    #[test]
    fn tick_no_harden_when_rate_zero() {
        let mut z = Zircon::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.hardness, 0.0);
    }

    #[test]
    fn tick_no_harden_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.hardness, 0.0);
    }

    #[test]
    fn tick_clears_just_flawless() {
        let mut z = Zircon::new(100.0, 200.0);
        z.hardness = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_flawless);
    }

    #[test]
    fn tick_clears_just_fractured() {
        let mut z = z();
        z.hardness = 10.0;
        z.fracture(10.0);
        z.tick(0.016);
        assert!(!z.just_fractured);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.hardness - 10.0).abs() < 1e-3);
    }

    // --- is_flawless / is_fractured ---

    #[test]
    fn is_flawless_false_when_disabled() {
        let mut z = z();
        z.hardness = 100.0;
        z.enabled = false;
        assert!(!z.is_flawless());
    }

    #[test]
    fn is_fractured_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_fractured());
    }

    // --- hardness_fraction / effective_lustre ---

    #[test]
    fn hardness_fraction_zero_when_fractured() {
        assert_eq!(z().hardness_fraction(), 0.0);
    }

    #[test]
    fn hardness_fraction_half_at_midpoint() {
        let mut z = z();
        z.hardness = 50.0;
        assert!((z.hardness_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_lustre_zero_when_fractured() {
        assert_eq!(z().effective_lustre(100.0), 0.0);
    }

    #[test]
    fn effective_lustre_scales_with_hardness() {
        let mut z = z();
        z.hardness = 70.0;
        assert!((z.effective_lustre(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_lustre_zero_when_disabled() {
        let mut z = z();
        z.hardness = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_lustre(100.0), 0.0);
    }
}
