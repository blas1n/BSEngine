use bevy_ecs::prelude::{Component, Entity};

/// What the pickup grants when collected.
#[derive(Debug, Clone, PartialEq)]
pub enum PickupEffect {
    /// Restore health by this amount.
    Health(f32),
    /// Restore mana/energy by this amount.
    Mana(f32),
    /// Grant experience points.
    Experience(f32),
    /// Grant a fixed amount of currency.
    Currency(u32),
    /// Add ammo of a given weapon index.
    Ammo { weapon_index: u32, amount: u32 },
    /// Trigger a named effect (handled by game code).
    Custom(String),
}

/// A collectible entity that can be picked up by player or AI entities.
///
/// The collection system queries `can_collect(entity)` to check eligibility,
/// then calls `collect(entity)` to mark the pickup as consumed.
/// After `collect`, the owning system should despawn or deactivate the entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Pickup {
    pub effect: PickupEffect,
    /// If `Some(entity)`, only this specific entity can collect it.
    pub reserved_for: Option<Entity>,
    /// Layer mask: only entities whose layer bit overlaps may collect. 0 = any.
    pub collector_mask: u32,
    /// Optional attraction radius: the pickup moves toward collectors within this distance.
    pub magnet_radius: Option<f32>,
    /// How quickly the pickup moves toward a collector (units/second).
    pub magnet_speed: f32,
    /// Whether this pickup has already been collected.
    pub collected: bool,
    /// Who collected it (set by `collect`).
    pub collected_by: Option<Entity>,
    pub enabled: bool,
}

impl Pickup {
    pub fn new(effect: PickupEffect) -> Self {
        Self {
            effect,
            reserved_for: None,
            collector_mask: 0,
            magnet_radius: None,
            magnet_speed: 5.0,
            collected: false,
            collected_by: None,
            enabled: true,
        }
    }

    pub fn health(amount: f32) -> Self {
        Self::new(PickupEffect::Health(amount))
    }

    pub fn experience(amount: f32) -> Self {
        Self::new(PickupEffect::Experience(amount))
    }

    pub fn currency(amount: u32) -> Self {
        Self::new(PickupEffect::Currency(amount))
    }

    pub fn with_reserved_for(mut self, entity: Entity) -> Self {
        self.reserved_for = Some(entity);
        self
    }

    pub fn with_collector_mask(mut self, mask: u32) -> Self {
        self.collector_mask = mask;
        self
    }

    pub fn with_magnet(mut self, radius: f32) -> Self {
        self.magnet_radius = Some(radius.max(0.0));
        self
    }

    pub fn with_magnet_speed(mut self, speed: f32) -> Self {
        self.magnet_speed = speed.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns `true` if `collector` is allowed to pick this up.
    pub fn can_collect(&self, collector: Entity) -> bool {
        if !self.enabled || self.collected {
            return false;
        }
        if let Some(reserved) = self.reserved_for {
            return collector == reserved;
        }
        true
    }

    /// Mark this pickup as collected by `collector`.
    /// Returns `false` if already collected or ineligible.
    pub fn collect(&mut self, collector: Entity) -> bool {
        if !self.can_collect(collector) {
            return false;
        }
        self.collected = true;
        self.collected_by = Some(collector);
        true
    }

    pub fn is_available(&self) -> bool {
        self.enabled && !self.collected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::World;

    fn entities(n: usize) -> Vec<Entity> {
        let mut w = World::new();
        (0..n).map(|_| w.spawn_empty().id()).collect()
    }

    #[test]
    fn collect_marks_as_consumed() {
        let es = entities(1);
        let mut p = Pickup::health(50.0);
        assert!(p.collect(es[0]));
        assert!(p.collected);
        assert_eq!(p.collected_by, Some(es[0]));
    }

    #[test]
    fn double_collect_fails() {
        let es = entities(1);
        let mut p = Pickup::health(50.0);
        p.collect(es[0]);
        assert!(!p.collect(es[0]));
    }

    #[test]
    fn reserved_pickup_rejects_other_entity() {
        let es = entities(2);
        let mut p = Pickup::currency(100).with_reserved_for(es[0]);
        assert!(!p.can_collect(es[1]));
        assert!(p.can_collect(es[0]));
    }

    #[test]
    fn disabled_pickup_cannot_be_collected() {
        let es = entities(1);
        let mut p = Pickup::experience(10.0).disabled();
        assert!(!p.collect(es[0]));
    }

    #[test]
    fn is_available_after_collect() {
        let es = entities(1);
        let mut p = Pickup::health(25.0);
        assert!(p.is_available());
        p.collect(es[0]);
        assert!(!p.is_available());
    }
}
