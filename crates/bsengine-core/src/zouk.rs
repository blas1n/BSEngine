use bevy_ecs::prelude::Component;

/// Caribbean-dance-music rhythm tracker. `rhythm` builds via `pulse(amount)`
/// and intensifies passively at `pulse_rate` per second in `tick(dt)` or
/// fades immediately via `quiet(amount)`.
///
/// Models DJ-set energy meters, dance-floor intensity fill levels,
/// Caribbean-music festival beat accumulators, club-scene atmosphere
/// gauges, Antillean-groove build-up trackers, zouk-night crowd-energy
/// indicators, bass-line saturation bars, Guadeloupean-music-style
/// pulse meters, or any mechanic where mounting rhythmic energy
/// carries a crowd to euphoric peak and then fades back to silence.
///
/// `pulse(amount)` adds rhythm; fires `just_euphoric` when first reaching
/// `max_rhythm`. No-op when disabled.
///
/// `quiet(amount)` reduces rhythm immediately; fires `just_silenced`
/// when reaching 0. No-op when disabled or already silenced.
///
/// `tick(dt)` clears both flags, then increases rhythm by
/// `pulse_rate * dt` (capped at `max_rhythm`). Fires `just_euphoric`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_euphoric()` returns `rhythm >= max_rhythm && enabled`.
///
/// `is_silenced()` returns `rhythm == 0.0` (not gated by `enabled`).
///
/// `rhythm_fraction()` returns `(rhythm / max_rhythm).clamp(0, 1)`.
///
/// `effective_groove(scale)` returns `scale * rhythm_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — pulses at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zouk {
    pub rhythm: f32,
    pub max_rhythm: f32,
    pub pulse_rate: f32,
    pub just_euphoric: bool,
    pub just_silenced: bool,
    pub enabled: bool,
}

impl Zouk {
    pub fn new(max_rhythm: f32, pulse_rate: f32) -> Self {
        Self {
            rhythm: 0.0,
            max_rhythm: max_rhythm.max(0.1),
            pulse_rate: pulse_rate.max(0.0),
            just_euphoric: false,
            just_silenced: false,
            enabled: true,
        }
    }

    /// Add rhythm; fires `just_euphoric` when first reaching max.
    /// No-op when disabled.
    pub fn pulse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.rhythm < self.max_rhythm;
        self.rhythm = (self.rhythm + amount).min(self.max_rhythm);
        if was_below && self.rhythm >= self.max_rhythm {
            self.just_euphoric = true;
        }
    }

    /// Reduce rhythm; fires `just_silenced` when reaching 0.
    /// No-op when disabled or already silenced.
    pub fn quiet(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.rhythm <= 0.0 {
            return;
        }
        self.rhythm = (self.rhythm - amount).max(0.0);
        if self.rhythm <= 0.0 {
            self.just_silenced = true;
        }
    }

    /// Clear flags, then increase rhythm by `pulse_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_euphoric = false;
        self.just_silenced = false;
        if self.enabled && self.pulse_rate > 0.0 && self.rhythm < self.max_rhythm {
            let was_below = self.rhythm < self.max_rhythm;
            self.rhythm = (self.rhythm + self.pulse_rate * dt).min(self.max_rhythm);
            if was_below && self.rhythm >= self.max_rhythm {
                self.just_euphoric = true;
            }
        }
    }

    /// `true` when rhythm is at maximum and component is enabled.
    pub fn is_euphoric(&self) -> bool {
        self.rhythm >= self.max_rhythm && self.enabled
    }

    /// `true` when rhythm is 0 (not gated by `enabled`).
    pub fn is_silenced(&self) -> bool {
        self.rhythm == 0.0
    }

    /// Fraction of maximum rhythm [0.0, 1.0].
    pub fn rhythm_fraction(&self) -> f32 {
        (self.rhythm / self.max_rhythm).clamp(0.0, 1.0)
    }

    /// Returns `scale * rhythm_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_groove(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.rhythm_fraction()
    }
}

impl Default for Zouk {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zouk {
        Zouk::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_silenced() {
        let z = z();
        assert_eq!(z.rhythm, 0.0);
        assert!(z.is_silenced());
        assert!(!z.is_euphoric());
    }

