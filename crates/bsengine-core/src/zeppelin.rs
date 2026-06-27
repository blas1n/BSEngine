use bevy_ecs::prelude::Component;

/// Buoyant-lift tracker. `lift` builds via `inflate(amount)` and leaks
/// passively at `leak_rate` per second in `tick(dt)` or can be vented
/// immediately via `vent(amount)`.
///
/// Models gas-bag lift pressure, slow-moving aerial momentum, floating
/// fortress capacity, balloon-powered ascent gauges, buoyancy-engine
/// reserves, or any mechanic where a sustained "lighter-than-air" resource
/// inflates slowly, holds at peak, and gradually escapes when not replenished.
///
/// `inflate(amount)` adds lift; fires `just_aloft` when first reaching
/// `max_lift`. No-op when disabled.
///
/// `vent(amount)` reduces lift immediately; fires `just_grounded` when
/// reaching 0. No-op when disabled or already grounded.
///
/// `tick(dt)` clears both flags, then leaks lift by `leak_rate * dt`
/// (floored at 0). Fires `just_grounded` when reaching 0 via leak.
/// No-op when disabled or rate is 0.
///
/// `is_aloft()` returns `lift >= max_lift && enabled`.
///
/// `is_grounded()` returns `lift == 0.0` (not gated by `enabled`).
///
/// `lift_fraction()` returns `(lift / max_lift).clamp(0, 1)`.
///
/// `effective_altitude(scale)` returns `scale * lift_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — leaks at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeppelin {
    pub lift: f32,
    pub max_lift: f32,
    pub leak_rate: f32,
    pub just_aloft: bool,
    pub just_grounded: bool,
    pub enabled: bool,
}

impl Zeppelin {
    pub fn new(max_lift: f32, leak_rate: f32) -> Self {
        Self {
            lift: 0.0,
            max_lift: max_lift.max(0.1),
            leak_rate: leak_rate.max(0.0),
            just_aloft: false,
            just_grounded: false,
            enabled: true,
        }
    }

    /// Add lift; fires `just_aloft` when first reaching max.
    /// No-op when disabled.
    pub fn inflate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.lift < self.max_lift;
        self.lift = (self.lift + amount).min(self.max_lift);
        if was_below && self.lift >= self.max_lift {
            self.just_aloft = true;
        }
    }

    /// Reduce lift; fires `just_grounded` when reaching 0.
    /// No-op when disabled or already grounded.
    pub fn vent(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.lift <= 0.0 {
            return;
        }
        self.lift = (self.lift - amount).max(0.0);
        if self.lift <= 0.0 {
            self.just_grounded = true;
        }
    }

    /// Clear flags, then leak lift by `leak_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_aloft = false;
        self.just_grounded = false;
        if self.enabled && self.leak_rate > 0.0 && self.lift > 0.0 {
            self.lift = (self.lift - self.leak_rate * dt).max(0.0);
            if self.lift <= 0.0 {
                self.just_grounded = true;
            }
        }
    }

    /// `true` when lift is at maximum and component is enabled.
    pub fn is_aloft(&self) -> bool {
        self.lift >= self.max_lift && self.enabled
    }

    /// `true` when lift is 0 (not gated by `enabled`).
    pub fn is_grounded(&self) -> bool {
        self.lift == 0.0
    }

    /// Fraction of maximum lift [0.0, 1.0].
    pub fn lift_fraction(&self) -> f32 {
        (self.lift / self.max_lift).clamp(0.0, 1.0)
    }

    /// Returns `scale * lift_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_altitude(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.lift_fraction()
    }
}

impl Default for Zeppelin {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeppelin {
        Zeppelin::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_grounded() {
        let z = z();
        assert_eq!(z.lift, 0.0);
        assert!(z.is_grounded());
        assert!(!z.is_aloft());
    }

