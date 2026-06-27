use bevy_ecs::prelude::Component;

/// Crowd-push and repeated-nudge destabilisation tracker. Unlike `Knockback`
/// (single large impulse that moves the entity), Jostle accumulates small
/// repeated displacements — shoulder-checks, terrain bumps, crowd pressure —
/// and fires `just_destabilized` when the total surpasses `threshold`. The
/// accumulated value decays over time when no new jostles arrive.
///
/// `jostle(amount)` adds `amount` to `accumulated` (capped at `threshold`).
/// Fires `just_destabilized` on the first transition from below- to
/// at-threshold. No-op when disabled or `amount ≤ 0`.
///
/// `tick(dt)` clears `just_destabilized` at the start, then decays
/// `accumulated` by `decay_rate * dt` (floored at 0.0). Does not decay
/// when `decay_rate` is 0.
///
/// `is_destabilized()` returns `accumulated >= threshold && enabled`.
///
/// `jostle_fraction()` returns `(accumulated / threshold).clamp(0, 1)`.
///
/// Distinct from `Knockback` (single large impulse that displaces the
/// entity immediately), `Stagger` (momentary animation/ability interrupt
/// from a heavy hit), `Flinch` (brief single-frame reaction to any hit),
/// and `Concuss` (confusion/control effect from blunt trauma): Jostle is
/// a **crowd-pressure accumulator** — it models sustained repeated nudges
/// building toward a destabilization threshold, not a single-event
/// displacement.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Jostle {
    /// Current accumulated jostle [0.0, threshold].
    pub accumulated: f32,
    /// Jostle level at which the entity is destabilized. Clamped ≥ 0.001.
    pub threshold: f32,
    /// Accumulated jostle lost per second when not being pushed. Clamped ≥ 0.0.
    pub decay_rate: f32,
    pub just_destabilized: bool,
    pub enabled: bool,
}

impl Jostle {
    pub fn new(threshold: f32, decay_rate: f32) -> Self {
        Self {
            accumulated: 0.0,
            threshold: threshold.max(0.001),
            decay_rate: decay_rate.max(0.0),
            just_destabilized: false,
            enabled: true,
        }
    }

    /// Add a jostle impulse. Caps `accumulated` at `threshold`. Fires
    /// `just_destabilized` on the first transition to threshold. No-op when
    /// disabled or `amount ≤ 0`.
    pub fn jostle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = !self.is_destabilized();
        self.accumulated = (self.accumulated + amount).min(self.threshold);
        if was_below && self.is_destabilized() {
            self.just_destabilized = true;
        }
    }

    /// Clear one-frame flags, then decay `accumulated` by `decay_rate * dt`.
    /// `accumulated` is floored at 0.0.
    pub fn tick(&mut self, dt: f32) {
        self.just_destabilized = false;

        if self.decay_rate > 0.0 && self.accumulated > 0.0 {
            self.accumulated = (self.accumulated - self.decay_rate * dt).max(0.0);
        }
    }

    /// `true` when accumulated jostle has reached `threshold` and the
    /// component is enabled.
    pub fn is_destabilized(&self) -> bool {
        self.accumulated >= self.threshold && self.enabled
    }

    /// Jostle fill fraction [0.0 = none, 1.0 = destabilized].
    pub fn jostle_fraction(&self) -> f32 {
        (self.accumulated / self.threshold).clamp(0.0, 1.0)
    }
}

