use bevy_ecs::prelude::Component;

/// Standard equipment slots. Values are bit positions in the `occupied` mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EquipSlot {
    Head = 0,
    Chest = 1,
    Legs = 2,
    Feet = 3,
    MainHand = 4,
    OffHand = 5,
    Ring = 6,
    Amulet = 7,
}

impl EquipSlot {
    pub const ALL: [EquipSlot; 8] = [
        EquipSlot::Head,
        EquipSlot::Chest,
        EquipSlot::Legs,
        EquipSlot::Feet,
        EquipSlot::MainHand,
        EquipSlot::OffHand,
        EquipSlot::Ring,
        EquipSlot::Amulet,
    ];

    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// Equipment slot manager component — tracks what is equipped in each slot.
///
/// Item identity is represented by a `u32` item-id (0 = nothing). The
/// `occupied` bitmask mirrors the slot contents for fast "is anything
/// equipped in slot X?" tests.
///
/// Call `equip(slot, item_id)` to place an item; `unequip(slot)` to remove
/// it. `swap(slot_a, slot_b)` exchanges two slots atomically.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Equip {
    /// Item id occupying each slot (index == EquipSlot as u8). 0 = empty.
    item_ids: [u32; 8],
    /// Bitmask of occupied slots (bit `EquipSlot as u8`).
    pub occupied: u8,
    /// Total equipment weight (sum added by callers when equipping).
    pub total_weight: f32,
    /// Maximum allowed total weight (0 = unlimited).
    pub max_weight: f32,
    pub enabled: bool,
}

impl Equip {
    pub fn new() -> Self {
        Self {
            item_ids: [0; 8],
            occupied: 0,
            total_weight: 0.0,
            max_weight: 0.0,
            enabled: true,
        }
    }

    pub fn with_max_weight(mut self, max: f32) -> Self {
        self.max_weight = max.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Equip `item_id` (non-zero) in `slot`, optionally providing its `weight`.
    /// Returns false if: disabled, item_id is 0, slot already occupied, or
    /// equipping would exceed max_weight.
    pub fn equip(&mut self, slot: EquipSlot, item_id: u32, weight: f32) -> bool {
        if !self.enabled || item_id == 0 {
            return false;
        }
        if self.is_occupied(slot) {
            return false;
        }
        if self.max_weight > 0.0 && self.total_weight + weight > self.max_weight {
            return false;
        }
        let idx = slot as usize;
        self.item_ids[idx] = item_id;
        self.occupied |= slot.bit();
        self.total_weight += weight.max(0.0);
        true
    }

    /// Remove whatever is in `slot`. Returns the removed item_id (0 if empty).
    pub fn unequip(&mut self, slot: EquipSlot) -> u32 {
        if !self.enabled {
            return 0;
        }
        let idx = slot as usize;
        let id = self.item_ids[idx];
        if id != 0 {
            self.item_ids[idx] = 0;
            self.occupied &= !slot.bit();
        }
        id
    }

    /// Swap the contents of two slots atomically. Returns false if disabled or
    /// both slots are empty.
    pub fn swap(&mut self, a: EquipSlot, b: EquipSlot) -> bool {
        if !self.enabled || a == b {
            return false;
        }
        let ai = a as usize;
        let bi = b as usize;
        if self.item_ids[ai] == 0 && self.item_ids[bi] == 0 {
            return false;
        }
        self.item_ids.swap(ai, bi);
        // Rebuild occupied bits for the two slots.
        let mask_a = a.bit();
        let mask_b = b.bit();
        let a_was = self.occupied & mask_a != 0;
        let b_was = self.occupied & mask_b != 0;
        if a_was {
            self.occupied |= mask_b;
        } else {
            self.occupied &= !mask_b;
        }
        if b_was {
            self.occupied |= mask_a;
        } else {
            self.occupied &= !mask_a;
        }
        true
    }

    pub fn item_in(&self, slot: EquipSlot) -> u32 {
        self.item_ids[slot as usize]
    }

    pub fn is_occupied(&self, slot: EquipSlot) -> bool {
        self.occupied & slot.bit() != 0
    }

    pub fn is_empty(&self) -> bool {
        self.occupied == 0
    }

    pub fn slot_count(&self) -> u32 {
        self.occupied.count_ones()
    }

    pub fn can_equip(&self, weight: f32) -> bool {
        self.enabled && (self.max_weight <= 0.0 || self.total_weight + weight <= self.max_weight)
    }
}

impl Default for Equip {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equip_and_query() {
        let mut e = Equip::new();
        assert!(e.equip(EquipSlot::MainHand, 42, 1.5));
        assert!(e.is_occupied(EquipSlot::MainHand));
        assert_eq!(e.item_in(EquipSlot::MainHand), 42);
        assert!((e.total_weight - 1.5).abs() < 1e-5);
    }

    #[test]
    fn cannot_double_equip_slot() {
        let mut e = Equip::new();
        e.equip(EquipSlot::Head, 1, 0.0);
        assert!(!e.equip(EquipSlot::Head, 2, 0.0));
        assert_eq!(e.item_in(EquipSlot::Head), 1);
    }

    #[test]
    fn unequip_clears_slot() {
        let mut e = Equip::new();
        e.equip(EquipSlot::Chest, 7, 2.0);
        let removed = e.unequip(EquipSlot::Chest);
        assert_eq!(removed, 7);
        assert!(!e.is_occupied(EquipSlot::Chest));
    }

    #[test]
    fn max_weight_blocks_equip() {
        let mut e = Equip::new().with_max_weight(5.0);
        assert!(e.equip(EquipSlot::Legs, 1, 4.0));
        assert!(!e.equip(EquipSlot::Feet, 2, 2.0)); // would exceed 5
        assert_eq!(e.slot_count(), 1);
    }

    #[test]
    fn swap_exchanges_slots() {
        let mut e = Equip::new();
        e.equip(EquipSlot::MainHand, 10, 0.0);
        e.equip(EquipSlot::OffHand, 20, 0.0);
        e.swap(EquipSlot::MainHand, EquipSlot::OffHand);
        assert_eq!(e.item_in(EquipSlot::MainHand), 20);
        assert_eq!(e.item_in(EquipSlot::OffHand), 10);
    }

    #[test]
    fn disabled_blocks_equip() {
        let mut e = Equip::new().disabled();
        assert!(!e.equip(EquipSlot::Ring, 99, 0.0));
        assert!(e.is_empty());
    }
}
