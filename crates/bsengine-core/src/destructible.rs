use bevy_ecs::prelude::Component;

/// Stage the entity passes through as it takes damage and breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestructionState {
    /// Undamaged and fully intact.
    Intact,
    /// Health has dropped below `damage_threshold` but the object still exists.
    Damaged,
    /// Health has reached 0; the destruction system will despawn or swap mesh.
    Destroyed,
}

/// Marks an entity as breakable.
///
/// The damage system writes `health` down to 0. This component tracks whether
/// the object has crossed the visual-damage threshold and whether it has been
/// destroyed so the rendering and physics systems can react (swap meshes,
/// spawn debris, etc.).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Destructible {
    pub state: DestructionState,
    /// Current structural health. When ≤ 0 the object is destroyed.
    pub health: f32,
    /// Starting health used to compute damage fraction.
    pub max_health: f32,
    /// Fraction of max_health below which `state` transitions to `Damaged`.
    /// Value in (0, 1]. E.g. 0.5 = visually damaged at half health.
    pub damage_threshold: f32,
    /// Whether this object should leave debris entities when destroyed.
    pub spawns_debris: bool,
    /// Optional ID for the debris/broken-mesh prefab to spawn.
    pub debris_template: Option<String>,
    /// Whether the destroyed state has been processed by the destruction system.
    pub destruction_handled: bool,
    pub enabled: bool,
}

impl Destructible {
    pub fn new(max_health: f32) -> Self {
        Self {
            state: DestructionState::Intact,
            health: max_health.max(1.0),
            max_health: max_health.max(1.0),
            damage_threshold: 0.5,
            spawns_debris: false,
            debris_template: None,
            destruction_handled: false,
            enabled: true,
        }
    }

    pub fn with_damage_threshold(mut self, threshold: f32) -> Self {
        self.damage_threshold = threshold.clamp(0.001, 1.0);
        self
    }

    pub fn with_debris(mut self, template: impl Into<String>) -> Self {
        self.spawns_debris = true;
        self.debris_template = Some(template.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Apply damage and update state. Returns `true` when the entity is newly destroyed.
    pub fn apply_damage(&mut self, amount: f32) -> bool {
        if !self.enabled || self.state == DestructionState::Destroyed {
            return false;
        }
        self.health = (self.health - amount.max(0.0)).max(0.0);
        self.update_state()
    }

    /// Repair by `amount`. Clears the Destroyed state if health is restored above 0.
    pub fn repair(&mut self, amount: f32) {
        self.health = (self.health + amount.max(0.0)).min(self.max_health);
        if self.health > 0.0 && self.state == DestructionState::Destroyed {
            self.destruction_handled = false;
        }
        self.update_state();
    }

    fn update_state(&mut self) -> bool {
        let previous = self.state;
        self.state = if self.health <= 0.0 {
            DestructionState::Destroyed
        } else if self.health / self.max_health <= self.damage_threshold {
            DestructionState::Damaged
        } else {
            DestructionState::Intact
        };
        self.state == DestructionState::Destroyed && previous != DestructionState::Destroyed
    }

    pub fn health_fraction(&self) -> f32 {
        (self.health / self.max_health).clamp(0.0, 1.0)
    }

    pub fn is_destroyed(&self) -> bool {
        self.state == DestructionState::Destroyed
    }

    pub fn is_damaged(&self) -> bool {
        self.state == DestructionState::Damaged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_damage_transitions_to_damaged() {
        let mut d = Destructible::new(100.0).with_damage_threshold(0.5);
        d.apply_damage(51.0);
        assert_eq!(d.state, DestructionState::Damaged);
        assert!(!d.is_destroyed());
    }

    #[test]
    fn apply_damage_transitions_to_destroyed() {
        let mut d = Destructible::new(100.0);
        let newly_destroyed = d.apply_damage(100.0);
        assert!(newly_destroyed);
        assert!(d.is_destroyed());
        assert!((d.health).abs() < 0.001);
    }

    #[test]
    fn second_hit_does_not_return_true() {
        let mut d = Destructible::new(100.0);
        d.apply_damage(100.0);
        let again = d.apply_damage(10.0);
        assert!(!again);
    }

    #[test]
    fn health_fraction_correct() {
        let mut d = Destructible::new(200.0);
        d.apply_damage(50.0);
        assert!((d.health_fraction() - 0.75).abs() < 0.001);
    }

    #[test]
    fn repair_restores_intact() {
        let mut d = Destructible::new(100.0);
        d.apply_damage(100.0);
        assert!(d.is_destroyed());
        d.repair(100.0);
        assert_eq!(d.state, DestructionState::Intact);
    }
}
