use bevy_ecs::prelude::Component;

/// Rhythmic-chant / repetition tracker. `resonance` builds via
/// `intone(amount)` and hums up passively at `hum_rate` per second in
/// `tick(dt)` or is broken immediately via `hush(amount)`.
///
/// Models mantra-meter gauges, prayer-charge bars, chant-power
/// accumulators, NPC-ritual-progress trackers, sound-resonance
/// amplifiers, echo-chamber fill levels, or any mechanic where
/// sustained repetition of an action builds up a harmonic charge
/// that is instantly broken by interruption.
///
/// `intone(amount)` adds resonance; fires `just_resonant` when first
/// reaching `max_resonance`. No-op when disabled.
///
/// `hush(amount)` reduces resonance immediately; fires `just_hushed`
/// when reaching 0. No-op when disabled or already hushed.
///
/// `tick(dt)` clears both flags, then increases resonance by
/// `hum_rate * dt` (capped at `max_resonance`). Fires `just_resonant`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_resonant()` returns `resonance >= max_resonance && enabled`.
///
/// `is_hushed()` returns `resonance == 0.0` (not gated by `enabled`).
///
/// `resonance_fraction()` returns `(resonance / max_resonance).clamp(0, 1)`.
///
/// `effective_chant(scale)` returns `scale * resonance_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 7.0)` — hums at 7 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zikr {
    pub resonance: f32,
    pub max_resonance: f32,
    pub hum_rate: f32,
    pub just_resonant: bool,
    pub just_hushed: bool,
    pub enabled: bool,
}

impl Zikr {
    pub fn new(max_resonance: f32, hum_rate: f32) -> Self {
        Self {
            resonance: 0.0,
            max_resonance: max_resonance.max(0.1),
            hum_rate: hum_rate.max(0.0),
            just_resonant: false,
            just_hushed: false,
            enabled: true,
        }
    }

    /// Add resonance; fires `just_resonant` when first reaching max.
    /// No-op when disabled.
    pub fn intone(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.resonance < self.max_resonance;
        self.resonance = (self.resonance + amount).min(self.max_resonance);
        if was_below && self.resonance >= self.max_resonance {
            self.just_resonant = true;
        }
    }

    /// Reduce resonance; fires `just_hushed` when reaching 0.
    /// No-op when disabled or already hushed.
    pub fn hush(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.resonance <= 0.0 {
            return;
        }
        self.resonance = (self.resonance - amount).max(0.0);
        if self.resonance <= 0.0 {
            self.just_hushed = true;
        }
    }

    /// Clear flags, then increase resonance by `hum_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_resonant = false;
        self.just_hushed = false;
        if self.enabled && self.hum_rate > 0.0 && self.resonance < self.max_resonance {
            let was_below = self.resonance < self.max_resonance;
            self.resonance = (self.resonance + self.hum_rate * dt).min(self.max_resonance);
            if was_below && self.resonance >= self.max_resonance {
                self.just_resonant = true;
            }
        }
    }

    /// `true` when resonance is at maximum and component is enabled.
    pub fn is_resonant(&self) -> bool {
        self.resonance >= self.max_resonance && self.enabled
    }

    /// `true` when resonance is 0 (not gated by `enabled`).
    pub fn is_hushed(&self) -> bool {
        self.resonance == 0.0
    }

    /// Fraction of maximum resonance [0.0, 1.0].
    pub fn resonance_fraction(&self) -> f32 {
        (self.resonance / self.max_resonance).clamp(0.0, 1.0)
    }

    /// Returns `scale * resonance_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_chant(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.resonance_fraction()
    }
}

