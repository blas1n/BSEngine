use bevy_ecs::prelude::Component;

/// Rolling-momentum tracker. `spin` builds via `roll(amount)` and
/// accelerates passively at `spin_rate` per second in `tick(dt)` or
/// is braked immediately via `brake(amount)`.
///
/// Models inflatable-ball momentum gauges, rolling-boulder charge bars,
/// gyroscopic-spin fill levels, sphere-physics acceleration meters,
/// carnival-ride speed indicators, downhill-roll build-up trackers,
/// hamster-wheel endurance gauges, or any mechanic where rotational
/// momentum accumulates and must be actively braked to stop.
///
/// `roll(amount)` adds spin; fires `just_spinning` when first reaching
/// `max_spin`. No-op when disabled.
///
/// `brake(amount)` reduces spin immediately; fires `just_stopped` when
/// reaching 0. No-op when disabled or already stopped.
///
/// `tick(dt)` clears both flags, then increases spin by
/// `spin_rate * dt` (capped at `max_spin`). Fires `just_spinning`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_spinning()` returns `spin >= max_spin && enabled`.
///
/// `is_stopped()` returns `spin == 0.0` (not gated by `enabled`).
///
/// `spin_fraction()` returns `(spin / max_spin).clamp(0, 1)`.
///
/// `effective_momentum(scale)` returns `scale * spin_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 6.0)` — spins up at 6 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zorb {
    pub spin: f32,
    pub max_spin: f32,
    pub spin_rate: f32,
    pub just_spinning: bool,
    pub just_stopped: bool,
    pub enabled: bool,
}

impl Zorb {
    pub fn new(max_spin: f32, spin_rate: f32) -> Self {
        Self {
            spin: 0.0,
            max_spin: max_spin.max(0.1),
            spin_rate: spin_rate.max(0.0),
            just_spinning: false,
            just_stopped: false,
            enabled: true,
        }
    }

    /// Add spin; fires `just_spinning` when first reaching max.
    /// No-op when disabled.
    pub fn roll(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.spin < self.max_spin;
        self.spin = (self.spin + amount).min(self.max_spin);
        if was_below && self.spin >= self.max_spin {
            self.just_spinning = true;
        }
    }

    /// Reduce spin; fires `just_stopped` when reaching 0.
    /// No-op when disabled or already stopped.
    pub fn brake(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.spin <= 0.0 {
            return;
        }
        self.spin = (self.spin - amount).max(0.0);
        if self.spin <= 0.0 {
            self.just_stopped = true;
        }
    }

    /// Clear flags, then increase spin by `spin_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_spinning = false;
        self.just_stopped = false;
        if self.enabled && self.spin_rate > 0.0 && self.spin < self.max_spin {
            let was_below = self.spin < self.max_spin;
            self.spin = (self.spin + self.spin_rate * dt).min(self.max_spin);
            if was_below && self.spin >= self.max_spin {
                self.just_spinning = true;
            }
        }
    }

    /// `true` when spin is at maximum and component is enabled.
    pub fn is_spinning(&self) -> bool {
        self.spin >= self.max_spin && self.enabled
    }

    /// `true` when spin is 0 (not gated by `enabled`).
    pub fn is_stopped(&self) -> bool {
        self.spin == 0.0
    }

    /// Fraction of maximum spin [0.0, 1.0].
    pub fn spin_fraction(&self) -> f32 {
        (self.spin / self.max_spin).clamp(0.0, 1.0)
    }

    /// Returns `scale * spin_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_momentum(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.spin_fraction()
    }
}

