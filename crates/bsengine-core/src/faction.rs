use bevy_ecs::prelude::Component;

/// Relationship of another faction toward the owner of this component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactionRel {
    Friendly,
    Neutral,
    Hostile,
}

/// Faction membership for an entity.
///
/// `id` identifies which team/side the entity belongs to.
/// `relations` maps foreign faction IDs to their relationship with this faction.
/// The AI, targeting, and damage systems query `relation_to(other_id)` to decide
/// whether to attack, assist, or ignore another entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Faction {
    /// This entity's faction ID.
    pub id: u32,
    /// Relationship table: faction_id → relationship toward this faction.
    /// Entries not in the table fall back to `default_rel`.
    pub relations: Vec<(u32, FactionRel)>,
    /// Relationship used for any faction not listed in `relations`.
    pub default_rel: FactionRel,
}

impl Faction {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            relations: Vec::new(),
            default_rel: FactionRel::Neutral,
        }
    }

    pub fn with_default_rel(mut self, rel: FactionRel) -> Self {
        self.default_rel = rel;
        self
    }

    /// Declare a specific relationship toward another faction.
    pub fn set_relation(&mut self, other_id: u32, rel: FactionRel) {
        if let Some(entry) = self.relations.iter_mut().find(|(id, _)| *id == other_id) {
            entry.1 = rel;
        } else {
            self.relations.push((other_id, rel));
        }
    }

    /// Builder-style `set_relation`.
    pub fn with_relation(mut self, other_id: u32, rel: FactionRel) -> Self {
        self.set_relation(other_id, rel);
        self
    }

    /// Return this faction's relationship toward `other_id`.
    pub fn relation_to(&self, other_id: u32) -> FactionRel {
        if other_id == self.id {
            return FactionRel::Friendly;
        }
        self.relations
            .iter()
            .find(|(id, _)| *id == other_id)
            .map_or(self.default_rel, |(_, rel)| *rel)
    }

    pub fn is_friendly_to(&self, other_id: u32) -> bool {
        self.relation_to(other_id) == FactionRel::Friendly
    }

    pub fn is_hostile_to(&self, other_id: u32) -> bool {
        self.relation_to(other_id) == FactionRel::Hostile
    }
}

impl Default for Faction {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_faction_is_friendly() {
        let f = Faction::new(1);
        assert_eq!(f.relation_to(1), FactionRel::Friendly);
    }

    #[test]
    fn explicit_relation_overrides_default() {
        let f = Faction::new(1)
            .with_default_rel(FactionRel::Neutral)
            .with_relation(2, FactionRel::Hostile);
        assert_eq!(f.relation_to(2), FactionRel::Hostile);
        assert_eq!(f.relation_to(3), FactionRel::Neutral);
    }

    #[test]
    fn set_relation_updates_existing() {
        let mut f = Faction::new(1).with_relation(2, FactionRel::Neutral);
        f.set_relation(2, FactionRel::Friendly);
        assert!(f.is_friendly_to(2));
    }

    #[test]
    fn hostile_query() {
        let f = Faction::new(1).with_relation(3, FactionRel::Hostile);
        assert!(f.is_hostile_to(3));
        assert!(!f.is_hostile_to(1));
    }

    #[test]
    fn default_rel_applies_to_unknown_factions() {
        let f = Faction::new(10).with_default_rel(FactionRel::Hostile);
        assert_eq!(f.relation_to(99), FactionRel::Hostile);
    }
}
