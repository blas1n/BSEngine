use bevy_ecs::prelude::Component;

/// Armor-penetration property: this entity's attacks bypass a portion of the
/// target's armor. Two independent layers are applied in sequence when the
/// damage system calls `effective_armor(armor)`:
///
/// 1. `flat_penetration` is subtracted first (floored at 0).
/// 2. `armor_penetration` fraction is then applied to the remainder.
///
/// So `effective_armor(armor) = (armor - flat_penetration).max(0) * (1 - armor_penetration)`.
///
/// `effective_armor` returns `armor` unchanged when disabled or both values
/// are 0. The damage system passes the target's armor value here; the result
/// is then used in its normal damage formula as the effective armor value.
///
/// Distinct from `Pierce` (projectile passes through targets geometrically),
/// `Expose` (reduces the target's defense via a status effect), and
/// `Sunder` (breaks shields entirely): Shear is a **permanent attacker
/// property** — it passively reduces how much protection armor grants on
/// every outgoing hit without altering the target's component state.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Shear {
    /// Fraction of target armor ignored after flat reduction. Clamped [0.0, 1.0].
    pub armor_penetration: f32,
    /// Flat armor value subtracted before fractional penetration. Clamped ≥ 0.0.
    pub flat_penetration: f32,
    pub enabled: bool,
}

impl Shear {
    pub fn new(armor_penetration: f32, flat_penetration: f32) -> Self {
        Self {
            armor_penetration: armor_penetration.clamp(0.0, 1.0),
            flat_penetration: flat_penetration.max(0.0),
            enabled: true,
        }
    }

    /// Returns the effective armor value after applying penetration.
    /// Applies flat subtraction first, then fractional reduction.
    /// Returns `armor` unchanged when disabled.
    pub fn effective_armor(&self, armor: f32) -> f32 {
        if !self.enabled {
            return armor;
        }
        let after_flat = (armor - self.flat_penetration).max(0.0);
        after_flat * (1.0 - self.armor_penetration)
    }

    /// Convenience helper: returns the additional damage dealt due to
    /// armor penetration when the normal damage formula subtracts armor
    /// from raw damage. Positive when shear reduces armor below full value.
    /// `raw_damage` is the pre-armor damage value; `armor` is the target's
    /// full armor value. Returns 0.0 when disabled.
    pub fn penetration_gain(&self, raw_damage: f32, armor: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let normal_after_armor = (raw_damage - armor).max(0.0);
        let shear_after_armor = (raw_damage - self.effective_armor(armor)).max(0.0);
        (shear_after_armor - normal_after_armor).max(0.0)
    }
}

impl Default for Shear {
    fn default() -> Self {
        Self::new(0.3, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_armor_fractional_penetration_only() {
        let s = Shear::new(0.5, 0.0);
        // 100 * (1 - 0.5) = 50
        assert!((s.effective_armor(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_armor_flat_penetration_only() {
        let s = Shear::new(0.0, 30.0);
        // (100 - 30) * 1.0 = 70
        assert!((s.effective_armor(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_armor_both_layers() {
        let s = Shear::new(0.5, 30.0);
        // (100 - 30) * (1 - 0.5) = 70 * 0.5 = 35
        assert!((s.effective_armor(100.0) - 35.0).abs() < 1e-3);
    }

    #[test]
    fn effective_armor_flat_floors_at_zero() {
        let s = Shear::new(0.0, 200.0);
        // (100 - 200).max(0) * 1.0 = 0
        assert!((s.effective_armor(100.0)).abs() < 1e-5);
    }

    #[test]
    fn effective_armor_full_penetration() {
        let s = Shear::new(1.0, 0.0);
        assert!((s.effective_armor(100.0)).abs() < 1e-5);
    }

    #[test]
    fn effective_armor_zero_penetration_returns_full() {
        let s = Shear::new(0.0, 0.0);
        assert!((s.effective_armor(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_returns_armor_unchanged() {
        let mut s = Shear::new(0.5, 30.0);
        s.enabled = false;
        assert!((s.effective_armor(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn penetration_gain_positive_when_penetrating() {
        let s = Shear::new(0.5, 0.0);
        // raw=150, armor=100: normal = 50, shear armor=50, after shear = 100, gain = 50
        let gain = s.penetration_gain(150.0, 100.0);
        assert!(gain > 0.0);
        assert!((gain - 50.0).abs() < 1e-3);
    }

    #[test]
    fn penetration_gain_zero_when_raw_below_armor() {
        let s = Shear::new(0.5, 0.0);
        // raw=10, armor=100: normal damage=0, shear armor=50, damage = 0, gain = 0
        // (10 - 100).max(0) = 0; (10 - 50).max(0) = 0 → gain = 0
        assert!((s.penetration_gain(10.0, 100.0)).abs() < 1e-5);
    }

    #[test]
    fn penetration_gain_zero_when_disabled() {
        let mut s = Shear::new(0.5, 0.0);
        s.enabled = false;
        assert!((s.penetration_gain(150.0, 100.0)).abs() < 1e-5);
    }

    #[test]
    fn armor_penetration_clamped_above_one() {
        let s = Shear::new(2.0, 0.0);
        assert!((s.armor_penetration - 1.0).abs() < 1e-5);
    }

    #[test]
    fn armor_penetration_clamped_below_zero() {
        let s = Shear::new(-0.5, 0.0);
        assert!((s.armor_penetration).abs() < 1e-5);
    }

    #[test]
    fn flat_penetration_clamped_below_zero() {
        let s = Shear::new(0.0, -10.0);
        assert!((s.flat_penetration).abs() < 1e-5);
    }

    #[test]
    fn combined_penetration_stacks_correctly() {
        let s = Shear::new(0.25, 20.0);
        // (80 - 20) * (1 - 0.25) = 60 * 0.75 = 45
        assert!((s.effective_armor(80.0) - 45.0).abs() < 1e-3);
    }
}
