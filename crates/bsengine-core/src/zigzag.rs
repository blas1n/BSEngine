use bevy_ecs::prelude::Component;

/// Bounce-oscillation tracker. `phase` advances from 0 to `period` then
/// reverses back to 0, repeating indefinitely. `just_reversed` fires on
/// each reversal at either boundary.
///
/// Models back-and-forth patrol paths, ping-pong animation phases,
/// alternating hazard patterns, pendulum pressure, tide meters, or any
/// mechanic that needs a continuously reversing wave rather than a
/// one-directional accumulator.
///
/// `advance(amount)` moves `phase` in the current direction; bounces at 0
/// and `period`, setting `just_reversed` each time. No-op when disabled or
/// `amount <= 0`.
///
/// `tick(dt)` clears `just_reversed` then calls `advance(speed * dt)`.
/// No-op when disabled or `speed == 0`.
///
/// `is_rising()` returns `rising && enabled`.
///
/// `is_falling()` returns `!rising && enabled`.
///
/// `phase_fraction()` returns `phase / period` in [0.0, 1.0].
///
/// `effective_position(scale)` returns `scale * phase_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 20.0)` — oscillates at 20 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zigzag {
    pub phase: f32,
    pub period: f32,
    pub speed: f32,
    /// `true` = phase advancing toward `period`; `false` = retreating to 0.
    pub rising: bool,
    pub just_reversed: bool,
    pub enabled: bool,
}

impl Zigzag {
    pub fn new(period: f32, speed: f32) -> Self {
        Self {
            phase: 0.0,
            period: period.max(0.1),
            speed: speed.max(0.0),
            rising: true,
            just_reversed: false,
            enabled: true,
        }
    }

    /// Advance `phase` by `amount`, bouncing at 0 and `period`.
    /// No-op when disabled or `amount <= 0`.
    pub fn advance(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let mut remaining = amount;
        while remaining > f32::EPSILON {
            if self.rising {
                let room = self.period - self.phase;
                if remaining < room {
                    self.phase += remaining;
                    remaining = 0.0;
                } else {
                    remaining -= room;
                    self.phase = self.period;
                    self.rising = false;
                    self.just_reversed = true;
                }
            } else {
                let room = self.phase;
                if remaining < room {
                    self.phase -= remaining;
                    remaining = 0.0;
                } else {
                    remaining -= room;
                    self.phase = 0.0;
                    self.rising = true;
                    self.just_reversed = true;
                }
            }
        }
    }

    /// Clear `just_reversed`, then advance by `speed * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_reversed = false;
        if self.enabled && self.speed > 0.0 {
            self.advance(self.speed * dt);
        }
    }

    /// `true` when phase is advancing toward `period` and enabled.
    pub fn is_rising(&self) -> bool {
        self.rising && self.enabled
    }

    /// `true` when phase is retreating toward 0 and enabled.
    pub fn is_falling(&self) -> bool {
        !self.rising && self.enabled
    }

    /// Phase fraction [0.0, 1.0].
    pub fn phase_fraction(&self) -> f32 {
        (self.phase / self.period).clamp(0.0, 1.0)
    }

    /// Returns `scale * phase_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_position(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.phase_fraction()
    }
}

