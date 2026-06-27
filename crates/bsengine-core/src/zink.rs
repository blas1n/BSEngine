use bevy_ecs::prelude::Component;

/// Breath-support tracker named after the Renaissance wind
/// instrument. `breath` builds via `blow(amount)` and replenishes
/// passively at `breath_rate` per second in `tick(dt)` or is
/// exhausted immediately via `exhale(amount)`.
///
/// Models wind-instrument breath-capacity fill levels, embouchure-
/// pressure accumulation bars, brass-embouchure endurance gauges,
/// reed-instrument breath-support trackers, woodwind phrasing
/// endurance meters, Renaissance-cornett wind-support fill levels,
/// sustained-tone breath-flow indicators, recorder-breath-column
/// saturation bars, singing-resonance breath-supply gauges, or any
/// mechanic where a player draws a full breath, shapes their lips
/// against the instrument's ivory mouthpiece, and releases
/// everything they have into a single suspended tone that hangs
/// in the chapel air until the last remaining column of breath runs
/// out and the note fades and everyone waits for the next phrase.
///
/// `blow(amount)` adds breath; fires `just_resonant` when first
/// reaching `max_breath`. No-op when disabled.
///
/// `exhale(amount)` reduces breath immediately; fires
/// `just_breathless` when reaching 0. No-op when disabled or
/// already breathless.
///
/// `tick(dt)` clears both flags, then increases breath by
/// `breath_rate * dt` (capped at `max_breath`). Fires
/// `just_resonant` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_resonant()` returns `breath >= max_breath && enabled`.
///
/// `is_breathless()` returns `breath == 0.0` (not gated by `enabled`).
///
/// `breath_fraction()` returns `(breath / max_breath).clamp(0, 1)`.
///
/// `effective_tone(scale)` returns `scale * breath_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — replenishes at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zink {
    pub breath: f32,
    pub max_breath: f32,
    pub breath_rate: f32,
    pub just_resonant: bool,
    pub just_breathless: bool,
    pub enabled: bool,
}

impl Zink {
    pub fn new(max_breath: f32, breath_rate: f32) -> Self {
        Self {
            breath: 0.0,
            max_breath: max_breath.max(0.1),
            breath_rate: breath_rate.max(0.0),
            just_resonant: false,
            just_breathless: false,
            enabled: true,
        }
    }

    /// Add breath; fires `just_resonant` when first reaching max.
    /// No-op when disabled.
    pub fn blow(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.breath < self.max_breath;
        self.breath = (self.breath + amount).min(self.max_breath);
        if was_below && self.breath >= self.max_breath {
            self.just_resonant = true;
        }
    }

    /// Reduce breath; fires `just_breathless` when reaching 0.
    /// No-op when disabled or already breathless.
    pub fn exhale(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.breath <= 0.0 {
            return;
        }
        self.breath = (self.breath - amount).max(0.0);
        if self.breath <= 0.0 {
            self.just_breathless = true;
        }
    }

    /// Clear flags, then increase breath by `breath_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_resonant = false;
        self.just_breathless = false;
        if self.enabled && self.breath_rate > 0.0 && self.breath < self.max_breath {
            let was_below = self.breath < self.max_breath;
            self.breath = (self.breath + self.breath_rate * dt).min(self.max_breath);
            if was_below && self.breath >= self.max_breath {
                self.just_resonant = true;
            }
        }
    }

    /// `true` when breath is at maximum and component is enabled.
    pub fn is_resonant(&self) -> bool {
        self.breath >= self.max_breath && self.enabled
    }

    /// `true` when breath is 0 (not gated by `enabled`).
    pub fn is_breathless(&self) -> bool {
        self.breath == 0.0
    }

    /// Fraction of maximum breath [0.0, 1.0].
    pub fn breath_fraction(&self) -> f32 {
        (self.breath / self.max_breath).clamp(0.0, 1.0)
    }

    /// Returns `scale * breath_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_tone(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.breath_fraction()
    }
}

impl Default for Zink {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zink {
        Zink::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_breathless() {
        let z = z();
        assert_eq!(z.breath, 0.0);
        assert!(z.is_breathless());
        assert!(!z.is_resonant());
    }

