use bevy_ecs::prelude::Component;

/// Tracks the carrying capacity and current load of an entity's inventory.
/// Actual items are separate entities; systems link them via the owner entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Inventory {
    /// Maximum number of item slots. `None` = unlimited.
    pub max_slots: Option<u32>,
    /// Maximum carry weight in kg. `None` = unlimited.
    pub max_weight: Option<f32>,
    /// Number of slots currently occupied.
    pub used_slots: u32,
    /// Current total weight of all carried items, in kg.
    pub current_weight: f32,
    /// When `false` the inventory rejects all additions.
    pub enabled: bool,
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_slots(mut self, slots: u32) -> Self {
        self.max_slots = Some(slots);
        self
    }

    pub fn with_max_weight(mut self, kg: f32) -> Self {
        self.max_weight = Some(kg.max(0.0));
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Remaining free slots, or `None` when there is no slot limit.
    pub fn free_slots(&self) -> Option<u32> {
        self.max_slots
            .map(|max| max.saturating_sub(self.used_slots))
    }

    /// Remaining carry capacity in kg, or `None` when there is no weight limit.
    pub fn remaining_weight(&self) -> Option<f32> {
        self.max_weight
            .map(|max| (max - self.current_weight).max(0.0))
    }

    /// Returns `true` if the item can be added (slots and weight both allow it).
    pub fn can_add(&self, weight: f32) -> bool {
        if !self.enabled {
            return false;
        }
        let slot_ok = self.free_slots().map_or(true, |f| f > 0);
        let weight_ok = self
            .max_weight
            .map_or(true, |max| self.current_weight + weight <= max);
        slot_ok && weight_ok
    }

    /// Adds an item; returns `true` on success. Does not modify if it would overflow.
    pub fn add_item(&mut self, weight: f32) -> bool {
        if !self.can_add(weight) {
            return false;
        }
        self.used_slots += 1;
        self.current_weight += weight;
        true
    }

    /// Removes an item. `weight` must match what was originally added.
    /// Does nothing if the slot or weight would go below zero.
    pub fn remove_item(&mut self, weight: f32) -> bool {
        if self.used_slots == 0 {
            return false;
        }
        self.used_slots -= 1;
        self.current_weight = (self.current_weight - weight).max(0.0);
        true
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            max_slots: None,
            max_weight: None,
            used_slots: 0,
            current_weight: 0.0,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_defaults() {
        let inv = Inventory::new();
        assert!(inv.max_slots.is_none());
        assert!(inv.max_weight.is_none());
        assert_eq!(inv.used_slots, 0);
        assert!(inv.enabled);
    }

    #[test]
    fn can_add_respects_slot_limit() {
        let mut inv = Inventory::new().with_max_slots(2);
        assert!(inv.add_item(0.5));
        assert!(inv.add_item(0.5));
        assert!(!inv.can_add(0.1));
    }

    #[test]
    fn can_add_respects_weight_limit() {
        let mut inv = Inventory::new().with_max_weight(10.0);
        assert!(inv.add_item(9.0));
        assert!(!inv.can_add(2.0));
        assert!(inv.can_add(1.0));
    }

    #[test]
    fn add_and_remove_item() {
        let mut inv = Inventory::new();
        assert!(inv.add_item(3.0));
        assert_eq!(inv.used_slots, 1);
        assert!((inv.current_weight - 3.0).abs() < 0.001);
        assert!(inv.remove_item(3.0));
        assert_eq!(inv.used_slots, 0);
    }

    #[test]
    fn disabled_inventory_rejects_adds() {
        let mut inv = Inventory::new().disabled();
        assert!(!inv.add_item(1.0));
    }
}
