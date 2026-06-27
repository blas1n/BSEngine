use bevy_ecs::prelude::Component;

/// Percussion-ring tracker. `ring` builds via `strike(amount)` and
/// sustains passively at `sustain_rate` per second in `tick(dt)` or
/// is damped immediately via `damp(amount)`.
///
/// Models finger-cymbal resonance meters, percussion-charge bars,
/// bell-tone sustain gauges, crystal-singing-bowl fill levels,
/// rhythm-game combo multipliers, tuning-fork vibration trackers,
/// or any mechanic where a struck instrument sustains its resonance
/// and must be actively damped to silence.
///
/// `strike(amount)` adds ring; fires `just_ringing` when first
/// reaching `max_ring`. No-op when disabled.
///
/// `damp(amount)` reduces ring immediately; fires `just_damped`
/// when reaching 0. No-op when disabled or already damped.
///
/// `tick(dt)` clears both flags, then increases ring by
/// `sustain_rate * dt` (capped at `max_ring`). Fires `just_ringing`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_ringing()` returns `ring >= max_ring && enabled`.
///
/// `is_damped()` returns `ring == 0.0` (not gated by `enabled`).
///
/// `ring_fraction()` returns `(ring / max_ring).clamp(0, 1)`.
///
/// `effective_tone(scale)` returns `scale * ring_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 8.0)` — sustains at 8 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zill {
    pub ring: f32,
    pub max_ring: f32,
    pub sustain_rate: f32,
    pub just_ringing: bool,
    pub just_damped: bool,
    pub enabled: bool,
}

impl Zill {
    pub fn new(max_ring: f32, sustain_rate: f32) -> Self {
        Self {
            ring: 0.0,
            max_ring: max_ring.max(0.1),
            sustain_rate: sustain_rate.max(0.0),
            just_ringing: false,
            just_damped: false,
            enabled: true,
        }
    }

    /// Add ring; fires `just_ringing` when first reaching max.
    /// No-op when disabled.
    pub fn strike(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.ring < self.max_ring;
        self.ring = (self.ring + amount).min(self.max_ring);
        if was_below && self.ring >= self.max_ring {
            self.just_ringing = true;
        }
    }

    /// Reduce ring; fires `just_damped` when reaching 0.
    /// No-op when disabled or already damped.
    pub fn damp(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.ring <= 0.0 {
            return;
        }
        self.ring = (self.ring - amount).max(0.0);
        if self.ring <= 0.0 {
            self.just_damped = true;
        }
    }

    /// Clear flags, then increase ring by `sustain_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_ringing = false;
        self.just_damped = false;
        if self.enabled && self.sustain_rate > 0.0 && self.ring < self.max_ring {
            let was_below = self.ring < self.max_ring;
            self.ring = (self.ring + self.sustain_rate * dt).min(self.max_ring);
            if was_below && self.ring >= self.max_ring {
                self.just_ringing = true;
            }
        }
    }

    /// `true` when ring is at maximum and component is enabled.
    pub fn is_ringing(&self) -> bool {
        self.ring >= self.max_ring && self.enabled
    }

    /// `true` when ring is 0 (not gated by `enabled`).
    pub fn is_damped(&self) -> bool {
        self.ring == 0.0
    }

    /// Fraction of maximum ring [0.0, 1.0].
    pub fn ring_fraction(&self) -> f32 {
        (self.ring / self.max_ring).clamp(0.0, 1.0)
    }

    /// Returns `scale * ring_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_tone(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.ring_fraction()
    }
}

