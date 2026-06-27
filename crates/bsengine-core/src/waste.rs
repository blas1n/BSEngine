use bevy_ecs::prelude::Component;

/// Maximum HP erosion from a wasting disease. While active, `accumulated`
/// rises at `decay_rate` per second (as a fraction of base max HP). Callers
/// use `effective_max_hp(base_max)` to obtain the reduced ceiling, then clamp
/// current HP to it if it exceeds the new maximum.
///
/// `tick(dt)` advances `accumulated` by `decay_rate * dt`, capped at 1.0.
/// Fires `just_peaked` on the tick that first reaches full wasting
/// (`accumulated ≥ 1.0`). Clears `just_peaked` at the start of each tick.
///
/// `effective_max_hp(base_max)` returns
/// `base_max * (1.0 - accumulated).max(0.0)` when enabled; returns `base_max`
/// when disabled. Returns 0.0 when fully wasted and enabled.
///
/// `cleanse(amount)` reduces `accumulated` by `amount` (floored at 0.0),
/// representing treatment. No-op when disabled or `amount ≤ 0`.
///
/// `is_fully_wasted()` returns `accumulated >= 1.0 && enabled`.
///
/// Distinct from `Bleed` (current HP loss over time), `Burn` (fire damage
/// over time), `Corrosion` (armor/defense reduction), and `Doom` (lethal
/// countdown clock): Waste is a **maximum HP erosion** — it shrinks the HP
/// ceiling rather than draining the current pool directly, so a healthy entity
/// gradually loses the headroom to absorb damage.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Waste {
    /// Fractional maximum HP erosion accumulated so far [0.0, 1.0].
    pub accumulated: f32,
    /// Fraction of base max HP eroded per second. Clamped [0.0, 1.0].
    pub decay_rate: f32,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Waste {
    pub fn new(decay_rate: f32) -> Self {
        Self {
            accumulated: 0.0,
            decay_rate: decay_rate.clamp(0.0, 1.0),
            just_peaked: false,
            enabled: true,
        }
    }

    /// Advance the wasting. Clears `just_peaked` at start, then adds
    /// `decay_rate * dt` to `accumulated` (capped at 1.0). Fires
    /// `just_peaked` on the tick that first reaches full wasting.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;

        if !self.enabled || self.decay_rate <= 0.0 {
            return;
        }
        if self.accumulated < 1.0 {
            let was_below = self.accumulated < 1.0;
            self.accumulated = (self.accumulated + self.decay_rate * dt).min(1.0);
            if was_below && self.accumulated >= 1.0 {
                self.just_peaked = true;
            }
        }
    }

    /// Reduce the accumulated wasting by `amount` (floored at 0.0).
    /// No-op when disabled or `amount ≤ 0`.
    pub fn cleanse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.accumulated = (self.accumulated - amount).max(0.0);
    }

    /// Effective maximum HP after erosion.
    /// Returns `base_max * (1.0 - accumulated).max(0.0)` when enabled;
    /// returns `base_max` when disabled.
    pub fn effective_max_hp(&self, base_max: f32) -> f32 {
        if self.enabled {
            base_max * (1.0 - self.accumulated).max(0.0)
        } else {
            base_max
        }
    }

    /// `true` when the entity is fully wasted and the component is enabled.
    pub fn is_fully_wasted(&self) -> bool {
        self.accumulated >= 1.0 && self.enabled
    }
}

