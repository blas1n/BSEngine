use bevy_ecs::prelude::Component;

/// Heavy-character archetype: entity has significant physical mass that
/// improves knockback resistance and slam/charge impact at the cost of
/// movement speed.
///
/// `bulk_level` [0.0, 1.0] is the canonical weight parameter. At 0.0 the
/// entity behaves like a normal-mass entity; at 1.0 it is maximally bulky.
///
/// - `knockback_resist(incoming)` — returns `(incoming * (1 - bulk_level)).max(0.0)`;
///   the net knockback force after mass absorbs it.
/// - `impact_bonus(base)` — returns `base * (1 + bulk_level)` — heavier
///   bodies hit harder on slam/charge attacks.
/// - `speed_penalty(base)` — returns `(base * (1 - 0.5 * bulk_level)).max(0.0)`;
///   up to 50% speed reduction at max bulk.
///
/// These methods all return their unmodified inputs when disabled.
///
/// Distinct from `Armor` (flat damage reduction), `Shield` (HP-limited
/// absorption layer), `Momentum` (accumulated velocity carry-over), and
/// the physics `Mass` component (simulation inertia): Bulk is a
/// **gameplay-archetype trade-off** — a deliberate design slider between
/// tankiness/impact and mobility, independent of physics mass.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Bulk {
    /// Weight level [0.0 = normal, 1.0 = maximum bulk]. Clamped to [0.0, 1.0].
    pub bulk_level: f32,
    pub enabled: bool,
}

impl Bulk {
    pub fn new(bulk_level: f32) -> Self {
        Self {
            bulk_level: bulk_level.clamp(0.0, 1.0),
            enabled: true,
        }
    }

    /// Net knockback force after mass absorption.
    /// Returns `(incoming * (1 - bulk_level)).max(0.0)` when enabled;
    /// returns `incoming` otherwise.
    pub fn knockback_resist(&self, incoming: f32) -> f32 {
        if self.enabled {
            (incoming * (1.0 - self.bulk_level)).max(0.0)
        } else {
            incoming
        }
    }

    /// Outgoing slam/charge impact bonus.
    /// Returns `base * (1 + bulk_level)` when enabled; returns `base` otherwise.
    pub fn impact_bonus(&self, base: f32) -> f32 {
        if self.enabled {
            base * (1.0 + self.bulk_level)
        } else {
            base
        }
    }

    /// Effective movement speed after bulk penalty.
    /// Returns `(base * (1 - 0.5 * bulk_level)).max(0.0)` when enabled;
    /// returns `base` otherwise.
    pub fn speed_penalty(&self, base: f32) -> f32 {
        if self.enabled {
            (base * (1.0 - 0.5 * self.bulk_level)).max(0.0)
        } else {
            base
        }
    }

    /// `true` when the entity has any bulk effect active.
    pub fn is_bulky(&self) -> bool {
        self.enabled && self.bulk_level > 0.0
    }
}

impl Default for Bulk {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_clamps_bulk_level() {
        assert_eq!(Bulk::new(2.0).bulk_level, 1.0);
        assert_eq!(Bulk::new(-0.5).bulk_level, 0.0);
    }

    #[test]
    fn knockback_resist_at_zero_bulk() {
        let b = Bulk::new(0.0);
        assert!((b.knockback_resist(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn knockback_resist_at_full_bulk() {
        let b = Bulk::new(1.0);
        // 100 * (1 - 1.0) = 0
        assert!(b.knockback_resist(100.0).abs() < 1e-5);
    }

    #[test]
    fn knockback_resist_at_half_bulk() {
        let b = Bulk::new(0.5);
        // 100 * (1 - 0.5) = 50
        assert!((b.knockback_resist(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn knockback_resist_floored_at_zero() {
        let b = Bulk::new(1.0);
        assert!(b.knockback_resist(-100.0).abs() < 1e-5);
    }

    #[test]
    fn knockback_resist_base_when_disabled() {
        let mut b = Bulk::new(1.0);
        b.enabled = false;
        assert!((b.knockback_resist(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn impact_bonus_at_zero_bulk() {
        let b = Bulk::new(0.0);
        // 100 * (1 + 0) = 100
        assert!((b.impact_bonus(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn impact_bonus_at_full_bulk() {
        let b = Bulk::new(1.0);
        // 100 * (1 + 1.0) = 200
        assert!((b.impact_bonus(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn impact_bonus_at_half_bulk() {
        let b = Bulk::new(0.5);
        // 100 * (1 + 0.5) = 150
        assert!((b.impact_bonus(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn impact_bonus_base_when_disabled() {
        let mut b = Bulk::new(1.0);
        b.enabled = false;
        assert!((b.impact_bonus(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn speed_penalty_at_zero_bulk() {
        let b = Bulk::new(0.0);
        // 100 * (1 - 0) = 100
        assert!((b.speed_penalty(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn speed_penalty_at_full_bulk() {
        let b = Bulk::new(1.0);
        // 100 * (1 - 0.5) = 50
        assert!((b.speed_penalty(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn speed_penalty_at_half_bulk() {
        let b = Bulk::new(0.5);
        // 100 * (1 - 0.25) = 75
        assert!((b.speed_penalty(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn speed_penalty_floored_at_zero() {
        let b = Bulk::new(1.0);
        assert!(b.speed_penalty(-100.0).abs() < 1e-5);
    }

    #[test]
    fn speed_penalty_base_when_disabled() {
        let mut b = Bulk::new(1.0);
        b.enabled = false;
        assert!((b.speed_penalty(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn is_bulky_true_when_enabled_and_nonzero() {
        let b = Bulk::new(0.5);
        assert!(b.is_bulky());
    }

    #[test]
    fn is_bulky_false_at_zero_level() {
        let b = Bulk::new(0.0);
        assert!(!b.is_bulky());
    }

    #[test]
    fn is_bulky_false_when_disabled() {
        let mut b = Bulk::new(1.0);
        b.enabled = false;
        assert!(!b.is_bulky());
    }

    #[test]
    fn default_bulk_level_is_half() {
        let b = Bulk::default();
        assert!((b.bulk_level - 0.5).abs() < 1e-5);
    }
}
