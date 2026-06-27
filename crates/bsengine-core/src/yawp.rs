use bevy_ecs::prelude::Component;

/// Noise-burst/vocalization tracker. `volume` builds with `cry(amount)` and
/// decays passively at `decay_rate` per second in `tick(dt)`.
///
/// Models NPC alert cries, alarm-noise meters, crowd-noise buildup, or any
/// system where a loud instantaneous event raises a level that then fades.
///
/// `cry(amount)` immediately raises `volume`; fires `just_shrieked` when
/// reaching `max_volume`. No-op when disabled.
///
/// `hush(amount)` immediately lowers `volume`; fires `just_silenced` when
/// reaching 0. No-op when disabled.
///
/// `tick(dt)` clears `just_shrieked` and `just_silenced`, then decays
/// `volume` by `decay_rate * dt` (floored at 0). Fires `just_silenced`
/// when reaching 0 via decay.
///
/// `is_shrieking()` returns `volume >= max_volume && enabled`.
///
/// `is_silent()` returns `volume == 0.0` (not gated by `enabled`).
///
/// `volume_fraction()` returns `(volume / max_volume).clamp(0, 1)`.
///
/// `effective_noise(base)` returns `base * volume_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(100.0, 15.0)` — decays at 15 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yawp {
    pub volume: f32,
    pub max_volume: f32,
    pub decay_rate: f32,
    pub just_shrieked: bool,
    pub just_silenced: bool,
    pub enabled: bool,
}

impl Yawp {
    pub fn new(max_volume: f32, decay_rate: f32) -> Self {
        Self {
            volume: 0.0,
            max_volume: max_volume.max(0.1),
            decay_rate: decay_rate.max(0.0),
            just_shrieked: false,
            just_silenced: false,
            enabled: true,
        }
    }

    /// Raise volume by `amount`; fires `just_shrieked` when reaching max.
    /// No-op when disabled.
    pub fn cry(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.volume < self.max_volume;
        self.volume = (self.volume + amount).min(self.max_volume);
        if was_below && self.volume >= self.max_volume {
            self.just_shrieked = true;
        }
    }

    /// Lower volume by `amount`; fires `just_silenced` when reaching 0.
    /// No-op when disabled.
    pub fn hush(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.volume <= 0.0 {
            return;
        }
        self.volume = (self.volume - amount).max(0.0);
        if self.volume <= 0.0 {
            self.just_silenced = true;
        }
    }

    /// Clear flags, then decay volume by `decay_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_shrieked = false;
        self.just_silenced = false;
        if self.enabled && self.decay_rate > 0.0 && self.volume > 0.0 {
            self.volume = (self.volume - self.decay_rate * dt).max(0.0);
            if self.volume <= 0.0 {
                self.just_silenced = true;
            }
        }
    }

    /// `true` when volume is at maximum and component is enabled.
    pub fn is_shrieking(&self) -> bool {
        self.volume >= self.max_volume && self.enabled
    }

    /// `true` when volume is 0 (not gated by `enabled`).
    pub fn is_silent(&self) -> bool {
        self.volume == 0.0
    }

    /// Fraction of maximum volume [0.0, 1.0].
    pub fn volume_fraction(&self) -> f32 {
        (self.volume / self.max_volume).clamp(0.0, 1.0)
    }

    /// Returns `base * volume_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_noise(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.volume_fraction()
    }
}

