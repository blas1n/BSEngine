use bevy_ecs::prelude::Component;

/// Single-charge elemental attack augment. A call to `imbue(bonus)` loads
/// the entity's next attack with `bonus_damage` extra damage. The first call
/// to `consume()` after charging drains the charge and returns the stored
/// bonus; subsequent calls without a recharge return 0.0.
///
/// `imbue(bonus_damage)` loads a charge and stores the bonus. Fires
/// `just_charged` on the inactive → charged transition. If already charged,
/// the existing charge is replaced with the new bonus (no just_charged
/// re-fire). No-op when disabled.
///
/// `consume() -> f32` drains an active charge: clears `charged`, fires
/// `just_consumed`, and returns `bonus_damage`. Returns 0.0 when not charged
/// or disabled.
///
/// `tick()` clears `just_charged` and `just_consumed` each frame.
///
/// `is_charged()` returns `charged && enabled`.
///
/// Distinct from `Empower` (continuous stat multiplier, no per-hit
/// consumption), `Buff` (general temporary stat layer with a duration),
/// `Amplify` (percentage damage scaling applied every hit), and `Galvanize`
/// (charge bar that releases a burst): Imbue is a **consumed single-charge
/// elemental augment** — it loads exactly one attack with bonus damage and
/// is spent on use, rewarding precise timing over passive stacking.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Imbue {
    pub charged: bool,
    /// Bonus damage stored by the last `imbue()` call. Clamped ≥ 0.0.
    pub bonus_damage: f32,
    pub just_charged: bool,
    pub just_consumed: bool,
    pub enabled: bool,
}

impl Imbue {
    pub fn new() -> Self {
        Self {
            charged: false,
            bonus_damage: 0.0,
            just_charged: false,
            just_consumed: false,
            enabled: true,
        }
    }

    /// Load a charge with the given bonus damage. Fires `just_charged` on the
    /// first activation from uncharged. Replaces an existing charge silently.
    /// No-op when disabled.
    pub fn imbue(&mut self, bonus_damage: f32) {
        if !self.enabled {
            return;
        }
        if !self.charged {
            self.just_charged = true;
        }
        self.charged = true;
        self.bonus_damage = bonus_damage.max(0.0);
    }

    /// Drain the charge and return the stored bonus. Fires `just_consumed` and
    /// returns `bonus_damage` when charged. Returns 0.0 when not charged or
    /// disabled.
    pub fn consume(&mut self) -> f32 {
        if !self.enabled || !self.charged {
            return 0.0;
        }
        self.charged = false;
        self.just_consumed = true;
        self.bonus_damage
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_charged = false;
        self.just_consumed = false;
    }

    /// `true` when a charge is loaded and the component is enabled.
    pub fn is_charged(&self) -> bool {
        self.charged && self.enabled
    }
}

impl Default for Imbue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_uncharged() {
        let i = Imbue::new();
        assert!(!i.charged);
        assert!(!i.is_charged());
    }

    #[test]
    fn imbue_sets_charged() {
        let mut i = Imbue::new();
        i.imbue(25.0);
        assert!(i.charged);
        assert!(i.is_charged());
    }

    #[test]
    fn imbue_stores_bonus_damage() {
        let mut i = Imbue::new();
        i.imbue(42.5);
        assert!((i.bonus_damage - 42.5).abs() < 1e-5);
    }

    #[test]
    fn imbue_fires_just_charged_on_first_activation() {
        let mut i = Imbue::new();
        i.imbue(10.0);
        assert!(i.just_charged);
    }

    #[test]
    fn imbue_no_just_charged_when_already_charged() {
        let mut i = Imbue::new();
        i.imbue(10.0);
        i.tick();
        i.imbue(20.0); // re-imbue while charged
        assert!(!i.just_charged);
    }

    #[test]
    fn imbue_replaces_bonus_when_already_charged() {
        let mut i = Imbue::new();
        i.imbue(10.0);
        i.tick();
        i.imbue(30.0);
        assert!((i.bonus_damage - 30.0).abs() < 1e-5);
    }

    #[test]
    fn imbue_clamps_bonus_to_zero() {
        let mut i = Imbue::new();
        i.imbue(-5.0);
        assert_eq!(i.bonus_damage, 0.0);
    }

    #[test]
    fn imbue_no_op_when_disabled() {
        let mut i = Imbue::new();
        i.enabled = false;
        i.imbue(50.0);
        assert!(!i.charged);
    }

    #[test]
    fn consume_returns_bonus_damage() {
        let mut i = Imbue::new();
        i.imbue(33.0);
        let bonus = i.consume();
        assert!((bonus - 33.0).abs() < 1e-5);
    }

    #[test]
    fn consume_clears_charge() {
        let mut i = Imbue::new();
        i.imbue(10.0);
        i.consume();
        assert!(!i.charged);
        assert!(!i.is_charged());
    }

    #[test]
    fn consume_fires_just_consumed() {
        let mut i = Imbue::new();
        i.imbue(10.0);
        i.consume();
        assert!(i.just_consumed);
    }

    #[test]
    fn consume_returns_zero_when_not_charged() {
        let mut i = Imbue::new();
        let bonus = i.consume();
        assert_eq!(bonus, 0.0);
    }

    #[test]
    fn consume_returns_zero_when_disabled() {
        let mut i = Imbue::new();
        i.imbue(50.0);
        i.enabled = false;
        let bonus = i.consume();
        assert_eq!(bonus, 0.0);
    }

    #[test]
    fn consume_does_not_clear_charge_when_disabled() {
        let mut i = Imbue::new();
        i.imbue(50.0);
        i.enabled = false;
        i.consume();
        assert!(i.charged); // still charged
    }

    #[test]
    fn double_consume_returns_zero_second_time() {
        let mut i = Imbue::new();
        i.imbue(20.0);
        i.consume();
        let second = i.consume();
        assert_eq!(second, 0.0);
    }

    #[test]
    fn tick_clears_just_charged() {
        let mut i = Imbue::new();
        i.imbue(10.0);
        i.tick();
        assert!(!i.just_charged);
    }

    #[test]
    fn tick_clears_just_consumed() {
        let mut i = Imbue::new();
        i.imbue(10.0);
        i.consume();
        i.tick();
        assert!(!i.just_consumed);
    }

    #[test]
    fn is_charged_false_when_disabled() {
        let mut i = Imbue::new();
        i.imbue(10.0);
        i.enabled = false;
        assert!(!i.is_charged());
    }

    #[test]
    fn charge_then_consume_cycle_repeatable() {
        let mut i = Imbue::new();
        i.imbue(15.0);
        let b1 = i.consume();
        i.tick();
        i.imbue(25.0);
        let b2 = i.consume();
        assert!((b1 - 15.0).abs() < 1e-5);
        assert!((b2 - 25.0).abs() < 1e-5);
    }

    #[test]
    fn just_charged_fires_again_after_cycle() {
        let mut i = Imbue::new();
        i.imbue(10.0);
        i.tick();
        i.consume();
        i.tick();
        i.imbue(20.0); // re-charged from uncharged
        assert!(i.just_charged);
    }

    #[test]
    fn zero_bonus_charge_still_fires_events() {
        let mut i = Imbue::new();
        i.imbue(0.0);
        assert!(i.just_charged);
        assert!(i.charged);
        let b = i.consume();
        assert_eq!(b, 0.0);
        assert!(i.just_consumed);
    }
}