impl Default for Waste {
    fn default() -> Self {
        Self::new(0.02)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_clean() {
        let w = Waste::new(0.02);
        assert_eq!(w.accumulated, 0.0);
        assert!(!w.is_fully_wasted());
    }

    #[test]
    fn tick_advances_accumulated() {
        let mut w = Waste::new(0.1);
        w.tick(1.0); // 0.1 * 1 = 0.1
        assert!((w.accumulated - 0.1).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_one() {
        let mut w = Waste::new(0.5);
        w.tick(10.0); // would exceed 1.0
        assert!((w.accumulated - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_peaked_on_first_full() {
        let mut w = Waste::new(1.0);
        w.tick(1.0); // reaches 1.0
        assert!(w.just_peaked);
        assert!(w.is_fully_wasted());
    }

    #[test]
    fn tick_no_just_peaked_when_already_full() {
        let mut w = Waste::new(1.0);
        w.tick(1.0); // peaks
        w.tick(1.0); // already full
        assert!(!w.just_peaked);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut w = Waste::new(1.0);
        w.tick(1.0); // peaks
        w.tick(0.016);
        assert!(!w.just_peaked);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = Waste::new(0.1);
        w.enabled = false;
        w.tick(10.0);
        assert_eq!(w.accumulated, 0.0);
    }

    #[test]
    fn tick_no_op_at_zero_decay_rate() {
        let mut w = Waste::new(0.0);
        w.tick(100.0);
        assert_eq!(w.accumulated, 0.0);
    }

    #[test]
    fn cleanse_reduces_accumulated() {
        let mut w = Waste::new(0.1);
        w.tick(5.0); // 0.5
        w.cleanse(0.2);
        assert!((w.accumulated - 0.3).abs() < 1e-5);
    }

    #[test]
    fn cleanse_floors_at_zero() {
        let mut w = Waste::new(0.1);
        w.tick(1.0); // 0.1
        w.cleanse(0.5); // would go negative
        assert_eq!(w.accumulated, 0.0);
    }

    #[test]
    fn cleanse_no_op_when_disabled() {
        let mut w = Waste::new(0.5);
        w.tick(1.0); // 0.5
        w.enabled = false;
        w.cleanse(0.5);
        assert!((w.accumulated - 0.5).abs() < 1e-5);
    }

    #[test]
    fn cleanse_no_op_at_zero_amount() {
        let mut w = Waste::new(0.1);
        w.tick(1.0); // 0.1
        w.cleanse(0.0);
        assert!((w.accumulated - 0.1).abs() < 1e-5);
    }

    #[test]
    fn effective_max_hp_reduces_with_accumulated() {
        let mut w = Waste::new(0.5);
        w.tick(1.0); // accumulated = 0.5
                     // 100 * (1 - 0.5) = 50
        assert!((w.effective_max_hp(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_max_hp_at_zero_accumulated() {
        let w = Waste::new(0.1);
        assert!((w.effective_max_hp(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_max_hp_at_full_accumulated() {
        let mut w = Waste::new(1.0);
        w.tick(5.0); // fully wasted
        assert_eq!(w.effective_max_hp(100.0), 0.0);
    }

    #[test]
    fn effective_max_hp_base_when_disabled() {
        let mut w = Waste::new(0.5);
        w.tick(1.0); // accumulated = 0.5
        w.enabled = false;
        assert!((w.effective_max_hp(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn is_fully_wasted_true_at_one() {
        let mut w = Waste::new(1.0);
        w.tick(1.0);
        assert!(w.is_fully_wasted());
    }

    #[test]
    fn is_fully_wasted_false_below_one() {
        let mut w = Waste::new(0.1);
        w.tick(1.0); // 0.1
        assert!(!w.is_fully_wasted());
    }

    #[test]
    fn is_fully_wasted_false_when_disabled() {
        let mut w = Waste::new(1.0);
        w.tick(1.0); // fully wasted
        w.enabled = false;
        assert!(!w.is_fully_wasted());
    }

    #[test]
    fn decay_rate_clamped_at_one() {
        let w = Waste::new(2.0);
        assert!((w.decay_rate - 1.0).abs() < 1e-5);
    }

    #[test]
    fn decay_rate_clamped_at_zero() {
        let w = Waste::new(-0.5);
        assert_eq!(w.decay_rate, 0.0);
    }

    #[test]
    fn cleanse_restores_after_full_waste() {
        let mut w = Waste::new(1.0);
        w.tick(2.0); // fully wasted
        w.cleanse(0.5);
        assert!((w.accumulated - 0.5).abs() < 1e-5);
        assert!(!w.is_fully_wasted());
    }

    #[test]
    fn multiple_ticks_accumulate() {
        let mut w = Waste::new(0.1);
        w.tick(1.0); // 0.1
        w.tick(1.0); // 0.2
        w.tick(1.0); // 0.3
        assert!((w.accumulated - 0.3).abs() < 1e-4);
    }

    #[test]
    fn effective_max_hp_at_quarter_accumulated() {
        let mut w = Waste::new(0.25);
        w.tick(1.0); // 0.25
                     // 200 * 0.75 = 150
        assert!((w.effective_max_hp(200.0) - 150.0).abs() < 1e-3);
    }
}
