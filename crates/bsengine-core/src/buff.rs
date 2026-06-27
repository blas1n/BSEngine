use bevy_ecs::prelude::{Component, Entity};

/// A single active buff entry on an entity.
#[derive(Debug, Clone, PartialEq)]
pub struct BuffEntry {
    /// Caller-defined buff type ID.
    pub kind: u32,
    /// Magnitude of the effect (e.g. 0.25 = +25% to something).
    pub strength: f32,
    /// Remaining duration in seconds.
    pub duration: f32,
    /// The entity that applied this buff, if any.
    pub source: Option<Entity>,
}

impl BuffEntry {
    pub fn new(kind: u32, strength: f32, duration: f32) -> Self {
        Self {
            kind,
            strength: strength.max(0.0),
            duration,
            source: None,
        }
    }

    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }
}

/// Manages a collection of timed buffs on an entity.
///
/// Buffs are identified by `kind` (a caller-defined `u32`). `apply` uses a
/// high-watermark strategy: if an entry with the same kind already exists, the
/// new one replaces it only when its duration is longer. Use `apply_stack` when
/// true additive stacking is needed (e.g. stat bonuses from multiple sources).
///
/// `tick(dt)` removes expired entries and sets `just_applied` / `just_expired`
/// for single-frame VFX and sound hooks.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Buff {
    pub entries: Vec<BuffEntry>,
    /// True the frame a new entry was added or an existing one refreshed.
    pub just_applied: bool,
    /// True the frame any entry expired naturally.
    pub just_expired: bool,
    pub enabled: bool,
}

impl Buff {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            just_applied: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Apply a buff. Refreshes duration/strength for the same `kind` only if
    /// the new duration is longer (high-watermark).
    pub fn apply(&mut self, entry: BuffEntry) {
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
    pub fn apply_stack(&mut self, entry: BuffEntry) {
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

impl Default for Buff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_adds_entry() {
        let mut b = Buff::new();
        b.apply(BuffEntry::new(1, 0.5, 5.0));
        assert!(b.has(1));
        assert!(b.just_applied);
    }

    #[test]
    fn apply_refreshes_on_longer_duration() {
        let mut b = Buff::new();
        b.apply(BuffEntry::new(1, 0.3, 3.0));
        b.apply(BuffEntry::new(1, 0.7, 8.0));
        assert_eq!(b.entries.len(), 1);
        assert!((b.entries[0].duration - 8.0).abs() < 1e-5);
        assert!((b.entries[0].strength - 0.7).abs() < 1e-5);
    }

    #[test]
    fn apply_no_refresh_on_shorter_duration() {
        let mut b = Buff::new();
        b.apply(BuffEntry::new(1, 0.5, 8.0));
        b.apply(BuffEntry::new(1, 0.9, 2.0));
        assert_eq!(b.entries.len(), 1);
        assert!((b.entries[0].duration - 8.0).abs() < 1e-5);
    }

    #[test]
    fn apply_stack_allows_multiple_same_kind() {
        let mut b = Buff::new();
        b.apply_stack(BuffEntry::new(1, 0.3, 5.0));
        b.apply_stack(BuffEntry::new(1, 0.3, 5.0));
        assert_eq!(b.entries.len(), 2);
    }

    #[test]
    fn tick_removes_expired() {
        let mut b = Buff::new();
        b.apply(BuffEntry::new(1, 0.5, 1.0));
        b.tick(1.1);
        assert!(!b.has(1));
        assert!(b.just_expired);
    }

    #[test]
    fn tick_clears_just_applied() {
        let mut b = Buff::new();
        b.apply(BuffEntry::new(1, 0.5, 5.0));
        b.tick(0.016);
        assert!(!b.just_applied);
    }

    #[test]
    fn remove_by_kind() {
        let mut b = Buff::new();
        b.apply(BuffEntry::new(1, 0.5, 5.0));
        b.apply(BuffEntry::new(2, 0.5, 5.0));
        b.remove(1);
        assert!(!b.has(1));
        assert!(b.has(2));
    }

    #[test]
    fn total_strength_sums_stacked() {
        let mut b = Buff::new();
        b.apply_stack(BuffEntry::new(3, 0.2, 5.0));
        b.apply_stack(BuffEntry::new(3, 0.3, 5.0));
        let s = b.total_strength(3);
        assert!((s - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut b = Buff::new();
        b.enabled = false;
        b.apply(BuffEntry::new(1, 0.5, 5.0));
        assert!(b.is_empty());
    }
}