impl Default for Zikr {
    fn default() -> Self {
        Self::new(100.0, 7.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zikr {
        Zikr::new(100.0, 7.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_hushed() {
        let z = z();
        assert_eq!(z.resonance, 0.0);
        assert!(z.is_hushed());
        assert!(!z.is_resonant());
    }

    #[test]
    fn new_clamps_max_resonance() {
        let z = Zikr::new(-5.0, 7.0);
        assert!((z.max_resonance - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_hum_rate() {
        let z = Zikr::new(100.0, -3.0);
        assert_eq!(z.hum_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zikr::default();
        assert!((z.max_resonance - 100.0).abs() < 1e-5);
        assert!((z.hum_rate - 7.0).abs() < 1e-5);
    }

    // --- intone ---

    #[test]
    fn intone_adds_resonance() {
        let mut z = z();
        z.intone(40.0);
        assert!((z.resonance - 40.0).abs() < 1e-3);
    }

    #[test]
    fn intone_clamps_at_max() {
        let mut z = z();
        z.intone(200.0);
        assert!((z.resonance - 100.0).abs() < 1e-3);
    }

    #[test]
    fn intone_fires_just_resonant_at_max() {
        let mut z = z();
        z.intone(100.0);
        assert!(z.just_resonant);
        assert!(z.is_resonant());
    }

    #[test]
    fn intone_no_just_resonant_when_already_at_max() {
        let mut z = z();
        z.resonance = 100.0;
        z.intone(10.0);
        assert!(!z.just_resonant);
    }

    #[test]
    fn intone_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.intone(50.0);
        assert_eq!(z.resonance, 0.0);
    }

    #[test]
    fn intone_no_op_when_amount_zero() {
        let mut z = z();
        z.intone(0.0);
        assert_eq!(z.resonance, 0.0);
    }

    // --- hush ---

    #[test]
    fn hush_reduces_resonance() {
        let mut z = z();
        z.resonance = 60.0;
        z.hush(20.0);
        assert!((z.resonance - 40.0).abs() < 1e-3);
    }

    #[test]
    fn hush_clamps_at_zero() {
        let mut z = z();
        z.resonance = 30.0;
        z.hush(200.0);
        assert_eq!(z.resonance, 0.0);
    }

    #[test]
    fn hush_fires_just_hushed_at_zero() {
        let mut z = z();
        z.resonance = 30.0;
        z.hush(30.0);
        assert!(z.just_hushed);
    }

    #[test]
    fn hush_no_op_when_already_hushed() {
        let mut z = z();
        z.hush(10.0);
        assert!(!z.just_hushed);
    }

    #[test]
    fn hush_no_op_when_disabled() {
        let mut z = z();
        z.resonance = 50.0;
        z.enabled = false;
        z.hush(50.0);
        assert!((z.resonance - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_hums_resonance() {
        let mut z = z(); // rate=7
        z.tick(1.0); // 0 + 7 = 7
        assert!((z.resonance - 7.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_resonant_on_hum_to_max() {
        let mut z = Zikr::new(100.0, 200.0);
        z.resonance = 95.0;
        z.tick(1.0);
        assert!(z.just_resonant);
        assert!(z.is_resonant());
    }

    #[test]
    fn tick_no_hum_when_already_resonant() {
        let mut z = z();
        z.resonance = 100.0;
        z.tick(1.0);
        assert!(!z.just_resonant);
    }

    #[test]
    fn tick_no_hum_when_rate_zero() {
        let mut z = Zikr::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.resonance, 0.0);
    }

    #[test]
    fn tick_no_hum_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.resonance, 0.0);
    }

    #[test]
    fn tick_clears_just_resonant() {
        let mut z = Zikr::new(100.0, 200.0);
        z.resonance = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_resonant);
    }

    #[test]
    fn tick_clears_just_hushed() {
        let mut z = z();
        z.resonance = 10.0;
        z.hush(10.0);
        z.tick(0.016);
        assert!(!z.just_hushed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=7
        z.tick(4.0); // 7*4 = 28
        assert!((z.resonance - 28.0).abs() < 1e-3);
    }

    // --- is_resonant / is_hushed ---

    #[test]
    fn is_resonant_false_when_disabled() {
        let mut z = z();
        z.resonance = 100.0;
        z.enabled = false;
        assert!(!z.is_resonant());
    }

    #[test]
    fn is_hushed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_hushed());
    }

    // --- resonance_fraction / effective_chant ---

    #[test]
    fn resonance_fraction_zero_when_hushed() {
        assert_eq!(z().resonance_fraction(), 0.0);
    }

    #[test]
    fn resonance_fraction_half_at_midpoint() {
        let mut z = z();
        z.resonance = 50.0;
        assert!((z.resonance_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_chant_zero_when_hushed() {
        assert_eq!(z().effective_chant(100.0), 0.0);
    }

    #[test]
    fn effective_chant_scales_with_resonance() {
        let mut z = z();
        z.resonance = 85.0;
        assert!((z.effective_chant(100.0) - 85.0).abs() < 1e-3);
    }

    #[test]
    fn effective_chant_zero_when_disabled() {
        let mut z = z();
        z.resonance = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_chant(100.0), 0.0);
    }
}