    #[test]
    fn new_clamps_max_rhythm() {
        let z = Zouk::new(-5.0, 4.0);
        assert!((z.max_rhythm - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_pulse_rate() {
        let z = Zouk::new(100.0, -3.0);
        assert_eq!(z.pulse_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zouk::default();
        assert!((z.max_rhythm - 100.0).abs() < 1e-5);
        assert!((z.pulse_rate - 4.0).abs() < 1e-5);
    }

    // --- pulse ---

    #[test]
    fn pulse_adds_rhythm() {
        let mut z = z();
        z.pulse(40.0);
        assert!((z.rhythm - 40.0).abs() < 1e-3);
    }

    #[test]
    fn pulse_clamps_at_max() {
        let mut z = z();
        z.pulse(200.0);
        assert!((z.rhythm - 100.0).abs() < 1e-3);
    }

    #[test]
    fn pulse_fires_just_euphoric_at_max() {
        let mut z = z();
        z.pulse(100.0);
        assert!(z.just_euphoric);
        assert!(z.is_euphoric());
    }

    #[test]
    fn pulse_no_just_euphoric_when_already_at_max() {
        let mut z = z();
        z.rhythm = 100.0;
        z.pulse(10.0);
        assert!(!z.just_euphoric);
    }

    #[test]
    fn pulse_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.pulse(50.0);
        assert_eq!(z.rhythm, 0.0);
    }

    #[test]
    fn pulse_no_op_when_amount_zero() {
        let mut z = z();
        z.pulse(0.0);
        assert_eq!(z.rhythm, 0.0);
    }

    // --- quiet ---

    #[test]
    fn quiet_reduces_rhythm() {
        let mut z = z();
        z.rhythm = 60.0;
        z.quiet(20.0);
        assert!((z.rhythm - 40.0).abs() < 1e-3);
    }

    #[test]
    fn quiet_clamps_at_zero() {
        let mut z = z();
        z.rhythm = 30.0;
        z.quiet(200.0);
        assert_eq!(z.rhythm, 0.0);
    }

    #[test]
    fn quiet_fires_just_silenced_at_zero() {
        let mut z = z();
        z.rhythm = 30.0;
        z.quiet(30.0);
        assert!(z.just_silenced);
    }

    #[test]
    fn quiet_no_op_when_already_silenced() {
        let mut z = z();
        z.quiet(10.0);
        assert!(!z.just_silenced);
    }

    #[test]
    fn quiet_no_op_when_disabled() {
        let mut z = z();
        z.rhythm = 50.0;
        z.enabled = false;
        z.quiet(50.0);
        assert!((z.rhythm - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_pulses_rhythm() {
        let mut z = z(); // rate=4
        z.tick(1.0); // 0 + 4 = 4
        assert!((z.rhythm - 4.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_euphoric_on_pulse_to_max() {
        let mut z = Zouk::new(100.0, 200.0);
        z.rhythm = 95.0;
        z.tick(1.0);
        assert!(z.just_euphoric);
        assert!(z.is_euphoric());
    }

    #[test]
    fn tick_no_pulse_when_already_euphoric() {
        let mut z = z();
        z.rhythm = 100.0;
        z.tick(1.0);
        assert!(!z.just_euphoric);
    }

    #[test]
    fn tick_no_pulse_when_rate_zero() {
        let mut z = Zouk::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.rhythm, 0.0);
    }

    #[test]
    fn tick_no_pulse_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.rhythm, 0.0);
    }

    #[test]
    fn tick_clears_just_euphoric() {
        let mut z = Zouk::new(100.0, 200.0);
        z.rhythm = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_euphoric);
    }

    #[test]
    fn tick_clears_just_silenced() {
        let mut z = z();
        z.rhythm = 10.0;
        z.quiet(10.0);
        z.tick(0.016);
        assert!(!z.just_silenced);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=4
        z.tick(5.0); // 4*5 = 20
        assert!((z.rhythm - 20.0).abs() < 1e-3);
    }

    // --- is_euphoric / is_silenced ---

    #[test]
    fn is_euphoric_false_when_disabled() {
        let mut z = z();
        z.rhythm = 100.0;
        z.enabled = false;
        assert!(!z.is_euphoric());
    }

    #[test]
    fn is_silenced_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_silenced());
    }

    // --- rhythm_fraction / effective_groove ---

    #[test]
    fn rhythm_fraction_zero_when_silenced() {
        assert_eq!(z().rhythm_fraction(), 0.0);
    }

    #[test]
    fn rhythm_fraction_half_at_midpoint() {
        let mut z = z();
        z.rhythm = 50.0;
        assert!((z.rhythm_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_groove_zero_when_silenced() {
        assert_eq!(z().effective_groove(100.0), 0.0);
    }

    #[test]
    fn effective_groove_scales_with_rhythm() {
        let mut z = z();
        z.rhythm = 75.0;
        assert!((z.effective_groove(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_groove_zero_when_disabled() {
        let mut z = z();
        z.rhythm = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_groove(100.0), 0.0);
    }
}
