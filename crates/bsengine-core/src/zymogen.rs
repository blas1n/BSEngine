use bevy_ecs::prelude::Component;

/// Enzyme-precursor tracker. `precursor` builds via `prime(amount)` and
/// accumulates passively at `activate_rate` per second in `tick(dt)` or
/// is cleaved immediately via `cleave(amount)`.
///
/// Models pro-enzyme storage fill levels, inactive-precursor accumulation
/// bars, signal-cascade priming gauges, substrate-readiness saturation
/// trackers, latent-catalyst reserve meters, biochemical-potential
/// build-up indicators, trypsinogen-to-trypsin conversion progress bars,
/// clotting-factor priming fill levels, complement-cascade readiness
/// trackers, or any mechanic where an inactive precursor accumulates in
/// safe storage until a cleavage signal transforms it into the active
/// form that actually does the enzymatic work — at which point the
/// precursor pool is drawn down and must refill from scratch.
///
/// `prime(amount)` adds precursor; fires `just_primed` when first
/// reaching `max_precursor`. No-op when disabled.
///
/// `cleave(amount)` reduces precursor immediately; fires `just_cleaved`
/// when reaching 0. No-op when disabled or already cleaved.
///
/// `tick(dt)` clears both flags, then increases precursor by
/// `activate_rate * dt` (capped at `max_precursor`). Fires `just_primed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_primed()` returns `precursor >= max_precursor && enabled`.
///
/// `is_cleaved()` returns `precursor == 0.0` (not gated by `enabled`).
///
/// `precursor_fraction()` returns `(precursor / max_precursor).clamp(0, 1)`.
///
/// `effective_catalysis(scale)` returns `scale * precursor_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — primes at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zymogen {
    pub precursor: f32,
    pub max_precursor: f32,
    pub activate_rate: f32,
    pub just_primed: bool,
    pub just_cleaved: bool,
    pub enabled: bool,
}

impl Zymogen {
    pub fn new(max_precursor: f32, activate_rate: f32) -> Self {
        Self {
            precursor: 0.0,
            max_precursor: max_precursor.max(0.1),
            activate_rate: activate_rate.max(0.0),
            just_primed: false,
            just_cleaved: false,
            enabled: true,
        }
    }

    /// Add precursor; fires `just_primed` when first reaching max.
    /// No-op when disabled.
    pub fn prime(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.precursor < self.max_precursor;
        self.precursor = (self.precursor + amount).min(self.max_precursor);
        if was_below && self.precursor >= self.max_precursor {
            self.just_primed = true;
        }
    }

    /// Reduce precursor; fires `just_cleaved` when reaching 0.
    /// No-op when disabled or already cleaved.
    pub fn cleave(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.precursor <= 0.0 {
            return;
        }
        self.precursor = (self.precursor - amount).max(0.0);
        if self.precursor <= 0.0 {
            self.just_cleaved = true;
        }
    }

    /// Clear flags, then increase precursor by `activate_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_primed = false;
        self.just_cleaved = false;
        if self.enabled && self.activate_rate > 0.0 && self.precursor < self.max_precursor {
            let was_below = self.precursor < self.max_precursor;
            self.precursor = (self.precursor + self.activate_rate * dt).min(self.max_precursor);
            if was_below && self.precursor >= self.max_precursor {
                self.just_primed = true;
            }
        }
    }

    /// `true` when precursor is at maximum and component is enabled.
    pub fn is_primed(&self) -> bool {
        self.precursor >= self.max_precursor && self.enabled
    }

    /// `true` when precursor is 0 (not gated by `enabled`).
    pub fn is_cleaved(&self) -> bool {
        self.precursor == 0.0
    }

    /// Fraction of maximum precursor [0.0, 1.0].
    pub fn precursor_fraction(&self) -> f32 {
        (self.precursor / self.max_precursor).clamp(0.0, 1.0)
    }

    /// Returns `scale * precursor_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_catalysis(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.precursor_fraction()
    }
}

impl Default for Zymogen {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zymogen {
        Zymogen::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_cleaved() {
        let z = z();
        assert_eq!(z.precursor, 0.0);
        assert!(z.is_cleaved());
        assert!(!z.is_primed());
    }

