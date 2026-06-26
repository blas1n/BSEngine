use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// Priority tier used by targeting systems to rank candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Marker placed on an entity to advertise it as a valid target.
///
/// AI systems query for `Target` when selecting attack/follow targets.
/// Systems that lock onto a target entity read the locked entity's `Target`
/// component to check visibility, priority, and allegiance flags.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Target {
    /// Display priority — higher-priority targets are preferred by AI.
    pub priority: TargetPriority,
    /// Layer mask identifying what targeting groups can lock onto this entity.
    pub targetable_by: u32,
    /// Whether this entity is currently targetable. False = immune to lock-on.
    pub is_targetable: bool,
    /// Faction the target belongs to (matches `Faction.id`).
    pub faction_id: u32,
    /// World-space aim point offset from the entity's origin (e.g., chest height).
    pub aim_offset: Vec3,
    /// Optional health fraction [0, 1] written by the health system for priority sorting.
    pub health_fraction: f32,
    /// Entity that currently has this target locked. None if untargeted.
    pub locked_by: Option<Entity>,
    pub enabled: bool,
}

impl Target {
    pub fn new() -> Self {
        Self {
            priority: TargetPriority::Normal,
            targetable_by: u32::MAX,
            is_targetable: true,
            faction_id: 0,
            aim_offset: Vec3::new(0.0, 1.0, 0.0),
            health_fraction: 1.0,
            locked_by: None,
            enabled: true,
        }
    }

    pub fn with_priority(mut self, priority: TargetPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_targetable_by(mut self, mask: u32) -> Self {
        self.targetable_by = mask;
        self
    }

    pub fn with_faction(mut self, faction_id: u32) -> Self {
        self.faction_id = faction_id;
        self
    }

    pub fn with_aim_offset(mut self, offset: Vec3) -> Self {
        self.aim_offset = offset;
        self
    }

    pub fn untargetable(mut self) -> Self {
        self.is_targetable = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns true if `attacker_layer` can target this entity.
    pub fn can_be_targeted_by(&self, attacker_layer: u32) -> bool {
        self.enabled && self.is_targetable && (self.targetable_by & attacker_layer) != 0
    }

    /// Lock `locker` onto this target. Returns false if already locked by someone else.
    pub fn lock(&mut self, locker: Entity) -> bool {
        if self.locked_by.is_some() && self.locked_by != Some(locker) {
            return false;
        }
        self.locked_by = Some(locker);
        true
    }

    /// Release the lock held by `locker`. No-op if locked by someone else.
    pub fn unlock(&mut self, locker: Entity) {
        if self.locked_by == Some(locker) {
            self.locked_by = None;
        }
    }

    pub fn is_locked(&self) -> bool {
        self.locked_by.is_some()
    }
}

impl Default for Target {
    fn default() -> Self {
        Self::new()
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
    fn can_be_targeted_layer_check() {
        let t = Target::new().with_targetable_by(0b0011);
        assert!(t.can_be_targeted_by(0b0001));
        assert!(!t.can_be_targeted_by(0b0100));
    }

    #[test]
    fn lock_and_unlock() {
        let es = entities(1);
        let mut t = Target::new();
        assert!(t.lock(es[0]));
        assert!(t.is_locked());
        t.unlock(es[0]);
        assert!(!t.is_locked());
    }

    #[test]
    fn second_locker_rejected() {
        let es = entities(2);
        let mut t = Target::new();
        t.lock(es[0]);
        assert!(!t.lock(es[1]));
        assert_eq!(t.locked_by, Some(es[0]));
    }

    #[test]
    fn untargetable_cannot_be_targeted() {
        let t = Target::new().untargetable();
        assert!(!t.can_be_targeted_by(u32::MAX));
    }

    #[test]
    fn priority_ordering() {
        assert!(TargetPriority::Critical > TargetPriority::Normal);
        assert!(TargetPriority::Low < TargetPriority::High);
    }
}
