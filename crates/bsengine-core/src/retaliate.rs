use bevy_ecs::prelude::Component;

/// Counter-attack trait: accumulates charges when the entity is hit, each granting
/// one retaliatory strike with a bonus damage multiplier.
///
/// When the entity takes a hit, the combat system calls `charge()` to bank a
/// retaliation. Before applying outgoing damage, call `consume()` — if charges are
/// available it returns the bonus multiplier and decrements the count. Call
/// `reset_events()` each frame to clear `just_charged`/`just_consumed` flags.
///
/// Distinct from `Thorns` (automatic reflected damage) and `Rage` (stacking stat
/// boost on hit): `Retaliate` grants controlled, explicit bonus damage on the next
/// outgoing attack only.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Retaliate {
    /// Bonus multiplier applied when consuming a retaliation charge.
    /// e.g. 1.5 = 50% bonus damage on the retaliatory hit.
    pub multiplier: f32,
    /// Maximum charges that can be banked at once.
    pub max_charges: u32,
    /// Currently available retaliation charges.
    pub charges: u32,
    /// Set on the frame a new charge is banked via `charge()`.
    pub just_charged: bool,
    /// Set on the frame a charge is consumed via `consume()`.
    pub just_consumed: bool,
    pub enabled: bool,
}

impl Retaliate {
    pub fn new(multiplier: f32) -> Self {
        Self {
            multiplier: multiplier.max(1.0),
            max_charges: 1,
            charges: 0,
            just_charged: false,
            just_consumed: false,
            enabled: true,
        }
    }

    pub fn with_max_charges(mut self, max: u32) -> Self {
        self.max_charges = max.max(1);
        self
    }

    /// Bank one retaliation charge (call when the entity takes a hit).
    /// No-op if disabled or at max capacity.
    pub fn charge(&mut self) {
        if !self.enabled || self.charges >= self.max_charges {
            return;
        }
        self.charges += 1;
        self.just_charged = true;
    }

    /// Consume one charge and return the bonus multiplier, or `None` if empty.
    pub fn consume(&mut self) -> Option<f32> {
        if !self.enabled || self.charges == 0 {
            return None;
        }
        self.charges -= 1;
        self.just_consumed = true;
        Some(self.multiplier)
    }

    /// Clear single-frame event flags. Call once per frame at frame start.
    pub fn reset_events(&mut self) {
        self.just_charged = false;
        self.just_consumed = false;
    }

    pub fn has_charges(&self) -> bool {
        self.charges > 0
    }

    /// Drain all pending charges.
    pub fn clear(&mut self) {
        self.charges = 0;
    }
}

impl Default for Retaliate {
    fn default() -> Self {
        Self::new(1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_banks_a_charge() {
        let mut r = Retaliate::new(2.0);
        r.charge();
        assert_eq!(r.charges, 1);
        assert!(r.just_charged);
    }

    #[test]
    fn charge_capped_at_max() {
        let mut r = Retaliate::new(2.0).with_max_charges(2);
        r.charge();
        r.charge();
        r.charge(); // exceeds max
        assert_eq!(r.charges, 2);
    }

    #[test]
    fn consume_returns_multiplier_and_decrements() {
        let mut r = Retaliate::new(1.5);
        r.charge();
        let m = r.consume();
        assert!(m.is_some());
        assert!((m.unwrap() - 1.5).abs() < 1e-5);
        assert_eq!(r.charges, 0);
        assert!(r.just_consumed);
    }

    #[test]
    fn consume_returns_none_when_empty() {
        let mut r = Retaliate::new(1.5);
        assert!(r.consume().is_none());
        assert!(!r.just_consumed);
    }

    #[test]
    fn reset_events_clears_flags() {
        let mut r = Retaliate::new(1.5);
        r.charge();
        r.reset_events();
        assert!(!r.just_charged);
        assert!(!r.just_consumed);
    }

    #[test]
    fn has_charges_true_after_charge() {
        let mut r = Retaliate::new(1.5);
        r.charge();
        assert!(r.has_charges());
    }

    #[test]
    fn clear_drains_all_charges() {
        let mut r = Retaliate::new(1.5).with_max_charges(3);
        r.charge();
        r.charge();
        r.clear();
        assert!(!r.has_charges());
    }

    #[test]
    fn multiplier_clamped_to_one() {
        let r = Retaliate::new(0.5); // < 1.0 → clamped to 1.0
        assert!((r.multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_charge_no_op() {
        let mut r = Retaliate::new(1.5);
        r.enabled = false;
        r.charge();
        assert_eq!(r.charges, 0);
    }

    #[test]
    fn disabled_consume_returns_none() {
        let mut r = Retaliate::new(1.5);
        r.charge();
        r.enabled = false;
        assert!(r.consume().is_none());
        assert_eq!(r.charges, 1); // charge not consumed
    }
}
