use bevy_ecs::prelude::Component;

/// Quality tier of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

/// Category used for inventory filtering and equipment slot validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemCategory {
    Weapon,
    Armor,
    Consumable,
    Material,
    Quest,
    Miscellaneous,
}

/// Item metadata component placed on item entities (both world items and inventory slots).
///
/// The inventory and equipment systems read this component to display item details,
/// enforce stack limits, and validate equip rules.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Item {
    /// Stable identifier used to look up item templates, icons, and descriptions.
    pub template_id: String,
    /// Localisation key or fallback display name.
    pub display_name: String,
    pub category: ItemCategory,
    pub rarity: ItemRarity,
    /// Current stack count.
    pub stack: u32,
    /// Maximum number that can occupy a single inventory slot. 1 = non-stackable.
    pub max_stack: u32,
    /// Base sell price in game currency units.
    pub value: u32,
    /// Item weight in arbitrary units (affects carry limit).
    pub weight: f32,
    /// Whether the item has been identified (hidden stats until true).
    pub identified: bool,
    pub enabled: bool,
}

impl Item {
    pub fn new(template_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            template_id: template_id.into(),
            display_name: display_name.into(),
            category: ItemCategory::Miscellaneous,
            rarity: ItemRarity::Common,
            stack: 1,
            max_stack: 1,
            value: 0,
            weight: 1.0,
            identified: true,
            enabled: true,
        }
    }

    pub fn consumable(template_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            category: ItemCategory::Consumable,
            max_stack: 99,
            ..Self::new(template_id, name)
        }
    }

    pub fn with_category(mut self, category: ItemCategory) -> Self {
        self.category = category;
        self
    }

    pub fn with_rarity(mut self, rarity: ItemRarity) -> Self {
        self.rarity = rarity;
        self
    }

    pub fn with_max_stack(mut self, max: u32) -> Self {
        self.max_stack = max.max(1);
        self
    }

    pub fn with_value(mut self, value: u32) -> Self {
        self.value = value;
        self
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight.max(0.0);
        self
    }

    pub fn unidentified(mut self) -> Self {
        self.identified = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Add `amount` to the stack, clamped to `max_stack`.
    /// Returns the leftover that could not be added.
    pub fn add(&mut self, amount: u32) -> u32 {
        let space = self.max_stack.saturating_sub(self.stack);
        let added = amount.min(space);
        self.stack += added;
        amount - added
    }

    /// Remove `amount` from the stack. Returns how much was actually removed.
    pub fn remove(&mut self, amount: u32) -> u32 {
        let removed = amount.min(self.stack);
        self.stack -= removed;
        removed
    }

    pub fn is_empty(&self) -> bool {
        self.stack == 0
    }

    pub fn is_full(&self) -> bool {
        self.stack >= self.max_stack
    }

    pub fn is_stackable(&self) -> bool {
        self.max_stack > 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_clamps_at_max_stack() {
        let mut item = Item::consumable("health_potion", "Health Potion").with_max_stack(10);
        let leftover = item.add(8);
        assert_eq!(item.stack, 9);
        assert_eq!(leftover, 0);
        let leftover2 = item.add(5);
        assert_eq!(item.stack, 10);
        assert_eq!(leftover2, 4);
    }

    #[test]
    fn remove_clamps_at_zero() {
        let mut item = Item::new("sword", "Iron Sword");
        let removed = item.remove(5);
        assert_eq!(removed, 1);
        assert_eq!(item.stack, 0);
        assert!(item.is_empty());
    }

    #[test]
    fn rarity_ordering() {
        assert!(ItemRarity::Legendary > ItemRarity::Common);
        assert!(ItemRarity::Rare > ItemRarity::Uncommon);
    }

    #[test]
    fn is_full_after_add() {
        let mut item = Item::consumable("arrow", "Arrow").with_max_stack(5);
        item.add(4);
        assert!(item.is_full());
    }

    #[test]
    fn non_stackable_item_is_full_from_start() {
        let item = Item::new("shield", "Wooden Shield");
        assert!(!item.is_stackable());
        assert!(item.is_full());
    }
}
