use bevy_ecs::prelude::Component;

/// A single entry in a loot table — an item identifier and its probability weight.
#[derive(Debug, Clone, PartialEq)]
pub struct LootEntry {
    /// Item identifier (e.g. `"item:sword_common"`, `"currency:gold"`).
    pub item_id: String,
    /// Relative drop weight. Higher = more likely relative to other entries.
    pub weight: f32,
    /// How many of this item to spawn per drop. Drawn uniformly in `[min_count, max_count]`.
    pub min_count: u32,
    pub max_count: u32,
}

impl LootEntry {
    pub fn new(item_id: impl Into<String>, weight: f32) -> Self {
        Self {
            item_id: item_id.into(),
            weight: weight.max(0.0),
            min_count: 1,
            max_count: 1,
        }
    }

    pub fn with_count(mut self, min: u32, max: u32) -> Self {
        self.min_count = min;
        self.max_count = max.max(min);
        self
    }
}

/// A weighted loot drop table attached to an entity (chest, enemy, boss).
/// The loot system selects `rolls` entries each time the entity is destroyed or opened.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct LootTable {
    /// All possible drop entries.
    pub entries: Vec<LootEntry>,
    /// How many entries to select per drop event. Defaults to 1.
    pub rolls: u32,
    /// Multiplier applied to all weights (e.g. 2.0 = double luck).
    pub luck_modifier: f32,
    pub enabled: bool,
}

impl LootTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            rolls: 1,
            luck_modifier: 1.0,
            enabled: true,
        }
    }

    pub fn with_rolls(mut self, rolls: u32) -> Self {
        self.rolls = rolls.max(1);
        self
    }

    pub fn with_luck(mut self, modifier: f32) -> Self {
        self.luck_modifier = modifier.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn add(mut self, entry: LootEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Total cumulative weight of all entries (after luck modifier).
    pub fn total_weight(&self) -> f32 {
        self.entries
            .iter()
            .map(|e| e.weight * self.luck_modifier)
            .sum()
    }

    /// Returns the entry whose cumulative weight bracket contains `sample` in [0, total_weight).
    /// Returns `None` if the table is empty or sample is out of range.
    pub fn pick(&self, sample: f32) -> Option<&LootEntry> {
        if !self.enabled || self.entries.is_empty() {
            return None;
        }
        let mut cumulative = 0.0;
        for entry in &self.entries {
            cumulative += entry.weight * self.luck_modifier;
            if sample < cumulative {
                return Some(entry);
            }
        }
        self.entries.last()
    }
}

impl Default for LootTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loot_table_total_weight() {
        let table = LootTable::new()
            .add(LootEntry::new("gold", 3.0))
            .add(LootEntry::new("sword", 1.0));
        assert!((table.total_weight() - 4.0).abs() < 0.001);
    }

    #[test]
    fn loot_table_pick_first() {
        let table = LootTable::new()
            .add(LootEntry::new("gold", 3.0))
            .add(LootEntry::new("sword", 1.0));
        let entry = table.pick(0.0).unwrap();
        assert_eq!(entry.item_id, "gold");
    }

    #[test]
    fn loot_table_pick_second() {
        let table = LootTable::new()
            .add(LootEntry::new("gold", 3.0))
            .add(LootEntry::new("sword", 1.0));
        let entry = table.pick(3.5).unwrap();
        assert_eq!(entry.item_id, "sword");
    }

    #[test]
    fn luck_modifier_scales_weight() {
        let table = LootTable::new()
            .with_luck(2.0)
            .add(LootEntry::new("gem", 1.0));
        assert!((table.total_weight() - 2.0).abs() < 0.001);
    }

    #[test]
    fn disabled_table_returns_none() {
        let table = LootTable::new().add(LootEntry::new("gold", 1.0)).disabled();
        assert!(table.pick(0.0).is_none());
    }
}