    #[test]
    fn new_clamps_max_lift() {
        let z = Zeppelin::new(-5.0, 4.0);
        assert!((z.max_lift - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_leak_rate() {
        let z = Zeppelin::new(100.0, -3.0);
        assert_eq!(z.leak_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeppelin::default();
        assert!((z.max_lift - 100.0).abs() < 1e-5);
        assert!((z.leak_rate - 4.0).abs() < 1e-5);
    }

    // --- inflate ---

    #[test]
    fn inflate_adds_lift() {
        let mut z = z();
        z.inflate(40.0);
        assert!((z.lift - 40.0).abs() < 1e-3);
    }

    #[test]
    fn inflate_clamps_at_max() {
        let mut z = z();
        z.inflate(200.0);
        assert!((z.lift - 100.0).abs() < 1e-3);
    }

    #[test]
    fn inflate_fires_just_aloft_at_max() {
        let mut z = z();
        z.inflate(100.0);
        assert!(z.just_aloft);
        assert!(z.is_aloft());
    }

    #[test]
    fn inflate_no_just_aloft_when_already_at_max() {
        let mut z = z();
        z.lift = 100.0;
        z.inflate(10.0);
        assert!(!z.just_aloft);
    }

    #[test]
    fn inflate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.inflate(50.0);
        assert_eq!(z.lift, 0.0);
    }

    #[test]
    fn inflate_no_op_when_amount_zero() {
        let mut z = z();
        z.inflate(0.0);
        assert_eq!(z.lift, 0.0);
    }

    // --- vent ---

    #[test]
    fn vent_reduces_lift() {
        let mut z = z();
        z.lift = 60.0;
        z.vent(20.0);
        assert!((z.lift - 40.0).abs() < 1e-3);
    }

    #[test]
    fn vent_clamps_at_zero() {
        let mut z = z();
        z.lift = 30.0;
        z.vent(200.0);
        assert_eq!(z.lift, 0.0);
    }

    #[test]
    fn vent_fires_just_grounded_at_zero() {
        let mut z = z();
        z.lift = 30.0;
        z.vent(30.0);
        assert!(z.just_grounded);
    }

    #[test]
    fn vent_no_op_when_already_grounded() {
        let mut z = z();
        z.vent(10.0);
        assert!(!z.just_grounded);
    }

    #[test]
    fn vent_no_op_when_disabled() {
        let mut z = z();
        z.lift = 50.0;
        z.enabled = false;
        z.vent(50.0);
        assert!((z.lift - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_leaks_lift() {
        let mut z = z(); // leak=4
        z.lift = 60.0;
        z.tick(1.0); // 60 - 4 = 56
        assert!((z.lift - 56.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_grounded_on_leak_to_zero() {
        let mut z = Zeppelin::new(100.0, 200.0);
        z.lift = 5.0;
        z.tick(1.0);
        assert!(z.just_grounded);
        assert!(z.is_grounded());
    }

    #[test]
    fn tick_no_leak_when_already_grounded() {
        let mut z = z();
        z.tick(10.0);
        assert!(!z.just_grounded);
    }

    #[test]
    fn tick_no_leak_when_rate_zero() {
        let mut z = Zeppelin::new(100.0, 0.0);
        z.lift = 50.0;
        z.tick(100.0);
        assert!((z.lift - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_leak_when_disabled() {
        let mut z = z();
        z.lift = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.lift - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_aloft() {
        let mut z = z();
        z.inflate(100.0);
        z.tick(0.016);
        assert!(!z.just_aloft);
    }

    #[test]
    fn tick_clears_just_grounded() {
        let mut z = Zeppelin::new(100.0, 200.0);
        z.lift = 5.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_grounded);
    }

    #[test]
    fn tick_scales_leak_with_dt() {
        let mut z = z(); // leak=4
        z.lift = 100.0;
        z.tick(5.0); // 100 - 4*5 = 80
        assert!((z.lift - 80.0).abs() < 1e-3);
    }

    // --- is_aloft / is_grounded ---

    #[test]
    fn is_aloft_false_when_disabled() {
        let mut z = z();
        z.lift = 100.0;
        z.enabled = false;
        assert!(!z.is_aloft());
    }

    #[test]
    fn is_grounded_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_grounded());
    }

    // --- lift_fraction / effective_altitude ---

    #[test]
    fn lift_fraction_zero_when_grounded() {
        assert_eq!(z().lift_fraction(), 0.0);
    }

    #[test]
    fn lift_fraction_half_at_midpoint() {
        let mut z = z();
        z.lift = 50.0;
        assert!((z.lift_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_altitude_zero_when_grounded() {
        assert_eq!(z().effective_altitude(100.0), 0.0);
    }

    #[test]
    fn effective_altitude_scales_with_lift() {
        let mut z = z();
        z.lift = 80.0;
        assert!((z.effective_altitude(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_altitude_zero_when_disabled() {
        let mut z = z();
        z.lift = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_altitude(100.0), 0.0);
    }
}