    #[test]
    fn new_clamps_max_breath() {
        let z = Zink::new(-5.0, 1.5);
        assert!((z.max_breath - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_breath_rate() {
        let z = Zink::new(100.0, -1.5);
        assert_eq!(z.breath_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zink::default();
        assert!((z.max_breath - 100.0).abs() < 1e-5);
        assert!((z.breath_rate - 1.5).abs() < 1e-5);
    }

    // --- blow ---

    #[test]
    fn blow_adds_breath() {
        let mut z = z();
        z.blow(40.0);
        assert!((z.breath - 40.0).abs() < 1e-3);
    }

    #[test]
    fn blow_clamps_at_max() {
        let mut z = z();
        z.blow(200.0);
        assert!((z.breath - 100.0).abs() < 1e-3);
    }

    #[test]
    fn blow_fires_just_resonant_at_max() {
        let mut z = z();
        z.blow(100.0);
        assert!(z.just_resonant);
        assert!(z.is_resonant());
    }

    #[test]
    fn blow_no_just_resonant_when_already_at_max() {
        let mut z = z();
        z.breath = 100.0;
        z.blow(10.0);
        assert!(!z.just_resonant);
    }

    #[test]
    fn blow_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.blow(50.0);
        assert_eq!(z.breath, 0.0);
    }

    #[test]
    fn blow_no_op_when_amount_zero() {
        let mut z = z();
        z.blow(0.0);
        assert_eq!(z.breath, 0.0);
    }

    // --- exhale ---

    #[test]
    fn exhale_reduces_breath() {
        let mut z = z();
        z.breath = 60.0;
        z.exhale(20.0);
        assert!((z.breath - 40.0).abs() < 1e-3);
    }

    #[test]
    fn exhale_clamps_at_zero() {
        let mut z = z();
        z.breath = 30.0;
        z.exhale(200.0);
        assert_eq!(z.breath, 0.0);
    }

    #[test]
    fn exhale_fires_just_breathless_at_zero() {
        let mut z = z();
        z.breath = 30.0;
        z.exhale(30.0);
        assert!(z.just_breathless);
    }

    #[test]
    fn exhale_no_op_when_already_breathless() {
        let mut z = z();
        z.exhale(10.0);
        assert!(!z.just_breathless);
    }

    #[test]
    fn exhale_no_op_when_disabled() {
        let mut z = z();
        z.breath = 50.0;
        z.enabled = false;
        z.exhale(50.0);
        assert!((z.breath - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_replenishes_breath() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.breath - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_resonant_on_replenish_to_max() {
        let mut z = Zink::new(100.0, 200.0);
        z.breath = 95.0;
        z.tick(1.0);
        assert!(z.just_resonant);
        assert!(z.is_resonant());
    }

    #[test]
    fn tick_no_replenish_when_already_resonant() {
        let mut z = z();
        z.breath = 100.0;
        z.tick(1.0);
        assert!(!z.just_resonant);
    }

    #[test]
    fn tick_no_replenish_when_rate_zero() {
        let mut z = Zink::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.breath, 0.0);
    }

    #[test]
    fn tick_no_replenish_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.breath, 0.0);
    }

    #[test]
    fn tick_clears_just_resonant() {
        let mut z = Zink::new(100.0, 200.0);
        z.breath = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_resonant);
    }

    #[test]
    fn tick_clears_just_breathless() {
        let mut z = z();
        z.breath = 10.0;
        z.exhale(10.0);
        z.tick(0.016);
        assert!(!z.just_breathless);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.breath - 9.0).abs() < 1e-3);
    }

    // --- is_resonant / is_breathless ---

    #[test]
    fn is_resonant_false_when_disabled() {
        let mut z = z();
        z.breath = 100.0;
        z.enabled = false;
        assert!(!z.is_resonant());
    }

    #[test]
    fn is_breathless_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_breathless());
    }

    // --- breath_fraction / effective_tone ---

    #[test]
    fn breath_fraction_zero_when_breathless() {
        assert_eq!(z().breath_fraction(), 0.0);
    }

    #[test]
    fn breath_fraction_half_at_midpoint() {
        let mut z = z();
        z.breath = 50.0;
        assert!((z.breath_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_tone_zero_when_breathless() {
        assert_eq!(z().effective_tone(100.0), 0.0);
    }

    #[test]
    fn effective_tone_scales_with_breath() {
        let mut z = z();
        z.breath = 75.0;
        assert!((z.effective_tone(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_tone_zero_when_disabled() {
        let mut z = z();
        z.breath = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_tone(100.0), 0.0);
    }
}