impl Default for Yawp {
    fn default() -> Self {
        Self::new(100.0, 15.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yawp {
        Yawp::new(100.0, 15.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_silent() {
        let y = y();
        assert_eq!(y.volume, 0.0);
        assert!(y.is_silent());
        assert!(!y.is_shrieking());
    }

    #[test]
    fn new_clamps_max_volume() {
        let y = Yawp::new(-5.0, 10.0);
        assert!((y.max_volume - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_decay_rate() {
        let y = Yawp::new(100.0, -3.0);
        assert_eq!(y.decay_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yawp::default();
        assert!((y.max_volume - 100.0).abs() < 1e-5);
        assert!((y.decay_rate - 15.0).abs() < 1e-5);
    }

    // --- cry ---

    #[test]
    fn cry_raises_volume() {
        let mut y = y();
        y.cry(40.0);
        assert!((y.volume - 40.0).abs() < 1e-3);
    }

    #[test]
    fn cry_clamps_at_max() {
        let mut y = y();
        y.cry(200.0);
        assert!((y.volume - 100.0).abs() < 1e-3);
    }

    #[test]
    fn cry_fires_just_shrieked_at_max() {
        let mut y = y();
        y.cry(100.0);
        assert!(y.just_shrieked);
        assert!(y.is_shrieking());
    }

    #[test]
    fn cry_no_just_shrieked_when_already_at_max() {
        let mut y = y();
        y.volume = 100.0;
        y.cry(10.0);
        assert!(!y.just_shrieked);
    }

    #[test]
    fn cry_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.cry(50.0);
        assert_eq!(y.volume, 0.0);
    }

    #[test]
    fn cry_no_op_when_amount_zero() {
        let mut y = y();
        y.cry(0.0);
        assert_eq!(y.volume, 0.0);
    }

    // --- hush ---

    #[test]
    fn hush_reduces_volume() {
        let mut y = y();
        y.volume = 60.0;
        y.hush(20.0);
        assert!((y.volume - 40.0).abs() < 1e-3);
    }

    #[test]
    fn hush_clamps_at_zero() {
        let mut y = y();
        y.volume = 30.0;
        y.hush(200.0);
        assert_eq!(y.volume, 0.0);
    }

    #[test]
    fn hush_fires_just_silenced_at_zero() {
        let mut y = y();
        y.volume = 30.0;
        y.hush(30.0);
        assert!(y.just_silenced);
    }

    #[test]
    fn hush_no_op_when_already_silent() {
        let mut y = y();
        y.hush(10.0);
        assert!(!y.just_silenced);
    }

    #[test]
    fn hush_no_op_when_disabled() {
        let mut y = y();
        y.volume = 50.0;
        y.enabled = false;
        y.hush(50.0);
        assert!((y.volume - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_decays_volume() {
        let mut y = y(); // decay=15
        y.volume = 60.0;
        y.tick(1.0); // 60 - 15 = 45
        assert!((y.volume - 45.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_at_zero() {
        let mut y = Yawp::new(100.0, 200.0);
        y.volume = 10.0;
        y.tick(1.0);
        assert_eq!(y.volume, 0.0);
    }

    #[test]
    fn tick_fires_just_silenced_on_decay_to_zero() {
        let mut y = Yawp::new(100.0, 200.0);
        y.volume = 10.0;
        y.tick(1.0);
        assert!(y.just_silenced);
    }

    #[test]
    fn tick_no_decay_when_already_silent() {
        let mut y = y();
        y.tick(10.0);
        assert!(!y.just_silenced);
    }

    #[test]
    fn tick_no_decay_when_rate_zero() {
        let mut y = Yawp::new(100.0, 0.0);
        y.volume = 50.0;
        y.tick(100.0);
        assert!((y.volume - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_decay_when_disabled() {
        let mut y = y();
        y.volume = 50.0;
        y.enabled = false;
        y.tick(1.0);
        assert!((y.volume - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_shrieked() {
        let mut y = y();
        y.cry(100.0); // just_shrieked fires
        y.tick(0.016);
        assert!(!y.just_shrieked);
    }

    #[test]
    fn tick_clears_just_silenced() {
        let mut y = Yawp::new(100.0, 200.0);
        y.volume = 5.0;
        y.tick(1.0); // just_silenced fires
        y.tick(0.016);
        assert!(!y.just_silenced);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = y(); // decay=15
        y.volume = 100.0;
        y.tick(2.0); // 100 - 15*2 = 70
        assert!((y.volume - 70.0).abs() < 1e-3);
    }

    // --- is_shrieking / is_silent ---

    #[test]
    fn is_shrieking_false_when_disabled() {
        let mut y = y();
        y.volume = 100.0;
        y.enabled = false;
        assert!(!y.is_shrieking());
    }

    #[test]
    fn is_silent_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_silent());
    }

    // --- fraction / effective ---

    #[test]
    fn volume_fraction_zero_when_silent() {
        assert_eq!(y().volume_fraction(), 0.0);
    }

    #[test]
    fn volume_fraction_half_at_midpoint() {
        let mut y = y();
        y.volume = 50.0;
        assert!((y.volume_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_noise_zero_when_silent() {
        assert_eq!(y().effective_noise(100.0), 0.0);
    }

    #[test]
    fn effective_noise_scales_with_volume() {
        let mut y = y();
        y.volume = 80.0;
        assert!((y.effective_noise(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_noise_zero_when_disabled() {
        let mut y = y();
        y.volume = 50.0;
        y.enabled = false;
        assert_eq!(y.effective_noise(100.0), 0.0);
    }
}
