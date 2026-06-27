use bevy_ecs::prelude::Component;

/// Resonance/vibration tracker. `resonance` builds via `pluck(amount)` and
/// decays passively at `decay_rate` per second in `tick(dt)` or immediately
/// via `mute(amount)`.
///
/// Models instrument string sustain, acoustic resonance chambers, sound
/// propagation meters, sympathetic vibration gauges, tuning-fork timers,
/// or any mechanic where an excited resonance fades unless re-excited.
///
/// `pluck(amount)` adds resonance; fires `just_harmonized` when first
/// reaching `max_resonance`. No-op when disabled.
///
/// `mute(amount)` reduces resonance immediately; fires `just_silenced`
/// when reaching 0. No-op when disabled or already silent.
///
/// `tick(dt)` clears both flags, then decays resonance by
/// `decay_rate * dt` (floored at 0). Fires `just_silenced` when reaching
/// 0 via decay. No-op when disabled or rate is 0.
///
/// `is_harmonized()` returns `resonance >= max_resonance && enabled`.
///
/// `is_silent()` returns `resonance == 0.0` (not gated by `enabled`).
///
/// `resonance_fraction()` returns `(resonance / max_resonance).clamp(0, 1)`.
///
/// `effective_tone(scale)` returns `scale * resonance_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 15.0)` — decays at 15 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zither {
    pub resonance: f32,
    pub max_resonance: f32,
    pub decay_rate: f32,
    pub just_harmonized: bool,
    pub just_silenced: bool,
    pub enabled: bool,
}

impl Zither {
    pub fn new(max_resonance: f32, decay_rate: f32) -> Self {
        Self {
            resonance: 0.0,
            max_resonance: max_resonance.max(0.1),
            decay_rate: decay_rate.max(0.0),
            just_harmonized: false,
            just_silenced: false,
            enabled: true,
        }
    }

    /// Add resonance; fires `just_harmonized` when first reaching max.
    /// No-op when disabled.
    pub fn pluck(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.resonance < self.max_resonance;
        self.resonance = (self.resonance + amount).min(self.max_resonance);
        if was_below && self.resonance >= self.max_resonance {
            self.just_harmonized = true;
        }
    }

    /// Reduce resonance; fires `just_silenced` when reaching 0.
    /// No-op when disabled or already silent.
    pub fn mute(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.resonance <= 0.0 {
            return;
        }
        self.resonance = (self.resonance - amount).max(0.0);
        if self.resonance <= 0.0 {
            self.just_silenced = true;
        }
    }

    /// Clear flags, then decay resonance by `decay_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_harmonized = false;
        self.just_silenced = false;
        if self.enabled && self.decay_rate > 0.0 && self.resonance > 0.0 {
            self.resonance = (self.resonance - self.decay_rate * dt).max(0.0);
            if self.resonance <= 0.0 {
                self.just_silenced = true;
            }
        }
    }

    /// `true` when resonance is at maximum and component is enabled.
    pub fn is_harmonized(&self) -> bool {
        self.resonance >= self.max_resonance && self.enabled
    }

    /// `true` when resonance is 0 (not gated by `enabled`).
    pub fn is_silent(&self) -> bool {
        self.resonance == 0.0
    }

    /// Fraction of maximum resonance [0.0, 1.0].
    pub fn resonance_fraction(&self) -> f32 {
        (self.resonance / self.max_resonance).clamp(0.0, 1.0)
    }

    /// Returns `scale * resonance_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_tone(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.resonance_fraction()
    }
}

