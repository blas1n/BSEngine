use bevy_ecs::prelude::Component;

/// Classifies what kind of damage a source or receiver deals/resists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DamageType {
    #[default]
    Physical,
    Fire,
    Ice,
    Lightning,
    Poison,
    /// Engine-defined catchall for game-specific damage types.
    Custom(u32),
}

/// Describes a pending or recent damage event carried on an entity.
/// The combat system reads this each frame to apply damage to `Health`.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Damage {
    /// Raw damage amount before resistances are applied. Always >= 0.
    pub amount: f32,
    pub damage_type: DamageType,
    /// Multiplier applied after resistances (e.g. critical hit = 2.0). Always >= 0.
    pub multiplier: f32,
    /// When `true` the combat system ignores the target's resistances/shields entirely.
    pub piercing: bool,
}

impl Damage {
    pub fn new(amount: f32, damage_type: DamageType) -> Self {
        Self {
            amount: amount.max(0.0),
            damage_type,
            multiplier: 1.0,
            piercing: false,
        }
    }

    pub fn physical(amount: f32) -> Self {
        Self::new(amount, DamageType::Physical)
    }

    pub fn fire(amount: f32) -> Self {
        Self::new(amount, DamageType::Fire)
    }

    pub fn with_multiplier(mut self, mul: f32) -> Self {
        self.multiplier = mul.max(0.0);
        self
    }

    pub fn piercing(mut self) -> Self {
        self.piercing = true;
        self
    }

    /// Final effective damage (amount × multiplier).
    pub fn effective(&self) -> f32 {
        self.amount * self.multiplier
    }
}

impl Default for Damage {
    fn default() -> Self {
        Self::physical(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_defaults() {
        let d = Damage::default();
        assert_eq!(d.amount, 0.0);
        assert_eq!(d.damage_type, DamageType::Physical);
        assert!((d.multiplier - 1.0).abs() < 0.001);
        assert!(!d.piercing);
    }

    #[test]
    fn effective_damage_with_multiplier() {
        let d = Damage::physical(10.0).with_multiplier(2.0);
        assert!((d.effective() - 20.0).abs() < 0.001);
    }

    #[test]
    fn amount_clamped_to_zero() {
        let d = Damage::physical(-5.0);
        assert_eq!(d.amount, 0.0);
    }

    #[test]
    fn multiplier_clamped_to_zero() {
        let d = Damage::fire(10.0).with_multiplier(-1.0);
        assert_eq!(d.multiplier, 0.0);
        assert_eq!(d.effective(), 0.0);
    }

    #[test]
    fn piercing_flag() {
        let d = Damage::physical(5.0).piercing();
        assert!(d.piercing);
    }
}