impl Default for Jostle {
    fn default() -> Self {
        Self::new(3.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let j = Jostle::new(3.0, 0.5);
        assert_eq!(j.accumulated, 0.0);
        assert!(!j.is_destabilized());
    }

    #[test]
    fn jostle_adds_amount() {
        let mut j = Jostle::new(5.0, 0.0);
        j.jostle(2.0);
        assert!((j.accumulated - 2.0).abs() < 1e-5);
    }

    #[test]
    fn jostle_caps_at_threshold() {
        let mut j = Jostle::new(3.0, 0.0);
        j.jostle(10.0);
        assert!((j.accumulated - 3.0).abs() < 1e-5);
    }

    #[test]
    fn jostle_fires_just_destabilized_on_threshold_reach() {
        let mut j = Jostle::new(3.0, 0.0);
        j.jostle(2.0);
        assert!(!j.just_destabilized);
        j.jostle(1.0); // reaches 3.0
        assert!(j.just_destabilized);
        assert!(j.is_destabilized());
    }

    #[test]
    fn jostle_no_just_destabilized_when_already_at_threshold() {
        let mut j = Jostle::new(2.0, 0.0);
        j.jostle(2.0); // peaks
        j.tick(0.0);
        j.jostle(1.0); // still at threshold
        assert!(!j.just_destabilized);
    }

    #[test]
    fn jostle_no_op_on_zero_amount() {
        let mut j = Jostle::new(3.0, 0.0);
        j.jostle(0.0);
        assert_eq!(j.accumulated, 0.0);
    }

    #[test]
    fn jostle_no_op_on_negative_amount() {
        let mut j = Jostle::new(3.0, 0.0);
        j.jostle(-1.0);
        assert_eq!(j.accumulated, 0.0);
    }

    #[test]
    fn jostle_no_op_when_disabled() {
        let mut j = Jostle::new(3.0, 0.0);
        j.enabled = false;
        j.jostle(5.0);
        assert_eq!(j.accumulated, 0.0);
    }

    #[test]
    fn tick_decays_accumulated() {
        let mut j = Jostle::new(10.0, 2.0);
        j.jostle(6.0);
        j.tick(1.0); // 6 - 2 = 4
        assert!((j.accumulated - 4.0).abs() < 1e-5);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut j = Jostle::new(5.0, 10.0);
        j.jostle(2.0);
        j.tick(1.0); // would go negative
        assert_eq!(j.accumulated, 0.0);
    }

    #[test]
    fn tick_no_decay_when_rate_zero() {
        let mut j = Jostle::new(5.0, 0.0);
        j.jostle(3.0);
        j.tick(100.0);
        assert!((j.accumulated - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_destabilized() {
        let mut j = Jostle::new(2.0, 0.0);
        j.jostle(2.0);
        assert!(j.just_destabilized);
        j.tick(0.0);
        assert!(!j.just_destabilized);
    }

    #[test]
    fn is_destabilized_false_when_disabled() {
        let mut j = Jostle::new(2.0, 0.0);
        j.jostle(2.0);
        j.enabled = false;
        assert!(!j.is_destabilized());
    }

    #[test]
    fn jostle_fraction_at_zero() {
        let j = Jostle::new(4.0, 0.0);
        assert_eq!(j.jostle_fraction(), 0.0);
    }

    #[test]
    fn jostle_fraction_at_half() {
        let mut j = Jostle::new(4.0, 0.0);
        j.jostle(2.0);
        assert!((j.jostle_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn jostle_fraction_at_full() {
        let mut j = Jostle::new(4.0, 0.0);
        j.jostle(4.0);
        assert!((j.jostle_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn re_destabilizes_after_decay() {
        let mut j = Jostle::new(3.0, 5.0);
        j.jostle(3.0); // destabilize
        j.tick(0.0);
        j.tick(1.0); // decay to 0 (3 - 5*1 < 0 → 0)
        j.jostle(3.0); // destabilize again
        assert!(j.just_destabilized);
    }

    #[test]
    fn incremental_jostle_accumulates() {
        let mut j = Jostle::new(5.0, 0.0);
        j.jostle(1.0);
        j.tick(0.0);
        j.jostle(1.0);
        j.tick(0.0);
        j.jostle(1.0);
        assert!((j.accumulated - 3.0).abs() < 1e-5);
    }

    #[test]
    fn threshold_clamped_to_minimum() {
        let j = Jostle::new(0.0, 0.5);
        assert!(j.threshold >= 0.001);
    }

    #[test]
    fn decay_rate_clamped_non_negative() {
        let j = Jostle::new(3.0, -1.0);
        assert_eq!(j.decay_rate, 0.0);
    }

    #[test]
    fn just_destabilized_false_when_below_threshold() {
        let mut j = Jostle::new(5.0, 0.0);
        j.jostle(3.0);
        assert!(!j.just_destabilized);
    }
}
