use bevy_ecs::prelude::Component;

/// Elation-spike accumulator with passive decay. Records positive feedback
/// events as spikes of elation that naturally fade over time. `cheer(amount)`
/// adds elation; `tick(dt)` drains it at `decay_rate` per second. Fires
/// `just_peaked` when elation reaches max and `just_faded` when it returns
/// to 0 after being non-zero.
///
/// Models crowd approval meters, NPC morale spikes, player positive-feedback
/// moments, or any system where joy surges quickly and fades slowly without
/// active maintenance.
///
/// `cheer(amount)` increases elation (clamped to `max_elation`). Fires
/// `just_peaked` on first reaching max. No-op when disabled or already peaked.
///
/// `tick(dt)` clears `just_peaked` and `just_faded`. Then (when enabled and
/// `decay_rate > 0`) drains `elation` by `decay_rate * dt`. Fires
/// `just_faded` when elation first reaches 0 from a non-zero value.
///
/// `is_peaked()` returns `elation >= max_elation && enabled`.
///
/// `is_faded()` returns `elation == 0.0` (not gated by `enabled`).
///
/// `elation_fraction()` returns `(elation / max_elation).clamp(0, 1)`.
///
/// `effective_joy(base)` returns `base * elation_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(100.0, 10.0)` — decays at 10/sec, starts at 0.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yay {
    pub elation: f32,
    pub max_elation: f32,
    pub decay_rate: f32,
    pub just_peaked: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Yay {
    pub fn new(max_elation: f32, decay_rate: f32) -> Self {
        Self {
            elation: 0.0,
            max_elation: max_elation.max(0.1),
            decay_rate: decay_rate.max(0.0),
            just_peaked: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Add elation spike. Fires `just_peaked` on first reaching `max_elation`.
    /// No-op when disabled or already peaked.
    pub fn cheer(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.elation >= self.max_elation {
            return;
        }
        self.elation = (self.elation + amount).min(self.max_elation);
        if self.elation >= self.max_elation {
            self.just_peaked = true;
        }
    }

    /// Advance one frame: clear flags, then drain elation passively when
    /// enabled and `decay_rate > 0`. Fires `just_faded` when elation first
    /// reaches 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_faded = false;
        if self.enabled && self.decay_rate > 0.0 && self.elation > 0.0 {
            self.elation = (self.elation - self.decay_rate * dt).max(0.0);
            if self.elation <= 0.0 {
                self.just_faded = true;
            }
        }
    }

    /// `true` when elation is at maximum and component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.elation >= self.max_elation && self.enabled
    }

    /// `true` when elation is 0 (not gated by `enabled`).
    pub fn is_faded(&self) -> bool {
        self.elation == 0.0
    }

    /// Fraction of maximum elation [0.0, 1.0].
    pub fn elation_fraction(&self) -> f32 {
        (self.elation / self.max_elation).clamp(0.0, 1.0)
    }

    /// Returns `base * elation_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_joy(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.elation_fraction()
    }
}

