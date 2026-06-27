use bevy_ecs::prelude::Component;

/// Integer-charge spell caster with sub-second fractional regen accumulator.
/// Tracks discrete spell charges that regenerate over time and are consumed by
/// casting. Multiple charges may be gained in a single tick when
/// `regen_rate` is high.
///
/// Unlike `Cooldown` (single timer, no stacking), `Fuel` (continuous float
/// resource), and `Ammo` (external reload required), Witch charges regenerate
/// **automatically** through the standard `tick()` loop with no manual
/// intervention, and track an integer count rather than a float level.
///
/// `cast()` consumes one charge. Fires `just_exhausted` when `charge_count`
/// reaches 0. No-op when at 0 or disabled.
///
/// `tick(dt)` clears one-frame flags first, then if enabled and
/// `charge_count < max_charges`: accumulates `regen_rate * dt` into
/// `charge_accum`. For each whole unit in `charge_accum`, increments
/// `charge_count` (fires `just_charged`) and decrements `charge_accum` by 1.0.
/// Clamps to `max_charges`. Clears `charge_accum` when full (no waste).
/// No-op (beyond flag clear) when disabled or already full.
///
/// `is_ready()` returns `charge_count > 0 && enabled`.
///
/// `is_full()` returns `charge_count >= max_charges && enabled`.
///
/// `charge_fraction()` returns `(charge_count as f32 / max_charges as f32).clamp(0.0, 1.0)`.
///
/// `effective_potency(base)` returns `base * charge_fraction()` when enabled —
/// scales 0→base as charges fill; returns `base` unchanged when disabled.
///
/// Default: `new(3, 1.0)` — 3 max charges, 1 charge/second regen.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Witch {
    /// Current integer charge count [0, max_charges].
    pub charge_count: u32,
    /// Maximum number of charges. Clamped >= 1.
    pub max_charges: u32,
    /// Charge regen rate in charges/second. Clamped >= 0.0.
    pub regen_rate: f32,
    /// Sub-unit fractional accumulator for regen [0.0, 1.0).
    pub charge_accum: f32,
    pub just_charged: bool,
    pub just_exhausted: bool,
    pub enabled: bool,
}

impl Witch {
    pub fn new(max_charges: u32, regen_rate: f32) -> Self {
        Self {
            charge_count: 0,
            max_charges: max_charges.max(1),
            regen_rate: regen_rate.max(0.0),
            charge_accum: 0.0,
            just_charged: false,
            just_exhausted: false,
            enabled: true,
        }
    }

    /// Consume one charge. Fires `just_exhausted` when reaching 0. No-op at
    /// 0 or when disabled.
    pub fn cast(&mut self) {
        if !self.enabled || self.charge_count == 0 {
            return;
        }
        self.charge_count -= 1;
        if self.charge_count == 0 {
            self.just_exhausted = true;
        }
    }

    /// Advance one frame: clear flags, then regen charges. No-op (beyond flag
    /// clear) when disabled or already full.
    pub fn tick(&mut self, dt: f32) {
        self.just_charged = false;
        self.just_exhausted = false;

        if !self.enabled || self.charge_count >= self.max_charges {
            return;
        }

        self.charge_accum += self.regen_rate * dt;
        while self.charge_accum >= 1.0 && self.charge_count < self.max_charges {
            self.charge_count += 1;
            self.charge_accum -= 1.0;
            self.just_charged = true;
        }
        if self.charge_count >= self.max_charges {
            self.charge_accum = 0.0;
        }
    }

    /// `true` when at least one charge is available and component is enabled.
    pub fn is_ready(&self) -> bool {
        self.charge_count > 0 && self.enabled
    }

    /// `true` when at maximum charges and component is enabled.
    pub fn is_full(&self) -> bool {
        self.charge_count >= self.max_charges && self.enabled
    }