impl Default for Zorb {
    fn default() -> Self {
        Self::new(100.0, 6.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zorb {
        Zorb::new(100.0, 6.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_stopped() {
        let z = z();
        assert_eq!(z.spin, 0.0);
        assert!(z.is_stopped());
        assert!(!z.is_spinning());
    }

    #[test]
    fn new_clamps_max_spin() {
        let z = Zorb::new(-5.0, 6.0);
        assert!((z.max_spin - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spin_rate() {
        let z = Zorb::new(100.0, -3.0);
        assert_eq!(z.spin_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zorb::default();
        assert!((z.max_spin - 100.0).abs() < 1e-5);
        assert!((z.spin_rate - 6.0).abs() < 1e-5);
    }

    // --- roll ---

    #[test]
    fn roll_adds_spin() {
        let mut z = z();
        z.roll(40.0);
        assert!((z.spin - 40.0).abs() < 1e-3);
    }

    #[test]
    fn roll_clamps_at_max() {
        let mut z = z();
        z.roll(200.0);
        assert!((z.spin - 100.0).abs() < 1e-3);
    }

    #[test]
    fn roll_fires_just_spinning_at_max() {
        let mut z = z();
        z.roll(100.0);
        assert!(z.just_spinning);
        assert!(z.is_spinning());
    }

    #[test]
    fn roll_no_just_spinning_when_already_at_max() {
        let mut z = z();
        z.spin = 100.0;
        z.roll(10.0);
        assert!(!z.just_spinning);
    }

    #[test]
    fn roll_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.roll(50.0);
        assert_eq!(z.spin, 0.0);
    }

    #[test]
    fn roll_no_op_when_amount_zero() {
        let mut z = z();
        z.roll(0.0);
        assert_eq!(z.spin, 0.0);
    }

    // --- brake ---

    #[test]
    fn brake_reduces_spin() {
        let mut z = z();
        z.spin = 60.0;
        z.brake(20.0);
        assert!((z.spin - 40.0).abs() < 1e-3);
    }

    #[test]
    fn brake_clamps_at_zero() {
        let mut z = z();
        z.spin = 30.0;
        z.brake(200.0);
        assert_eq!(z.spin, 0.0);
    }

    #[test]
    fn brake_fires_just_stopped_at_zero() {
        let mut z = z();
        z.spin = 30.0;
        z.brake(30.0);
        assert!(z.just_stopped);
    }

    #[test]
    fn brake_no_op_when_already_stopped() {
        let mut z = z();
        z.brake(10.0);
        assert!(!z.just_stopped);
    }

    #[test]
    fn brake_no_op_when_disabled() {
        let mut z = z();
        z.spin = 50.0;
        z.enabled = false;
        z.brake(50.0);
        assert!((z.spin - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_spins_up() {
        let mut z = z(); // rate=6
        z.tick(1.0); // 0 + 6 = 6
        assert!((z.spin - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_spinning_on_spin_to_max() {
        let mut z = Zorb::new(100.0, 200.0);
        z.spin = 95.0;
        z.tick(1.0);
        assert!(z.just_spinning);
        assert!(z.is_spinning());
    }

    #[test]
    fn tick_no_spin_when_already_spinning() {
        let mut z = z();
        z.spin = 100.0;
        z.tick(1.0);
        assert!(!z.just_spinning);
    }

    #[test]
    fn tick_no_spin_when_rate_zero() {
        let mut z = Zorb::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.spin, 0.0);
    }

    #[test]
    fn tick_no_spin_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.spin, 0.0);
    }

    #[test]
    fn tick_clears_just_spinning() {
        let mut z = Zorb::new(100.0, 200.0);
        z.spin = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_spinning);
    }

    #[test]
    fn tick_clears_just_stopped() {
        let mut z = z();
        z.spin = 10.0;
        z.brake(10.0);
        z.tick(0.016);
        assert!(!z.just_stopped);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=6
        z.tick(3.0); // 6*3 = 18
        assert!((z.spin - 18.0).abs() < 1e-3);
    }

    // --- is_spinning / is_stopped ---

    #[test]
    fn is_spinning_false_when_disabled() {
        let mut z = z();
        z.spin = 100.0;
        z.enabled = false;
        assert!(!z.is_spinning());
    }

    #[test]
    fn is_stopped_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_stopped());
    }

    // --- spin_fraction / effective_momentum ---

    #[test]
    fn spin_fraction_zero_when_stopped() {
        assert_eq!(z().spin_fraction(), 0.0);
    }

    #[test]
    fn spin_fraction_half_at_midpoint() {
        let mut z = z();
        z.spin = 50.0;
        assert!((z.spin_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_momentum_zero_when_stopped() {
        assert_eq!(z().effective_momentum(100.0), 0.0);
    }

    #[test]
    fn effective_momentum_scales_with_spin() {
        let mut z = z();
        z.spin = 75.0;
        assert!((z.effective_momentum(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_momentum_zero_when_disabled() {
        let mut z = z();
        z.spin = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_momentum(100.0), 0.0);
    }
}
