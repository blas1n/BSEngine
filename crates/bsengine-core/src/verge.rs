use bevy_ecs::prelude::Component;

/// Ultimate-charge meter: accumulates `charge` [0.0, 1.0] as external events
/// (damage dealt, kills, hits received) are fed into it. When `charge` reaches
/// 1.0, `just_peaked` fires once and the entity is considered "at the verge"
/// of unleashing a special ability.
///
/// The combat system calls `feed(amount)` each time a qualifying event
/// occurs — e.g. `feed(damage_dealt)` or `feed(1.0)` per kill. The amount
/// is scaled by `charge_rate` before being added to `charge`.
///
/// `consume()` resets `charge` to 0.0 (use this when the ability fires).
///
/// `tick()` clears `just_peaked` each frame.
///
/// `is_ready()` returns `charge >= 1.0 && enabled`.
///
/// Distinct from `Fervor` (on-kill stack accumulator that scales damage),
/// `Rage` (discrete triggered anger state), `Galvanize` (charge
/// specifically from taking damage), and `Overload` (accumulate until a
/// dangerous threshold): Verge is a **generic ultimate-charge meter** —
/// it can be fed by any source (damage, kills, heals, time) and peaks once
/// per consume/reset cycle, designed for limit-break or rage-strike abilities.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Verge {
    /// Current charge level [0.0, 1.0].
    pub charge: f32,
    /// Charge gained per unit of input fed. Clamped ≥ 0.0.
    /// e.g. `0.01` means 100 damage points fill the meter.
    pub charge_rate: f32,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Verge {
    pub fn new(charge_rate: f32) -> Self {
        Self {
            charge: 0.0,
            charge_rate: charge_rate.max(0.0),
            just_peaked: false,
            enabled: true,
        }
    }

    /// Add `amount * charge_rate` to `charge`, clamping at 1.0. Fires
    /// `just_peaked` on the transition that first reaches 1.0. No-op when
    /// disabled or `amount ≤ 0`.
    pub fn feed(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_ready = self.is_ready();
        self.charge = (self.charge + amount * self.charge_rate).min(1.0);
        if !was_ready && self.is_ready() {
            self.just_peaked = true;
        }
    }

    /// Reset `charge` to 0.0. Silent — fires no event. No-op when disabled.
    pub fn consume(&mut self) {
        if !self.enabled {
            return;
        }
        self.charge = 0.0;
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_peaked = false;
    }

    /// `true` when charge has reached 1.0 and the component is enabled.
    pub fn is_ready(&self) -> bool {
        self.charge >= 1.0 && self.enabled
    }
}

impl Default for Verge {
    fn default() -> Self {
        Self::new(0.01)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let v = Verge::new(0.01);
        assert_eq!(v.charge, 0.0);
        assert!(!v.is_ready());
    }

    #[test]
    fn feed_increments_charge() {
        let mut v = Verge::new(0.01);
        v.feed(50.0); // 50 * 0.01 = 0.5
        assert!((v.charge - 0.5).abs() < 1e-5);
    }

    #[test]
    fn feed_caps_at_one() {
        let mut v = Verge::new(0.01);
        v.feed(200.0); // 200 * 0.01 = 2.0 → clamped to 1.0
        assert!((v.charge - 1.0).abs() < 1e-5);
    }

    #[test]
    fn feed_fires_just_peaked_at_threshold() {
        let mut v = Verge::new(0.01);
        v.feed(100.0);
        assert!(v.just_peaked);
        assert!(v.is_ready());
    }

    #[test]
    fn feed_no_just_peaked_when_already_ready() {
        let mut v = Verge::new(0.01);
        v.feed(100.0); // peaks
        v.tick();
        v.feed(1.0); // already at 1.0
        assert!(!v.just_peaked);
    }

    #[test]
    fn feed_fires_just_peaked_on_exact_amount() {
        let mut v = Verge::new(0.1);
        v.feed(9.0); // 0.9
        assert!(!v.just_peaked);
        v.feed(1.0); // 0.9 + 0.1 = 1.0
        assert!(v.just_peaked);
    }

    #[test]
    fn feed_no_op_when_disabled() {
        let mut v = Verge::new(0.01);
        v.enabled = false;
        v.feed(100.0);
        assert_eq!(v.charge, 0.0);
    }

    #[test]
    fn feed_no_op_at_zero_or_negative() {
        let mut v = Verge::new(0.01);
        v.feed(0.0);
        v.feed(-10.0);
        assert_eq!(v.charge, 0.0);
    }

    #[test]
    fn consume_resets_charge() {
        let mut v = Verge::new(0.01);
        v.feed(100.0);
        v.consume();
        assert_eq!(v.charge, 0.0);
        assert!(!v.is_ready());
    }

    #[test]
    fn consume_no_op_when_disabled() {
        let mut v = Verge::new(0.01);
        v.charge = 1.0;
        v.enabled = false;
        v.consume();
        assert!((v.charge - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut v = Verge::new(0.01);
        v.feed(100.0);
        v.tick();
        assert!(!v.just_peaked);
    }

    #[test]
    fn is_ready_false_when_disabled() {
        let mut v = Verge::new(0.01);
        v.charge = 1.0;
        v.enabled = false;
        assert!(!v.is_ready());
    }

    #[test]
    fn refill_after_consume_fires_just_peaked_again() {
        let mut v = Verge::new(0.01);
        v.feed(100.0); // peak
        v.tick();
        v.consume(); // reset
        v.feed(100.0); // peak again
        assert!(v.just_peaked);
    }

    #[test]
    fn charge_rate_clamped_non_negative() {
        let v = Verge::new(-0.5);
        assert_eq!(v.charge_rate, 0.0);
    }

    #[test]
    fn accumulates_across_multiple_feeds() {
        let mut v = Verge::new(0.1);
        v.feed(3.0); // 0.3
        v.feed(3.0); // 0.6
        v.feed(3.0); // 0.9
        assert!((v.charge - 0.9).abs() < 1e-4);
        assert!(!v.is_ready());
        v.feed(1.0); // 1.0
        assert!(v.is_ready());
    }
}