    #[test]
    fn new_clamps_max_precursor() {
        let z = Zymogen::new(-5.0, 1.5);
        assert!((z.max_precursor - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_activate_rate() {
        let z = Zymogen::new(100.0, -1.5);
        assert_eq!(z.activate_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zymogen::default();
        assert!((z.max_precursor - 100.0).abs() < 1e-5);
        assert!((z.activate_rate - 1.5).abs() < 1e-5);
    }

    // --- prime ---

    #[test]
    fn prime_adds_precursor() {
        let mut z = z();
        z.prime(40.0);
        assert!((z.precursor - 40.0).abs() < 1e-3);
    }

    #[test]
    fn prime_clamps_at_max() {
        let mut z = z();
        z.prime(200.0);
        assert!((z.precursor - 100.0).abs() < 1e-3);
    }

    #[test]
    fn prime_fires_just_primed_at_max() {
        let mut z = z();
        z.prime(100.0);
        assert!(z.just_primed);
        assert!(z.is_primed());
    }

    #[test]
    fn prime_no_just_primed_when_already_at_max() {
        let mut z = z();
        z.precursor = 100.0;
        z.prime(10.0);
        assert!(!z.just_primed);
    }

    #[test]
    fn prime_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.prime(50.0);
        assert_eq!(z.precursor, 0.0);
    }

    #[test]
    fn prime_no_op_when_amount_zero() {
        let mut z = z();
        z.prime(0.0);
        assert_eq!(z.precursor, 0.0);
    }

    // --- cleave ---

    #[test]
    fn cleave_reduces_precursor() {
        let mut z = z();
        z.precursor = 60.0;
        z.cleave(20.0);
        assert!((z.precursor - 40.0).abs() < 1e-3);
    }

    #[test]
    fn cleave_clamps_at_zero() {
        let mut z = z();
        z.precursor = 30.0;
        z.cleave(200.0);
        assert_eq!(z.precursor, 0.0);
    }

    #[test]
    fn cleave_fires_just_cleaved_at_zero() {
        let mut z = z();
        z.precursor = 30.0;
        z.cleave(30.0);
        assert!(z.just_cleaved);
    }

    #[test]
    fn cleave_no_op_when_already_cleaved() {
        let mut z = z();
        z.cleave(10.0);
        assert!(!z.just_cleaved);
    }

    #[test]
    fn cleave_no_op_when_disabled() {
        let mut z = z();
        z.precursor = 50.0;
        z.enabled = false;
        z.cleave(50.0);
        assert!((z.precursor - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_primes_precursor() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.precursor - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_primed_on_activate_to_max() {
        let mut z = Zymogen::new(100.0, 200.0);
        z.precursor = 95.0;
        z.tick(1.0);
        assert!(z.just_primed);
        assert!(z.is_primed());
    }

    #[test]
    fn tick_no_prime_when_already_primed() {
        let mut z = z();
        z.precursor = 100.0;
        z.tick(1.0);
        assert!(!z.just_primed);
    }

    #[test]
    fn tick_no_prime_when_rate_zero() {
        let mut z = Zymogen::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.precursor, 0.0);
    }

    #[test]
    fn tick_no_prime_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.precursor, 0.0);
    }

    #[test]
    fn tick_clears_just_primed() {
        let mut z = Zymogen::new(100.0, 200.0);
        z.precursor = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_primed);
    }

    #[test]
    fn tick_clears_just_cleaved() {
        let mut z = z();
        z.precursor = 10.0;
        z.cleave(10.0);
        z.tick(0.016);
        assert!(!z.just_cleaved);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.precursor - 9.0).abs() < 1e-3);
    }

    // --- is_primed / is_cleaved ---

    #[test]
    fn is_primed_false_when_disabled() {
        let mut z = z();
        z.precursor = 100.0;
        z.enabled = false;
        assert!(!z.is_primed());
    }

    #[test]
    fn is_cleaved_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_cleaved());
    }

    // --- precursor_fraction / effective_catalysis ---

    #[test]
    fn precursor_fraction_zero_when_cleaved() {
        assert_eq!(z().precursor_fraction(), 0.0);
    }

    #[test]
    fn precursor_fraction_half_at_midpoint() {
        let mut z = z();
        z.precursor = 50.0;
        assert!((z.precursor_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_catalysis_zero_when_cleaved() {
        assert_eq!(z().effective_catalysis(100.0), 0.0);
    }

    #[test]
    fn effective_catalysis_scales_with_precursor() {
        let mut z = z();
        z.precursor = 75.0;
        assert!((z.effective_catalysis(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_catalysis_zero_when_disabled() {
        let mut z = z();
        z.precursor = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_catalysis(100.0), 0.0);
    }
}
