use bevy_ecs::prelude::Component;

/// Selective immunity to damage types and status effects, identified by
/// caller-defined bitmasks.
///
/// Unlike `Invincible` (all-or-nothing), `Immune` lets the game define
/// immunity granularly. Each bit in `damage_type_mask` / `effect_type_mask`
/// corresponds to a game-specific enum value cast to a `u32` bit position
/// (e.g. `DamageType::Fire as u32` → bit 2 → `1 << 2`).
///
/// Query pattern:
/// ```
/// if immune.has_damage_immunity(1 << DamageType::Fire as u32) {
///     // skip fire damage
/// }
/// ```
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Immune {
    /// Bitmask of damage types this entity is immune to.
    pub damage_type_mask: u32,
    /// Bitmask of status-effect types this entity is immune to.
    pub effect_type_mask: u32,
    pub enabled: bool,
}

impl Immune {
    pub fn new() -> Self {
        Self {
            damage_type_mask: 0,
            effect_type_mask: 0,
            enabled: true,
        }
    }

    // --- Damage immunity ---

    pub fn add_damage_immunity(&mut self, bit: u32) {
        self.damage_type_mask |= bit;
    }

    pub fn remove_damage_immunity(&mut self, bit: u32) {
        self.damage_type_mask &= !bit;
    }

    /// Returns `true` when `enabled` and all bits in `mask` are set.
    pub fn has_damage_immunity(&self, mask: u32) -> bool {
        self.enabled && (self.damage_type_mask & mask) == mask
    }

    // --- Effect immunity ---

    pub fn add_effect_immunity(&mut self, bit: u32) {
        self.effect_type_mask |= bit;
    }

    pub fn remove_effect_immunity(&mut self, bit: u32) {
        self.effect_type_mask &= !bit;
    }

    /// Returns `true` when `enabled` and all bits in `mask` are set.
    pub fn has_effect_immunity(&self, mask: u32) -> bool {
        self.enabled && (self.effect_type_mask & mask) == mask
    }

    // --- Convenience ---

    pub fn clear_damage_immunities(&mut self) {
        self.damage_type_mask = 0;
    }

    pub fn clear_effect_immunities(&mut self) {
        self.effect_type_mask = 0;
    }

    pub fn clear_all(&mut self) {
        self.damage_type_mask = 0;
        self.effect_type_mask = 0;
    }

    pub fn is_any_immune(&self) -> bool {
        self.enabled && (self.damage_type_mask != 0 || self.effect_type_mask != 0)
    }
}

impl Default for Immune {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRE: u32 = 1 << 0;
    const COLD: u32 = 1 << 1;
    const POISON: u32 = 1 << 2;
    const STUN: u32 = 1 << 0;
    const BLIND: u32 = 1 << 1;

    #[test]
    fn add_damage_immunity() {
        let mut im = Immune::new();
        im.add_damage_immunity(FIRE);
        assert!(im.has_damage_immunity(FIRE));
        assert!(!im.has_damage_immunity(COLD));
    }

    #[test]
    fn remove_damage_immunity() {
        let mut im = Immune::new();
        im.add_damage_immunity(FIRE | COLD);
        im.remove_damage_immunity(FIRE);
        assert!(!im.has_damage_immunity(FIRE));
        assert!(im.has_damage_immunity(COLD));
    }

    #[test]
    fn multi_bit_damage_check() {
        let mut im = Immune::new();
        im.add_damage_immunity(FIRE);
        // FIRE | COLD set required — only FIRE set → false
        assert!(!im.has_damage_immunity(FIRE | COLD));
        im.add_damage_immunity(COLD);
        assert!(im.has_damage_immunity(FIRE | COLD));
    }

    #[test]
    fn add_effect_immunity() {
        let mut im = Immune::new();
        im.add_effect_immunity(STUN);
        assert!(im.has_effect_immunity(STUN));
        assert!(!im.has_effect_immunity(BLIND));
    }

    #[test]
    fn remove_effect_immunity() {
        let mut im = Immune::new();
        im.add_effect_immunity(STUN | BLIND);
        im.remove_effect_immunity(STUN);
        assert!(!im.has_effect_immunity(STUN));
        assert!(im.has_effect_immunity(BLIND));
    }

    #[test]
    fn clear_damage_immunities() {
        let mut im = Immune::new();
        im.add_damage_immunity(FIRE | COLD | POISON);
        im.clear_damage_immunities();
        assert!(!im.has_damage_immunity(FIRE));
    }

    #[test]
    fn clear_all() {
        let mut im = Immune::new();
        im.add_damage_immunity(FIRE);
        im.add_effect_immunity(STUN);
        im.clear_all();
        assert!(!im.is_any_immune());
    }

    #[test]
    fn disabled_never_immune() {
        let mut im = Immune::new();
        im.add_damage_immunity(FIRE);
        im.add_effect_immunity(STUN);
        im.enabled = false;
        assert!(!im.has_damage_immunity(FIRE));
        assert!(!im.has_effect_immunity(STUN));
        assert!(!im.is_any_immune());
    }

    #[test]
    fn is_any_immune_false_when_empty() {
        let im = Immune::new();
        assert!(!im.is_any_immune());
    }
}