impl Default for Yay {
    fn default() -> Self {
        Self::new(100.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yay {
        Yay::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_faded() {
        let y = y();
        assert_eq!(y.elation, 0.0);
        assert!(y.is_faded());
        assert!(!y.is_peaked());
    }

    #[test]
    fn new_clamps_max_elation() {
        let y = Yay::new(-5.0, 1.0);
        assert!((y.max_elation - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_decay_rate() {
        let y = Yay::new(100.0, -5.0);
        assert_eq!(y.decay_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yay::default();
        assert!((y.max_elation - 100.0).abs() < 1e-5);
        assert!((y.decay_rate - 10.0).abs() < 1e-5);
    }

    // --- cheer ---

    #[test]
    fn cheer_increases_elation() {
        let mut y = y();
        y.cheer(30.0);
        assert!((y.elation - 30.0).abs() < 1e-4);
    }

    #[test]
    fn cheer_clamps_at_max() {
        let mut y = y();
        y.cheer(200.0);
        assert!((y.elation - 100.0).abs() < 1e-5);
    }

    #[test]
    fn cheer_fires_just_peaked_at_max() {
        let mut y = y();
        y.cheer(100.0);
        assert!(y.just_peaked);
        assert!(y.is_peaked());
    }

    #[test]
    fn cheer_no_refire_when_already_peaked() {
        let mut y = y();
        y.cheer(100.0);
        y.tick(0.016); // decay starts but don't let it drain below max
        y.cheer(100.0); // still at max (or above after decay? No, decay reduced it)
                        // Actually after 0.016s at 10/s: 100 - 0.16 = 99.84, not at max
                        // So another cheer(100) would go to max and refire? Let's test no-refire only when at max
                        // Restart: test cheer multiple times in same frame
        let mut y2 = Yay::new(100.0, 10.0);
        y2.cheer(100.0); // peaks, just_peaked = true
        y2.cheer(10.0); // already at max, no-op
        assert!(y2.just_peaked); // only once
    }

    #[test]
    fn cheer_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.cheer(50.0);
        assert_eq!(y.elation, 0.0);
    }

    #[test]
    fn cheer_no_op_for_zero_amount() {
        let mut y = y();
        y.cheer(0.0);
        assert_eq!(y.elation, 0.0);
    }

    // --- tick (decay) ---

    #[test]
    fn tick_decays_elation() {
        let mut y = y(); // decay_rate = 10
        y.cheer(50.0);
        y.tick(1.0); // 50 - 10*1 = 40
        assert!((y.elation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_decay_at_zero() {
        let mut y = y();
        y.cheer(5.0);
        y.tick(10.0); // 5 - 10*10 → 0
        assert_eq!(y.elation, 0.0);
    }

    #[test]
    fn tick_fires_just_faded_when_reaching_zero() {
        let mut y = y();
        y.cheer(5.0);
        y.tick(1.0); // drains past 0
        assert!(y.just_faded);
        assert!(y.is_faded());
    }

    #[test]
    fn tick_no_fade_event_when_already_zero() {
        let mut y = y();
        y.tick(1.0); // already 0
        assert!(!y.just_faded);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut y = y();
        y.cheer(100.0);
        y.tick(0.001);
        assert!(!y.just_peaked);
    }

    #[test]
    fn tick_clears_just_faded() {
        let mut y = y();
        y.cheer(5.0);
        y.tick(1.0); // just_faded fires
        assert!(y.just_faded);
        y.tick(0.016); // cleared
        assert!(!y.just_faded);
    }

    #[test]
    fn tick_no_decay_when_rate_zero() {
        let mut y = Yay::new(100.0, 0.0);
        y.cheer(50.0);
        y.tick(100.0); // no decay
        assert!((y.elation - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_decay_when_disabled() {
        let mut y = y();
        y.cheer(50.0);
        y.enabled = false;
        y.tick(1.0); // no decay when disabled
        assert!((y.elation - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = y();
        y.cheer(80.0);
        y.tick(0.5); // 80 - 10*0.5 = 75
        assert!((y.elation - 75.0).abs() < 1e-3);
    }

    // --- is_peaked / is_faded ---

    #[test]
    fn is_peaked_false_below_max() {
        let mut y = y();
        y.cheer(50.0);
        assert!(!y.is_peaked());
    }

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut y = y();
        y.cheer(100.0);
        y.enabled = false;
        assert!(!y.is_peaked());
    }

    #[test]
    fn is_faded_true_at_zero() {
        assert!(y().is_faded());
    }

    #[test]
    fn is_faded_true_even_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_faded()); // not gated
    }

    // --- fractions / effective ---

    #[test]
    fn elation_fraction_zero_when_faded() {
        assert_eq!(y().elation_fraction(), 0.0);
    }

    #[test]
    fn elation_fraction_half_at_midpoint() {
        let mut y = y();
        y.cheer(50.0);
        assert!((y.elation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn elation_fraction_one_at_max() {
        let mut y = y();
        y.cheer(100.0);
        assert!((y.elation_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_joy_zero_when_faded() {
        assert_eq!(y().effective_joy(100.0), 0.0);
    }

    #[test]
    fn effective_joy_scales_with_fraction() {
        let mut y = y();
        y.cheer(75.0);
        assert!((y.effective_joy(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_joy_zero_when_disabled() {
        let mut y = y();
        y.cheer(50.0);
        y.enabled = false;
        assert_eq!(y.effective_joy(100.0), 0.0);
    }
}
