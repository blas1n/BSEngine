use bevy_ecs::prelude::Component;

/// Two-state alternating-stripe tracker. `phase` advances through
/// `[0, stripe_width)` and flips between dark and light bands each time
/// it wraps.
///
/// Models striped zones, alternating floor patterns, two-state oscillators,
/// lane-hazard toggles, day/night micro-cycles, or any mechanic that
/// repeatedly flips between two conditions on a regular interval.
///
/// `advance(amount)` moves `phase` forward. Each time `phase` reaches
/// `stripe_width` it wraps back to `0`, flips `dark_stripe`, and sets
/// `just_switched`. No-op when disabled.
///
/// `tick(dt)` clears `just_switched`, then calls `advance(speed * dt)`
/// when `speed > 0`. No-op when disabled.
///
/// `is_dark()` returns `dark_stripe && enabled`.
///
/// `is_light()` returns `!dark_stripe && enabled`.
///
/// `phase_fraction()` returns `phase / stripe_width` clamped to [0, 1].
///
/// `effective_contrast(dark_value, light_value)` returns `dark_value` when
/// in a dark stripe, `light_value` when in a light stripe; `0.0` when
/// disabled.
///
/// Default: `new(50.0, 20.0)` — stripe width 50, advance at 20 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zebra {
    pub phase: f32,
    pub stripe_width: f32,
    pub speed: f32,
    /// `true` = currently in a dark stripe; `false` = light stripe.
    pub dark_stripe: bool,
    pub just_switched: bool,
    pub enabled: bool,
}

impl Zebra {
    pub fn new(stripe_width: f32, speed: f32) -> Self {
        Self {
            phase: 0.0,
            stripe_width: stripe_width.max(0.1),
            speed: speed.max(0.0),
            dark_stripe: true,
            just_switched: false,
            enabled: true,
        }
    }

    /// Advance `phase` by `amount`, flipping stripe on each wrap.
    /// No-op when disabled.
    pub fn advance(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.phase += amount;
        while self.phase >= self.stripe_width {
            self.phase -= self.stripe_width;
            self.dark_stripe = !self.dark_stripe;
            self.just_switched = true;
        }
    }

    /// Clear `just_switched`, then advance by `speed * dt` when `speed > 0`.
    pub fn tick(&mut self, dt: f32) {
        self.just_switched = false;
        if self.enabled && self.speed > 0.0 {
            self.advance(self.speed * dt);
        }
    }

    /// `true` when in the dark stripe and enabled.
    pub fn is_dark(&self) -> bool {
        self.dark_stripe && self.enabled
    }

    /// `true` when in the light stripe and enabled.
    pub fn is_light(&self) -> bool {
        !self.dark_stripe && self.enabled
    }

    /// Fraction through the current stripe [0.0, 1.0].
    pub fn phase_fraction(&self) -> f32 {
        (self.phase / self.stripe_width).clamp(0.0, 1.0)
    }

    /// Returns `dark_value` in a dark stripe or `light_value` in a light
    /// stripe when enabled; `0.0` when disabled.
    pub fn effective_contrast(&self, dark_value: f32, light_value: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        if self.dark_stripe {
            dark_value
        } else {
            light_value
        }
    }
}

