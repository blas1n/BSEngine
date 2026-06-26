use bevy_ecs::prelude::Component;

/// Damage type that a particular armor layer covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorType {
    Physical,
    Magic,
    Fire,
    Cold,
    Lightning,
    Poison,
    /// Reduces all incoming damage types.
    Universal,
}

/// A single armor layer (one entity can carry multiple via separate components or a `Vec`).
///
/// The damage system reads `reduction_for(damage_type)` to compute final damage:
///   `final = max(0, raw - flat_reduction) * (1 - percent_reduction)`
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Armor {
    pub armor_type: ArmorType,
    /// Flat damage subtracted before percent reduction (non-negative).
    pub flat_reduction: f32,
    /// Fractional reduction applied after flat reduction [0, 1]. 0.25 = 25% reduction.
    pub percent_reduction: f32,
    /// Maximum damage this armor layer can absorb per hit. `None` = unlimited.
    pub absorption_cap: Option<f32>,
    /// Current durability. When ≤ 0 the armor is broken and provides no protection.
    pub durability: f32,
    /// Maximum durability (for repair calculation).
    pub max_durability: f32,
    /// Fraction of incoming damage dealt as durability loss.
    pub durability_damage_rate: f32,
    pub enabled: bool,
}

impl Armor {
    pub fn new(armor_type: ArmorType, flat_reduction: f32) -> Self {
        Self {
            armor_type,
            flat_reduction: flat_reduction.max(0.0),
            percent_reduction: 0.0,
            absorption_cap: None,
            durability: 100.0,
            max_durability: 100.0,
            durability_damage_rate: 0.0,
            enabled: true,
        }
    }

    pub fn physical(flat: f32) -> Self {
        Self::new(ArmorType::Physical, flat)
    }

    pub fn universal(flat: f32, percent: f32) -> Self {
        Self::new(ArmorType::Universal, flat).with_percent_reduction(percent)
    }

    pub fn with_percent_reduction(mut self, percent: f32) -> Self {
        self.percent_reduction = percent.clamp(0.0, 1.0);
        self
    }

    pub fn with_absorption_cap(mut self, cap: f32) -> Self {
        self.absorption_cap = Some(cap.max(0.0));
        self
    }

    pub fn with_durability(mut self, durability: f32) -> Self {
        self.durability = durability.max(0.0);
        self.max_durability = self.durability;
        self
    }

    pub fn with_durability_damage_rate(mut self, rate: f32) -> Self {
        self.durability_damage_rate = rate.clamp(0.0, 1.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns true if this armor layer applies to `incoming_type`.
    pub fn covers(&self, incoming_type: ArmorType) -> bool {
        self.armor_type == ArmorType::Universal || self.armor_type == incoming_type
    }

    /// Amount of `raw_damage` absorbed by this layer (before durability check).
    /// Returns 0 if broken, disabled, or wrong type.
    pub fn reduction_for(&self, raw_damage: f32, incoming_type: ArmorType) -> f32 {
        if !self.enabled || self.durability <= 0.0 || !self.covers(incoming_type) {
            return 0.0;
        }
        let after_flat = (raw_damage - self.flat_reduction).max(0.0);
        let reduced = raw_damage - after_flat * (1.0 - self.percent_reduction);
        match self.absorption_cap {
            Some(cap) => reduced.min(cap),
            None => reduced,
        }
    }

    /// Apply `raw_damage` and update durability. Returns the amount absorbed.
    pub fn absorb(&mut self, raw_damage: f32, incoming_type: ArmorType) -> f32 {
        let absorbed = self.reduction_for(raw_damage, incoming_type);
        if absorbed > 0.0 && self.durability_damage_rate > 0.0 {
            self.durability = (self.durability - raw_damage * self.durability_damage_rate).max(0.0);
        }
        absorbed
    }

    pub fn repair(&mut self, amount: f32) {
        self.durability = (self.durability + amount.max(0.0)).min(self.max_durability);
    }

    pub fn is_broken(&self) -> bool {
        self.durability <= 0.0
    }

    pub fn durability_fraction(&self) -> f32 {
        if self.max_durability <= 0.0 {
            return 0.0;
        }
        (self.durability / self.max_durability).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_reduction_applied() {
        let a = Armor::physical(20.0);
        assert!((a.reduction_for(100.0, ArmorType::Physical) - 20.0).abs() < 0.001);
    }

    #[test]
    fn percent_reduction_applied_after_flat() {
        let a = Armor::universal(10.0, 0.25);
        let absorbed = a.reduction_for(100.0, ArmorType::Physical);
        // after flat: 90; 25% of 90 = 22.5; total = 10 + 22.5 = 32.5
        assert!((absorbed - 32.5).abs() < 0.001);
    }

    #[test]
    fn wrong_type_provides_no_reduction() {
        let a = Armor::physical(50.0);
        assert!((a.reduction_for(100.0, ArmorType::Magic)).abs() < 0.001);
    }

    #[test]
    fn broken_armor_provides_no_reduction() {
        let mut a = Armor::physical(50.0)
            .with_durability(10.0)
            .with_durability_damage_rate(1.0);
        a.absorb(20.0, ArmorType::Physical);
        assert!(a.is_broken());
        assert!((a.reduction_for(100.0, ArmorType::Physical)).abs() < 0.001);
    }

    #[test]
    fn absorption_cap_limits_reduction() {
        let a = Armor::physical(100.0).with_absorption_cap(30.0);
        assert!((a.reduction_for(200.0, ArmorType::Physical) - 30.0).abs() < 0.001);
    }
}