impl Default for Zill {
    fn default() -> Self {
        Self::new(100.0, 8.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zill {
        Zill::new(100.0, 8.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_damped() {
        let z = z();
        assert_eq!(z.ring, 0.0);
        assert!(z.is_damped());
        assert!(!z.is_ringing());
    }

    #[test]
    fn new_clamps_max_ring() {
        let z = Zill::new(-5.0, 8.0);
        assert!((z.max_ring - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_sustain_rate() {
        let z = Zill::new(100.0, -3.0);
        assert_eq!(z.sustain_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zill::default();
        assert!((z.max_ring - 100.0).abs() < 1e-5);
        assert!((z.sustain_rate - 8.0).abs() < 1e-5);
    }

    // --- strike ---

    #[test]
    fn strike_adds_ring() {
        let mut z = z();
        z.strike(40.0);
        assert!((z.ring - 40.0).abs() < 1e-3);
    }

    #[test]
    fn strike_clamps_at_max() {
        let mut z = z();
        z.strike(200.0);
        assert!((z.ring - 100.0).abs() < 1e-3);
    }

    #[test]
    fn strike_fires_just_ringing_at_max() {
        let mut z = z();
        z.strike(100.0);
        assert!(z.just_ringing);
        assert!(z.is_ringing());
    }

    #[test]
    fn strike_no_just_ringing_when_already_at_max() {
        let mut z = z();
        z.ring = 100.0;
        z.strike(10.0);
        assert!(!z.just_ringing);
    }

    #[test]
    fn strike_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.strike(50.0);
        assert_eq!(z.ring, 0.0);
    }

    #[test]
    fn strike_no_op_when_amount_zero() {
        let mut z = z();
        z.strike(0.0);
        assert_eq!(z.ring, 0.0);
    }

    // --- damp ---

    #[test]
    fn damp_reduces_ring() {
        let mut z = z();
        z.ring = 60.0;
        z.damp(20.0);
        assert!((z.ring - 40.0).abs() < 1e-3);
    }

    #[test]
    fn damp_clamps_at_zero() {
        let mut z = z();
        z.ring = 30.0;
        z.damp(200.0);
        assert_eq!(z.ring, 0.0);
    }

    #[test]
    fn damp_fires_just_damped_at_zero() {
        let mut z = z();
        z.ring = 30.0;
        z.damp(30.0);
        assert!(z.just_damped);
    }

    #[test]
    fn damp_no_op_when_already_damped() {
        let mut z = z();
        z.damp(10.0);
        assert!(!z.just_damped);
    }

    #[test]
    fn damp_no_op_when_disabled() {
        let mut z = z();
        z.ring = 50.0;
        z.enabled = false;
        z.damp(50.0);
        assert!((z.ring - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_sustains_ring() {
        let mut z = z(); // rate=8
        z.tick(1.0); // 0 + 8 = 8
        assert!((z.ring - 8.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_ringing_on_sustain_to_max() {
        let mut z = Zill::new(100.0, 200.0);
        z.ring = 95.0;
        z.tick(1.0);
        assert!(z.just_ringing);
        assert!(z.is_ringing());
    }

    #[test]
    fn tick_no_sustain_when_already_ringing() {
        let mut z = z();
        z.ring = 100.0;
        z.tick(1.0);
        assert!(!z.just_ringing);
    }

    #[test]
    fn tick_no_sustain_when_rate_zero() {
        let mut z = Zill::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.ring, 0.0);
    }

    #[test]
    fn tick_no_sustain_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.ring, 0.0);
    }

    #[test]
    fn tick_clears_just_ringing() {
        let mut z = Zill::new(100.0, 200.0);
        z.ring = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_ringing);
    }

    #[test]
    fn tick_clears_just_damped() {
        let mut z = z();
        z.ring = 10.0;
        z.damp(10.0);
        z.tick(0.016);
        assert!(!z.just_damped);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=8
        z.tick(3.0); // 8*3 = 24
        assert!((z.ring - 24.0).abs() < 1e-3);
    }

    // --- is_ringing / is_damped ---

    #[test]
    fn is_ringing_false_when_disabled() {
        let mut z = z();
        z.ring = 100.0;
        z.enabled = false;
        assert!(!z.is_ringing());
    }

    #[test]
    fn is_damped_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_damped());
    }

    // --- ring_fraction / effective_tone ---

    #[test]
    fn ring_fraction_zero_when_damped() {
        assert_eq!(z().ring_fraction(), 0.0);
    }

    #[test]
    fn ring_fraction_half_at_midpoint() {
        let mut z = z();
        z.ring = 50.0;
        assert!((z.ring_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_tone_zero_when_damped() {
        assert_eq!(z().effective_tone(100.0), 0.0);
    }

    #[test]
    fn effective_tone_scales_with_ring() {
        let mut z = z();
        z.ring = 65.0;
        assert!((z.effective_tone(100.0) - 65.0).abs() < 1e-3);
    }

    #[test]
    fn effective_tone_zero_when_disabled() {
        let mut z = z();
        z.ring = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_tone(100.0), 0.0);
    }
}