impl Default for Zebra {
    fn default() -> Self {
        Self::new(50.0, 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zebra {
        Zebra::new(50.0, 20.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_in_dark_stripe() {
        let z = z();
        assert_eq!(z.phase, 0.0);
        assert!(z.dark_stripe);
        assert!(z.is_dark());
        assert!(!z.is_light());
    }

    #[test]
    fn new_clamps_stripe_width() {
        let z = Zebra::new(-5.0, 20.0);
        assert!((z.stripe_width - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_speed() {
        let z = Zebra::new(50.0, -3.0);
        assert_eq!(z.speed, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zebra::default();
        assert!((z.stripe_width - 50.0).abs() < 1e-5);
        assert!((z.speed - 20.0).abs() < 1e-5);
    }

    // --- advance ---

    #[test]
    fn advance_increases_phase() {
        let mut z = z();
        z.advance(20.0);
        assert!((z.phase - 20.0).abs() < 1e-3);
        assert!(z.dark_stripe);
        assert!(!z.just_switched);
    }

    #[test]
    fn advance_wraps_and_flips_stripe() {
        let mut z = z();
        z.advance(50.0); // exactly one stripe
        assert!((z.phase).abs() < 1e-3);
        assert!(!z.dark_stripe); // flipped to light
        assert!(z.just_switched);
    }

    #[test]
    fn advance_wraps_with_remainder() {
        let mut z = z();
        z.advance(60.0); // 50 + 10 remainder
        assert!((z.phase - 10.0).abs() < 1e-3);
        assert!(!z.dark_stripe);
    }

    #[test]
    fn advance_multiple_wraps() {
        let mut z = z();
        z.advance(110.0); // 2 full stripes + 10
        assert!((z.phase - 10.0).abs() < 1e-3);
        assert!(z.dark_stripe); // back to dark (2 flips)
        assert!(z.just_switched);
    }

    #[test]
    fn advance_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.advance(100.0);
        assert_eq!(z.phase, 0.0);
        assert!(z.dark_stripe);
    }

    #[test]
    fn advance_no_op_when_amount_zero() {
        let mut z = z();
        z.advance(0.0);
        assert_eq!(z.phase, 0.0);
        assert!(!z.just_switched);
    }

    #[test]
    fn advance_accumulates_across_calls() {
        let mut z = z();
        z.advance(30.0);
        z.advance(30.0); // total 60 → flip
        assert!(!z.dark_stripe);
    }

    // --- tick ---

    #[test]
    fn tick_advances_phase_by_speed_times_dt() {
        let mut z = z(); // speed=20
        z.tick(1.0); // 0 + 20*1 = 20
        assert!((z.phase - 20.0).abs() < 1e-3);
    }

    #[test]
    fn tick_flips_stripe_on_wrap() {
        let mut z = z();
        z.phase = 40.0;
        z.tick(1.0); // 40 + 20 = 60 → wraps, flips
        assert!(z.just_switched);
        assert!(!z.dark_stripe);
    }

    #[test]
    fn tick_clears_just_switched_next_frame() {
        let mut z = z();
        z.phase = 40.0;
        z.tick(1.0); // just_switched fires
        z.tick(0.016); // cleared
        assert!(!z.just_switched);
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
        let mut z = Zebra::new(50.0, 0.0);
        z.phase = 25.0;
        z.tick(100.0);
        assert!((z.phase - 25.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_switched_even_without_wrap() {
        let mut z = z();
        z.just_switched = true;
        z.tick(0.01);
        assert!(!z.just_switched);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = Zebra::new(50.0, 5.0);
        z.tick(3.0); // 5*3 = 15
        assert!((z.phase - 15.0).abs() < 1e-3);
    }

    // --- is_dark / is_light ---

    #[test]
    fn is_dark_false_when_disabled() {
        let mut z = z();
        z.enabled = false;
        assert!(!z.is_dark());
    }

    #[test]
    fn is_light_false_when_disabled() {
        let mut z = z();
        z.dark_stripe = false;
        z.enabled = false;
        assert!(!z.is_light());
    }

    #[test]
    fn is_light_when_in_light_stripe() {
        let mut z = z();
        z.advance(50.0); // flip to light
        assert!(z.is_light());
        assert!(!z.is_dark());
    }

    // --- phase_fraction ---

    #[test]
    fn phase_fraction_zero_at_start() {
        assert_eq!(z().phase_fraction(), 0.0);
    }

    #[test]
    fn phase_fraction_half_at_midpoint() {
        let mut z = z();
        z.phase = 25.0;
        assert!((z.phase_fraction() - 0.5).abs() < 1e-4);
    }

    // --- effective_contrast ---

    #[test]
    fn effective_contrast_returns_dark_value_in_dark_stripe() {
        let z = z();
        assert!((z.effective_contrast(0.8, 0.2) - 0.8).abs() < 1e-4);
    }

    #[test]
    fn effective_contrast_returns_light_value_in_light_stripe() {
        let mut z = z();
        z.advance(50.0); // flip to light
        assert!((z.effective_contrast(0.8, 0.2) - 0.2).abs() < 1e-4);
    }

    #[test]
    fn effective_contrast_zero_when_disabled() {
        let z = Zebra {
            enabled: false,
            ..Zebra::default()
        };
        assert_eq!(z.effective_contrast(1.0, 0.5), 0.0);
    }
}
