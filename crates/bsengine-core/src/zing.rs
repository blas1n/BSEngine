use bevy_ecs::prelude::Component;

/// Charge-and-fire threshold trigger. Accumulates `zing_charge` via
/// `charge(amount)`; each time the charge meets `zing_threshold` it
/// automatically discharges: `just_zinged` fires, `zing_count` increments,
/// and `zing_charge` resets — all within the same call. A single
/// `charge()` call can fire multiple times if the amount exceeds several
/// multiples of the threshold.
///
/// Unlike `Wow` (a duration gate that must manually re-trigger) and `Witch`
/// (managed charge slots), Zing is **self-resetting on threshold crossing**:
/// no user action is required to re-arm it. The mechanic models static
/// buildup, overload bursts, or combo finishers that fire automatically once
/// enough energy accumulates.
///
/// `charge(amount)` adds `amount` to `zing_charge`. Fires `just_zinged` and
/// increments `zing_count` for every full threshold's worth of charge
/// consumed. No-op when `amount <= 0` or disabled.
///
/// `drain()` resets `zing_charge` to 0 without firing. Models a
/// safe-discharge (grounding) that wastes accumulated charge.
///
/// `tick(_dt)` clears one-frame flags only. No time-based logic.
///
/// `is_charged()` returns `zing_charge > 0.0 && enabled`.
///
/// `charge_fraction()` returns `(zing_charge / zing_threshold).clamp(0.0, 1.0)`.
///
/// `effective_spark(base)` returns `base * (1.0 + charge_fraction())` when
/// enabled — 1× when empty, 2× when at threshold; `base` when disabled.
///
/// Default: `new(10.0)` — fires every 10 units of charge.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zing {
    /// Current charge below the next threshold [0, zing_threshold).
    pub zing_charge: f32,
    /// Charge required to trigger a zing. Clamped >= 0.1.
    pub zing_threshold: f32,
    /// Lifetime count of times zinged.
    pub zing_count: u32,
    pub just_zinged: bool,
    pub enabled: bool,
}

impl Zing {
    pub fn new(zing_threshold: f32) -> Self {
        Self {
            zing_charge: 0.0,
            zing_threshold: zing_threshold.max(0.1),
            zing_count: 0,
            just_zinged: false,
            enabled: true,
        }
    }

    /// Add charge. Fires `just_zinged` and increments `zing_count` once per
    /// threshold crossed. No-op when `amount <= 0` or disabled.
    pub fn charge(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.zing_charge += amount;
        while self.zing_charge >= self.zing_threshold {
            self.zing_charge -= self.zing_threshold;
            self.zing_count += 1;
            self.just_zinged = true;
        }
    }

    /// Reset charge to 0 without firing. No-op when already at 0.
    pub fn drain(&mut self) {
        self.zing_charge = 0.0;
    }

    /// Advance one frame: clear one-frame flags only. No time-based changes.
    pub fn tick(&mut self, _dt: f32) {
        self.just_zinged = false;
    }

    /// `true` when charge is non-zero and component is enabled.
    pub fn is_charged(&self) -> bool {
        self.zing_charge > 0.0 && self.enabled
    }

    /// Charge as a fraction of the next threshold [0.0, 1.0].
    pub fn charge_fraction(&self) -> f32 {
        (self.zing_charge / self.zing_threshold).clamp(0.0, 1.0)
    }

    /// Scale `base` by current charge level. Returns `base * (1.0 +
    /// charge_fraction())` when enabled — 1× when empty, 2× at threshold;
    /// `base` when disabled.
    pub fn effective_spark(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.charge_fraction())
    }
}

