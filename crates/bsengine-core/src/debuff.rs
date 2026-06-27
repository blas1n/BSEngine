use bevy_ecs::prelude::{Component, Entity};

/// A generic debuff entry attached to an entity.
#[derive(Debug, Clone, PartialEq)]
pub struct DebuffEntry {
    /// Caller-defined debuff kind (maps to a game-specific enum value cast to u32).
    pub kind: u32,
    /// Magnitude of the effect (0.0 = none, 1.0 = full).
    pub strength: f32,
    /// Remaining duration in seconds.
    pub duration: f32,
    /// The entity that applied this debuff, if any.
    pub source: Option<Entity>,
}

impl DebuffEntry {
    pub fn new(kind: u32, strength: f32, duration: f32) -> Self {
        Self {
            kind,
            strength,
            duration,
            source: None,
        }
    }

    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }
}

/// Manages a collection of timed debuffs on an entity.
///
/// Debuffs are identified by `kind` (a caller-defined `u32`). When the same
/// kind is applied again, `apply` refreshes the duration if the new duration is
/// longer (high-watermark) rather than stacking — matching common RPG behaviour.
/// Use `apply_stack` when true stacking is desired.
///
/// `tick(dt)` removes expired entries and sets `just_applied` / `just_expired`
/// flags for single-frame hooks (animation, VFX, sound).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Debuff {
    pub entries: Vec<DebuffEntry>,
    /// True the frame a new debuff kind was first added or an existing one was refreshed.
    pub just_applied: bool,
    /// True the frame any debuff expired naturally.
    pub just_expired: bool,
    pub enabled: bool,
}

impl Debuff {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            just_applied: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Apply a debuff by kind. Refreshes duration/strength if an entry with
    /// the same `kind` already exists and the new duration is longer.
    pub fn apply(&mut self, entry: DebuffEntry) {
        if !self.enabled {
            return;
        }

        if let Some(existing) = self.entries.iter_mut().find(|e| e.kind == entry.kind) {
            if entry.duration > existing.duration {
                existing.duration = entry.duration;
                existing.strength = entry.strength;
                existing.source = entry.source;
            }
        } else {
            self.entries.push(entry);
        }

        self.just_applied = true;
    }

    /// Always push a new entry regardless of existing ones (true stacking).
    pub fn apply_stack(&mut self, entry: DebuffEntry) {
        if !self.enabled {
            return;
        }
        self.entries.push(entry);
        self.just_applied = true;
    }

    /// Remove all entries with the given kind.
    pub fn remove(&mut self, kind: u32) {
        self.entries.retain(|e| e.kind != kind);
    }

    /// Remove all entries from a specific source entity.
    pub fn remove_from(&mut self, source: Entity) {
        self.entries.retain(|e| e.source != Some(source));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Tick down all entries by `dt`; remove expired ones and set `just_expired`.
    pub fn tick(&mut self, dt: f32) {
        self.just_applied = false;
        let before = self.entries.len();
        self.entries.retain_mut(|e| {
            e.duration -= dt;
            e.duration > 0.0
        });
        self.just_expired = self.entries.len() < before;
    }

    pub fn has(&self, kind: u32) -> bool {
        self.entries.iter().any(|e| e.kind == kind)
    }

    /// Total strength of all entries with the given `kind` (for stacked effects).
    pub fn total_strength(&self, kind: u32) -> f32 {
        self.entries
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| e.strength)
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Debuff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_adds_entry() {
        let mut d = Debuff::new();
        d.apply(DebuffEntry::new(1, 0.5, 5.0));
        assert!(d.has(1));
        assert!(d.just_applied);
    }

    #[test]
    fn apply_refreshes_on_longer_duration() {
        let mut d = Debuff::new();
        d.apply(DebuffEntry::new(1, 0.5, 3.0));
        d.apply(DebuffEntry::new(1, 0.8, 6.0));
        assert_eq!(d.entries.len(), 1);
        assert!((d.entries[0].duration - 6.0).abs() < 1e-5);
        assert!((d.entries[0].strength - 0.8).abs() < 1e-5);
    }

    #[test]
    fn apply_no_refresh_on_shorter_duration() {
        let mut d = Debuff::new();
        d.apply(DebuffEntry::new(1, 0.5, 6.0));
        d.apply(DebuffEntry::new(1, 0.9, 2.0));
        assert_eq!(d.entries.len(), 1);
        assert!((d.entries[0].duration - 6.0).abs() < 1e-5);
    }

    #[test]
    fn apply_stack_allows_multiple_same_kind() {
        let mut d = Debuff::new();
        d.apply_stack(DebuffEntry::new(1, 0.3, 5.0));
        d.apply_stack(DebuffEntry::new(1, 0.3, 5.0));
        assert_eq!(d.entries.len(), 2);
    }

    #[test]
    fn tick_removes_expired() {
        let mut d = Debuff::new();
        d.apply(DebuffEntry::new(1, 0.5, 1.0));
        d.tick(1.1);
        assert!(!d.has(1));
        assert!(d.just_expired);
    }

    #[test]
    fn tick_clears_just_applied() {
        let mut d = Debuff::new();
        d.apply(DebuffEntry::new(1, 0.5, 5.0));
        d.tick(0.016);
        assert!(!d.just_applied);
    }

    #[test]
    fn remove_by_kind() {
        let mut d = Debuff::new();
        d.apply(DebuffEntry::new(1, 0.5, 5.0));
        d.apply(DebuffEntry::new(2, 0.5, 5.0));
        d.remove(1);
        assert!(!d.has(1));
        assert!(d.has(2));
    }

    #[test]
    fn total_strength_sums_stacked() {
        let mut d = Debuff::new();
        d.apply_stack(DebuffEntry::new(3, 0.2, 5.0));
        d.apply_stack(DebuffEntry::new(3, 0.3, 5.0));
        let s = d.total_strength(3);
        assert!((s - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut d = Debuff::new();
        d.enabled = false;
        d.apply(DebuffEntry::new(1, 0.5, 5.0));
        assert!(d.is_empty());
    }
}