impl Default for Zither {
    fn default() -> Self {
        Self::new(100.0, 15.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zither {
        Zither::new(100.0, 15.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_silent() {
        let z = z();
        assert_eq!(z.resonance, 0.0);
        assert!(z.is_silent());
        assert!(!z.is_harmonized());
    }

    #[test]
    fn new_clamps_max_resonance() {
        let z = Zither::new(-5.0, 15.0);
        assert!((z.max_resonance - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_decay_rate() {
        let z = Zither::new(100.0, -3.0);
        assert_eq!(z.decay_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zither::default();
        assert!((z.max_resonance - 100.0).abs() < 1e-5);
        assert!((z.decay_rate - 15.0).abs() < 1e-5);
    }

    // --- pluck ---

    #[test]
    fn pluck_adds_resonance() {
        let mut z = z();
        z.pluck(40.0);
        assert!((z.resonance - 40.0).abs() < 1e-3);
    }

    #[test]
    fn pluck_clamps_at_max() {
        let mut z = z();
        z.pluck(200.0);
        assert!((z.resonance - 100.0).abs() < 1e-3);
    }

    #[test]
    fn pluck_fires_just_harmonized_at_max() {
        let mut z = z();
        z.pluck(100.0);
        assert!(z.just_harmonized);
        assert!(z.is_harmonized());
    }

    #[test]
    fn pluck_no_just_harmonized_when_already_at_max() {
        let mut z = z();
        z.resonance = 100.0;
        z.pluck(10.0);
        assert!(!z.just_harmonized);
    }

    #[test]
    fn pluck_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.pluck(50.0);
        assert_eq!(z.resonance, 0.0);
    }

    #[test]
    fn pluck_no_op_when_amount_zero() {
        let mut z = z();
        z.pluck(0.0);
        assert_eq!(z.resonance, 0.0);
    }

    // --- mute ---

    #[test]
    fn mute_reduces_resonance() {
        let mut z = z();
        z.resonance = 60.0;
        z.mute(20.0);
        assert!((z.resonance - 40.0).abs() < 1e-3);
    }

    #[test]
    fn mute_clamps_at_zero() {
        let mut z = z();
        z.resonance = 30.0;
        z.mute(200.0);
        assert_eq!(z.resonance, 0.0);
    }

    #[test]
    fn mute_fires_just_silenced_at_zero() {
        let mut z = z();
        z.resonance = 30.0;
        z.mute(30.0);
        assert!(z.just_silenced);
    }

    #[test]
    fn mute_no_op_when_already_silent() {
        let mut z = z();
        z.mute(10.0);
        assert!(!z.just_silenced);
    }

    #[test]
    fn mute_no_op_when_disabled() {
        let mut z = z();
        z.resonance = 50.0;
        z.enabled = false;
        z.mute(50.0);
        assert!((z.resonance - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_decays_resonance() {
        let mut z = z(); // decay=15
        z.resonance = 60.0;
        z.tick(1.0); // 60 - 15 = 45
        assert!((z.resonance - 45.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_silenced_on_decay_to_zero() {
        let mut z = Zither::new(100.0, 200.0);
        z.resonance = 5.0;
        z.tick(1.0);
        assert!(z.just_silenced);
        assert!(z.is_silent());
    }

    #[test]
    fn tick_no_decay_when_already_silent() {
        let mut z = z();
        z.tick(10.0);
        assert!(!z.just_silenced);
    }

    #[test]
    fn tick_no_decay_when_rate_zero() {
        let mut z = Zither::new(100.0, 0.0);
        z.resonance = 50.0;
        z.tick(100.0);
        assert!((z.resonance - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_decay_when_disabled() {
        let mut z = z();
        z.resonance = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.resonance - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_harmonized() {
        let mut z = z();
        z.pluck(100.0);
        z.tick(0.016);
        assert!(!z.just_harmonized);
    }

    #[test]
    fn tick_clears_just_silenced() {
        let mut z = Zither::new(100.0, 200.0);
        z.resonance = 5.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_silenced);
    }

    #[test]
    fn tick_scales_decay_with_dt() {
        let mut z = z(); // decay=15
        z.resonance = 100.0;
        z.tick(2.0); // 100 - 15*2 = 70
        assert!((z.resonance - 70.0).abs() < 1e-3);
    }

    // --- is_harmonized / is_silent ---

    #[test]
    fn is_harmonized_false_when_disabled() {
        let mut z = z();
        z.resonance = 100.0;
        z.enabled = false;
        assert!(!z.is_harmonized());
    }

    #[test]
    fn is_silent_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_silent());
    }

    // --- resonance_fraction / effective_tone ---

    #[test]
    fn resonance_fraction_zero_when_silent() {
        assert_eq!(z().resonance_fraction(), 0.0);
    }

    #[test]
    fn resonance_fraction_half_at_midpoint() {
        let mut z = z();
        z.resonance = 50.0;
        assert!((z.resonance_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_tone_zero_when_silent() {
        assert_eq!(z().effective_tone(100.0), 0.0);
    }

    #[test]
    fn effective_tone_scales_with_resonance() {
        let mut z = z();
        z.resonance = 80.0;
        assert!((z.effective_tone(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_tone_zero_when_disabled() {
        let mut z = z();
        z.resonance = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_tone(100.0), 0.0);
    }
}