impl Default for Zing {
    fn default() -> Self {
        Self::new(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zing {
        Zing::new(10.0) // fires every 10 units
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let z = z();
        assert_eq!(z.zing_charge, 0.0);
        assert_eq!(z.zing_count, 0);
        assert!(!z.just_zinged);
        assert!(!z.is_charged());
    }

    #[test]
    fn zing_threshold_clamped_to_point_one() {
        let z = Zing::new(0.0);
        assert!((z.zing_threshold - 0.1).abs() < 1e-5);
    }

    // --- charge ---

    #[test]
    fn charge_accumulates_below_threshold() {
        let mut z = z();
        z.charge(6.0);
        assert!((z.zing_charge - 6.0).abs() < 1e-4);
        assert_eq!(z.zing_count, 0);
        assert!(!z.just_zinged);
    }

    #[test]
    fn charge_fires_at_threshold() {
        let mut z = z(); // threshold=10
        z.charge(10.0); // exactly hits
        assert!(z.just_zinged);
        assert_eq!(z.zing_count, 1);
        assert_eq!(z.zing_charge, 0.0);
    }

    #[test]
    fn charge_fires_crossing_threshold() {
        let mut z = z();
        z.charge(12.0); // 2.0 remainder
        assert!(z.just_zinged);
        assert_eq!(z.zing_count, 1);
        assert!((z.zing_charge - 2.0).abs() < 1e-4);
    }

    #[test]
    fn charge_fires_multiple_times_in_one_call() {
        let mut z = z(); // threshold=10
        z.charge(25.0); // fires ×2, 5.0 remainder
        assert!(z.just_zinged);
        assert_eq!(z.zing_count, 2);
        assert!((z.zing_charge - 5.0).abs() < 1e-4);
    }

    #[test]
    fn charge_three_times_across_calls() {
        let mut z = z();
        z.charge(10.0); // count=1
        z.tick(0.016);
        z.charge(10.0); // count=2
        z.tick(0.016);
        z.charge(10.0); // count=3
        assert_eq!(z.zing_count, 3);
    }

    #[test]
    fn charge_partial_then_fires_on_next_call() {
        let mut z = z();
        z.charge(7.0); // 7.0 accumulated
        z.tick(0.016);
        z.charge(4.0); // 11.0 total → fires once, 1.0 remainder
        assert!(z.just_zinged);
        assert_eq!(z.zing_count, 1);
        assert!((z.zing_charge - 1.0).abs() < 1e-4);
    }

    #[test]
    fn charge_no_op_with_zero() {
        let mut z = z();
        z.charge(0.0);
        assert_eq!(z.zing_charge, 0.0);
        assert!(!z.just_zinged);
    }

    #[test]
    fn charge_no_op_with_negative() {
        let mut z = z();
        z.charge(-5.0);
        assert_eq!(z.zing_charge, 0.0);
        assert!(!z.just_zinged);
    }

    #[test]
    fn charge_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.charge(10.0);
        assert_eq!(z.zing_charge, 0.0);
        assert_eq!(z.zing_count, 0);
        assert!(!z.just_zinged);
    }

    // --- drain ---

    #[test]
    fn drain_resets_charge() {
        let mut z = z();
        z.charge(7.0);
        z.drain();
        assert_eq!(z.zing_charge, 0.0);
    }

    #[test]
    fn drain_does_not_fire_just_zinged() {
        let mut z = z();
        z.charge(7.0);
        z.drain();
        assert!(!z.just_zinged);
    }

    #[test]
    fn drain_does_not_increment_count() {
        let mut z = z();
        z.charge(7.0);
        z.drain();
        assert_eq!(z.zing_count, 0);
    }

    #[test]
    fn drain_no_op_when_already_empty() {
        let mut z = z();
        z.drain(); // fine, no-op
        assert_eq!(z.zing_charge, 0.0);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_zinged() {
        let mut z = z();
        z.charge(10.0); // just_zinged=true
        z.tick(0.016);
        assert!(!z.just_zinged);
    }

    #[test]
    fn tick_does_not_change_charge_or_count() {
        let mut z = z();
        z.charge(5.0);
        z.tick(1000.0); // no time-based change
        assert!((z.zing_charge - 5.0).abs() < 1e-4);
        assert_eq!(z.zing_count, 0);
    }

    // --- is_charged ---

    #[test]
    fn is_charged_false_when_empty() {
        let z = z();
        assert!(!z.is_charged());
    }

    #[test]
    fn is_charged_true_with_partial_charge() {
        let mut z = z();
        z.charge(5.0);
        assert!(z.is_charged());
    }

    #[test]
    fn is_charged_false_after_exact_threshold() {
        let mut z = z();
        z.charge(10.0); // fires, resets to 0
        assert!(!z.is_charged());
    }

    #[test]
    fn is_charged_true_with_remainder() {
        let mut z = z();
        z.charge(12.0); // 2.0 remainder
        assert!(z.is_charged());
    }

    #[test]
    fn is_charged_false_when_disabled() {
        let mut z = z();
        z.charge(5.0);
        z.enabled = false;
        assert!(!z.is_charged());
    }

    // --- charge_fraction ---

    #[test]
    fn charge_fraction_zero_when_empty() {
        let z = z();
        assert_eq!(z.charge_fraction(), 0.0);
    }

    #[test]
    fn charge_fraction_half_at_midpoint() {
        let mut z = z(); // threshold=10
        z.charge(5.0); // 5/10=0.5
        assert!((z.charge_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn charge_fraction_zero_after_exact_fire() {
        let mut z = z();
        z.charge(10.0); // fires, remainder=0 → 0/10=0
        assert_eq!(z.charge_fraction(), 0.0);
    }

    #[test]
    fn charge_fraction_partial_after_over_fire() {
        let mut z = z();
        z.charge(15.0); // fires once, 5.0 remainder → 5/10=0.5
        assert!((z.charge_fraction() - 0.5).abs() < 1e-4);
    }

    // --- effective_spark ---

    #[test]
    fn effective_spark_passthrough_when_empty() {
        let z = z();
        assert!((z.effective_spark(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_spark_at_half_charge() {
        let mut z = z();
        z.charge(5.0); // fraction=0.5 → 100*(1+0.5)=150
        assert!((z.effective_spark(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_spark_passthrough_when_disabled() {
        let mut z = z();
        z.charge(5.0);
        z.enabled = false;
        assert!((z.effective_spark(100.0) - 100.0).abs() < 1e-4);
    }

    // --- zing_count monotonicity ---

    #[test]
    fn zing_count_never_decreases() {
        let mut z = z();
        for _ in 0..5 {
            z.charge(10.0);
            z.tick(0.016);
        }
        assert_eq!(z.zing_count, 5);
    }
}