impl Default for Zigzag {
    fn default() -> Self {
        Self::new(100.0, 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zigzag {
        Zigzag::new(100.0, 20.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_at_zero_rising() {
        let z = z();
        assert_eq!(z.phase, 0.0);
        assert!(z.rising);
        assert!(!z.just_reversed);
    }

    #[test]
    fn new_clamps_period() {
        let z = Zigzag::new(-5.0, 20.0);
        assert!((z.period - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_speed() {
        let z = Zigzag::new(100.0, -3.0);
        assert_eq!(z.speed, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zigzag::default();
        assert!((z.period - 100.0).abs() < 1e-5);
        assert!((z.speed - 20.0).abs() < 1e-5);
    }

    // --- advance ---

    #[test]
    fn advance_moves_phase_forward() {
        let mut z = z();
        z.advance(30.0);
        assert!((z.phase - 30.0).abs() < 1e-3);
        assert!(z.rising);
        assert!(!z.just_reversed);
    }

    #[test]
    fn advance_bounces_at_period() {
        let mut z = z();
        z.advance(110.0); // 100 up + 10 back
        assert!((z.phase - 90.0).abs() < 1e-3);
        assert!(!z.rising);
        assert!(z.just_reversed);
    }

    #[test]
    fn advance_bounces_at_zero() {
        let mut z = z();
        z.advance(100.0); // reaches period, now falling
        z.just_reversed = false;
        z.advance(110.0); // 100 down + 10 up
        assert!((z.phase - 10.0).abs() < 1e-3);
        assert!(z.rising);
        assert!(z.just_reversed);
    }

    #[test]
    fn advance_double_bounce() {
        let mut z = z();
        z.advance(250.0); // 100 up, 100 down, 50 up
        assert!((z.phase - 50.0).abs() < 1e-3);
        assert!(z.rising);
        assert!(z.just_reversed);
    }

    #[test]
    fn advance_exact_period() {
        let mut z = z();
        z.advance(100.0);
        assert!((z.phase - 100.0).abs() < 1e-3);
        assert!(!z.rising);
        assert!(z.just_reversed);
    }

    #[test]
    fn advance_exact_full_cycle() {
        let mut z = z();
        z.advance(200.0); // up 100 + down 100
        assert!((z.phase - 0.0).abs() < 1e-3);
        assert!(z.rising);
    }

    #[test]
    fn advance_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.advance(50.0);
        assert_eq!(z.phase, 0.0);
    }

    #[test]
    fn advance_no_op_when_amount_zero() {
        let mut z = z();
        z.advance(0.0);
        assert_eq!(z.phase, 0.0);
    }

    // --- tick ---

    #[test]
    fn tick_advances_by_speed_times_dt() {
        let mut z = z(); // speed=20
        z.tick(2.0); // 0 + 20*2 = 40
        assert!((z.phase - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_reversed_on_bounce() {
        let mut z = z();
        z.phase = 90.0;
        z.tick(1.0); // 90 + 20 = 110 → bounce
        assert!(z.just_reversed);
        assert!(!z.rising);
    }

    #[test]
    fn tick_clears_just_reversed_next_frame() {
        let mut z = z();
        z.phase = 90.0;
        z.tick(1.0); // bounce fires
        z.tick(0.016); // cleared
        assert!(!z.just_reversed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(10.0);
        assert_eq!(z.phase, 0.0);
    }

    #[test]
    fn tick_no_op_when_speed_zero() {
        let mut z = Zigzag::new(100.0, 0.0);
        z.phase = 40.0;
        z.tick(100.0);
        assert!((z.phase - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_reversed_even_without_bounce() {
        let mut z = z();
        z.just_reversed = true;
        z.tick(0.1);
        assert!(!z.just_reversed);
    }

    #[test]
    fn tick_scales_advance_with_dt() {
        let mut z = z(); // speed=20
        z.tick(3.0); // 0 + 20*3 = 60
        assert!((z.phase - 60.0).abs() < 1e-3);
    }

    // --- is_rising / is_falling ---

    #[test]
    fn is_rising_true_at_start() {
        assert!(z().is_rising());
    }

    #[test]
    fn is_rising_false_when_disabled() {
        let mut z = z();
        z.enabled = false;
        assert!(!z.is_rising());
    }

    #[test]
    fn is_falling_true_after_bounce() {
        let mut z = z();
        z.advance(100.0);
        assert!(z.is_falling());
    }

    #[test]
    fn is_falling_false_when_disabled() {
        let mut z = z();
        z.advance(100.0);
        z.enabled = false;
        assert!(!z.is_falling());
    }

    // --- phase_fraction / effective_position ---

    #[test]
    fn phase_fraction_zero_at_start() {
        assert_eq!(z().phase_fraction(), 0.0);
    }

    #[test]
    fn phase_fraction_half_at_midpoint() {
        let mut z = z();
        z.phase = 50.0;
        assert!((z.phase_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn phase_fraction_one_at_period() {
        let mut z = z();
        z.phase = 100.0;
        assert!((z.phase_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_position_zero_at_start() {
        assert_eq!(z().effective_position(100.0), 0.0);
    }

    #[test]
    fn effective_position_scales_with_phase() {
        let mut z = z();
        z.phase = 75.0;
        assert!((z.effective_position(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_position_zero_when_disabled() {
        let mut z = z();
        z.phase = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_position(100.0), 0.0);
    }
}