    /// Charge count as a fraction of maximum [0.0, 1.0].
    pub fn charge_fraction(&self) -> f32 {
        (self.charge_count as f32 / self.max_charges as f32).clamp(0.0, 1.0)
    }

    /// Scale `base` by charge fraction. Returns `base * charge_fraction()`
    /// when enabled; `base` when disabled.
    pub fn effective_potency(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * self.charge_fraction()
    }
}

impl Default for Witch {
    fn default() -> Self {
        Self::new(3, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Witch {
        Witch::new(3, 1.0) // 3 max charges, 1/s regen
    }

    #[test]
    fn new_starts_empty() {
        let w = w();
        assert_eq!(w.charge_count, 0);
        assert_eq!(w.charge_accum, 0.0);
        assert!(!w.just_charged);
        assert!(!w.just_exhausted);
        assert!(!w.is_ready());
        assert!(!w.is_full());
    }

    // --- tick: regen ---

    #[test]
    fn tick_accumulates_below_threshold() {
        let mut w = w();
        w.tick(0.5); // 0.5 accumulated, no charge gained
        assert_eq!(w.charge_count, 0);
        assert!((w.charge_accum - 0.5).abs() < 1e-5);
        assert!(!w.just_charged);
    }

    #[test]
    fn tick_grants_charge_at_one_second() {
        let mut w = w();
        w.tick(1.0);
        assert_eq!(w.charge_count, 1);
        assert!(w.just_charged);
    }

    #[test]
    fn tick_grants_multiple_charges_in_one_tick() {
        let mut w = w(); // max=3, rate=1/s
        w.tick(2.5); // 2 charges + 0.5 leftover
        assert_eq!(w.charge_count, 2);
        assert!((w.charge_accum - 0.5).abs() < 1e-5);
        assert!(w.just_charged);
    }

    #[test]
    fn tick_clamps_at_max_charges() {
        let mut w = w(); // max=3
        w.tick(10.0); // way more than enough
        assert_eq!(w.charge_count, 3);
    }

    #[test]
    fn tick_clears_accum_when_full() {
        let mut w = w();
        w.tick(10.0);
        assert_eq!(w.charge_accum, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_full() {
        let mut w = w();
        w.tick(10.0); // fill up
        w.tick(10.0); // should not add more
        assert_eq!(w.charge_count, 3);
        assert!(!w.just_charged);
    }

    #[test]
    fn tick_clears_just_charged_next_frame() {
        let mut w = w();
        w.tick(1.0); // just_charged=true
        w.tick(0.1);
        assert!(!w.just_charged);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(10.0);
        assert_eq!(w.charge_count, 0);
        assert_eq!(w.charge_accum, 0.0);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_charged = true;
        w.just_exhausted = true;
        w.enabled = false;
        w.tick(1.0);
        assert!(!w.just_charged);
        assert!(!w.just_exhausted);
    }

    #[test]
    fn tick_accumulator_carries_over() {
        let mut w = w();
        w.tick(0.6); // accum=0.6, no charge
        w.tick(0.6); // accum=1.2 → 1 charge, leftover 0.2
        assert_eq!(w.charge_count, 1);
        assert!((w.charge_accum - 0.2).abs() < 1e-4);
    }

    // --- cast ---

    #[test]
    fn cast_consumes_charge() {
        let mut w = w();
        w.tick(2.0); // 2 charges
        w.cast();
        assert_eq!(w.charge_count, 1);
    }

    #[test]
    fn cast_fires_just_exhausted_on_last_charge() {
        let mut w = w();
        w.tick(1.0); // 1 charge
        w.cast();
        assert!(w.just_exhausted);
        assert_eq!(w.charge_count, 0);
    }

    #[test]
    fn cast_does_not_fire_just_exhausted_unless_empty() {
        let mut w = w();
        w.tick(2.0); // 2 charges
        w.cast(); // 1 remaining — not exhausted
        assert!(!w.just_exhausted);
    }

    #[test]
    fn cast_no_op_when_empty() {
        let mut w = w();
        w.cast(); // already 0
        assert_eq!(w.charge_count, 0);
        assert!(!w.just_exhausted);
    }

    #[test]
    fn cast_no_op_when_disabled() {
        let mut w = w();
        w.tick(2.0);
        w.enabled = false;
        w.cast();
        assert_eq!(w.charge_count, 2);
    }

    #[test]
    fn tick_clears_just_exhausted_next_frame() {
        let mut w = w();
        w.tick(1.0); // 1 charge
        w.cast(); // just_exhausted=true
        w.tick(0.016);
        assert!(!w.just_exhausted);
    }

    // --- is_ready / is_full ---

    #[test]
    fn is_ready_false_when_empty() {
        let w = w();
        assert!(!w.is_ready());
    }

    #[test]
    fn is_ready_true_with_charges() {
        let mut w = w();
        w.tick(1.0);
        assert!(w.is_ready());
    }

    #[test]
    fn is_ready_false_when_disabled() {
        let mut w = w();
        w.tick(1.0);
        w.enabled = false;
        assert!(!w.is_ready());
    }

    #[test]
    fn is_full_false_when_not_at_max() {
        let mut w = w();
        w.tick(2.0); // 2 of 3
        assert!(!w.is_full());
    }

    #[test]
    fn is_full_true_at_max() {
        let mut w = w();
        w.tick(3.0);
        assert!(w.is_full());
    }

    #[test]
    fn is_full_false_when_disabled() {
        let mut w = w();
        w.tick(3.0);
        w.enabled = false;
        assert!(!w.is_full());
    }

    // --- charge_fraction ---

    #[test]
    fn charge_fraction_zero_when_empty() {
        let w = w();
        assert_eq!(w.charge_fraction(), 0.0);
    }

    #[test]
    fn charge_fraction_at_partial() {
        let mut w = Witch::new(4, 1.0);
        w.tick(2.0); // 2/4 = 0.5
        assert!((w.charge_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn charge_fraction_one_when_full() {
        let mut w = w();
        w.tick(3.0);
        assert!((w.charge_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_potency ---

    #[test]
    fn effective_potency_zero_when_empty() {
        let w = w();
        assert!((w.effective_potency(100.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn effective_potency_partial_at_partial_charges() {
        let mut w = Witch::new(4, 1.0);
        w.tick(2.0); // 2/4=0.5 → 100*0.5=50
        assert!((w.effective_potency(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_potency_full_at_max_charges() {
        let mut w = w();
        w.tick(3.0); // fraction=1.0 → 100
        assert!((w.effective_potency(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_potency_passthrough_when_disabled() {
        let mut w = w();
        w.tick(2.0);
        w.enabled = false;
        assert!((w.effective_potency(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_charges_clamped_to_one() {
        let w = Witch::new(0, 1.0);
        assert_eq!(w.max_charges, 1);
    }

    #[test]
    fn regen_rate_clamped_to_zero() {
        let w = Witch::new(3, -1.0);
        assert_eq!(w.regen_rate, 0.0);
    }

    #[test]
    fn zero_regen_rate_never_gains_charges() {
        let mut w = Witch::new(3, 0.0);
        w.tick(100.0);
        assert_eq!(w.charge_count, 0);
    }

    // --- cast-then-regen cycle ---

    #[test]
    fn cast_then_regen_cycle() {
        let mut w = w(); // max=3, 1/s
        w.tick(3.0); // full (3 charges)
        w.cast(); // 2 remaining
        w.cast(); // 1 remaining
        w.tick(2.0); // regen 2 → back to 3
        assert_eq!(w.charge_count, 3);
    }

    #[test]
    fn high_regen_fills_instantly() {
        let mut w = Witch::new(5, 10.0);
        w.tick(0.5); // 10*0.5=5 → all 5 charges
        assert_eq!(w.charge_count, 5);
        assert_eq!(w.charge_accum, 0.0);
    }
}
