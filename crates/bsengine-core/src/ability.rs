use bevy_ecs::prelude::Component;

/// A single activatable ability attached to an entity (attack, dash, spell, etc.).
/// The ability system reads this each frame to determine whether an activation is legal.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ability {
    /// Display name, used by the UI and action binding system.
    pub name: String,
    /// Time in seconds that must elapse after use before the ability can fire again.
    pub cooldown: f32,
    /// How many seconds remain on the current cooldown. 0 = ready.
    pub cooldown_remaining: f32,
    /// Maximum number of charges held simultaneously. `0` means unlimited (cooldown-only).
    pub max_charges: u32,
    /// Current stored charges.
    pub charges: u32,
    /// Time in seconds between charge regenerations (used only when `max_charges > 0`).
    pub charge_regen_time: f32,
    /// Accumulated seconds toward the next charge.
    pub charge_regen_accumulated: f32,
    pub enabled: bool,
}

impl Ability {
    pub fn new(name: impl Into<String>, cooldown: f32) -> Self {
        Self {
            name: name.into(),
            cooldown: cooldown.max(0.0),
            cooldown_remaining: 0.0,
            max_charges: 0,
            charges: 0,
            charge_regen_time: 0.0,
            charge_regen_accumulated: 0.0,
            enabled: true,
        }
    }

    /// Add charge-based usage on top of the cooldown.
    pub fn with_charges(mut self, max: u32, regen_time: f32) -> Self {
        self.max_charges = max;
        self.charges = max;
        self.charge_regen_time = regen_time.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns `true` if the ability can be activated right now.
    pub fn is_ready(&self) -> bool {
        if !self.enabled {
            return false;
        }
        let cooldown_ok = self.cooldown_remaining <= 0.0;
        let charges_ok = self.max_charges == 0 || self.charges > 0;
        cooldown_ok && charges_ok
    }

    /// Activate the ability. Returns `true` on success; does nothing and returns `false` if not ready.
    pub fn activate(&mut self) -> bool {
        if !self.is_ready() {
            return false;
        }
        self.cooldown_remaining = self.cooldown;
        if self.max_charges > 0 {
            self.charges = self.charges.saturating_sub(1);
        }
        true
    }

    /// Tick cooldown and charge regen by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if self.cooldown_remaining > 0.0 {
            self.cooldown_remaining = (self.cooldown_remaining - dt).max(0.0);
        }
        if self.max_charges > 0 && self.charges < self.max_charges && self.charge_regen_time > 0.0 {
            self.charge_regen_accumulated += dt;
            while self.charge_regen_accumulated >= self.charge_regen_time
                && self.charges < self.max_charges
            {
                self.charge_regen_accumulated -= self.charge_regen_time;
                self.charges += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ability_ready_with_no_cooldown() {
        let mut a = Ability::new("dash", 0.0);
        assert!(a.is_ready());
        assert!(a.activate());
    }

    #[test]
    fn cooldown_blocks_reuse() {
        let mut a = Ability::new("slash", 1.0);
        assert!(a.activate());
        assert!(!a.is_ready());
        a.tick(1.0);
        assert!(a.is_ready());
    }

    #[test]
    fn charge_usage() {
        let mut a = Ability::new("arrow", 0.0).with_charges(3, 5.0);
        assert!(a.activate());
        assert!(a.activate());
        assert!(a.activate());
        assert!(!a.activate());
    }

    #[test]
    fn charge_regen() {
        let mut a = Ability::new("shield", 0.0).with_charges(2, 2.0);
        a.activate();
        assert_eq!(a.charges, 1);
        a.tick(2.0);
        assert_eq!(a.charges, 2);
    }

    #[test]
    fn disabled_ability_not_ready() {
        let a = Ability::new("fireball", 0.0).disabled();
        assert!(!a.is_ready());
    }
}
