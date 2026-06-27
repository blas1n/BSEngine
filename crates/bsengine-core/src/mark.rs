use bevy_ecs::prelude::{Component, Entity};

/// Marks a target entity, amplifying incoming damage and signaling coordinated focus-fire.
///
/// Common in RPGs and action games (e.g. "Marked for Death", "Death Mark"):
/// a caller applies a mark with a kind ID, damage multiplier, and duration.
/// The damage system checks `is_marked` and multiplies incoming damage by
/// `(1.0 + damage_bonus_fraction)` before applying it. Multiple concurrent
/// marks from different sources are supported via `marks: Vec<MarkEntry>`;
/// `total_damage_bonus()` sums all active bonuses.
///
/// `tick(dt)` ages entries, removes expired ones, and sets `just_marked` /
/// `just_unmarked` flags for sound and VFX.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkEntry {
    /// Caller-defined mark type/source ID.
    pub kind: u32,
    /// Bonus damage fraction (0.5 = +50% incoming damage).
    pub damage_bonus_fraction: f32,
    /// Remaining duration in seconds.
    pub duration: f32,
    /// Entity that applied this mark.
    pub source: Option<Entity>,
}

impl MarkEntry {
    pub fn new(kind: u32, damage_bonus_fraction: f32, duration: f32) -> Self {
        Self {
            kind,
            damage_bonus_fraction: damage_bonus_fraction.max(0.0),
            duration,
            source: None,
        }
    }

    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }
}

/// Manages all active marks on an entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Mark {
    pub marks: Vec<MarkEntry>,
    pub just_marked: bool,
    pub just_unmarked: bool,
    pub enabled: bool,
}

impl Mark {
    pub fn new() -> Self {
        Self {
            marks: Vec::new(),
            just_marked: false,
            just_unmarked: false,
            enabled: true,
        }
    }

    /// Apply a mark. If an entry with the same `kind` already exists, the new
    /// one replaces it only if its duration is longer (high-watermark).
    pub fn apply(&mut self, entry: MarkEntry) {
        if !self.enabled {
            return;
        }

        if let Some(existing) = self.marks.iter_mut().find(|m| m.kind == entry.kind) {
            if entry.duration > existing.duration {
                *existing = entry;
            }
        } else {
            self.marks.push(entry);
        }

        self.just_marked = true;
    }

    /// Remove all marks with the given kind.
    pub fn remove(&mut self, kind: u32) {
        self.marks.retain(|m| m.kind != kind);
    }

    pub fn clear(&mut self) {
        self.marks.clear();
    }

    /// Advance all mark timers; remove expired entries.
    pub fn tick(&mut self, dt: f32) {
        self.just_marked = false;
        let before = self.marks.len();
        self.marks.retain_mut(|m| {
            m.duration -= dt;
            m.duration > 0.0
        });
        self.just_unmarked = self.marks.len() < before;
    }

    pub fn is_marked(&self) -> bool {
        !self.marks.is_empty()
    }

    pub fn has_kind(&self, kind: u32) -> bool {
        self.marks.iter().any(|m| m.kind == kind)
    }

    /// Combined outgoing damage multiplier to apply. Returns 1.0 if no marks active.
    ///
    /// Bonuses add additively: two marks each with 0.25 bonus → multiplier 1.5.
    pub fn damage_multiplier(&self) -> f32 {
        1.0 + self.total_damage_bonus()
    }

    pub fn total_damage_bonus(&self) -> f32 {
        self.marks.iter().map(|m| m.damage_bonus_fraction).sum()
    }
}

impl Default for Mark {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_adds_mark() {
        let mut m = Mark::new();
        m.apply(MarkEntry::new(1, 0.5, 5.0));
        assert!(m.is_marked());
        assert!(m.just_marked);
    }

    #[test]
    fn apply_refreshes_longer_duration() {
        let mut m = Mark::new();
        m.apply(MarkEntry::new(1, 0.3, 3.0));
        m.apply(MarkEntry::new(1, 0.5, 8.0));
        assert_eq!(m.marks.len(), 1);
        assert!((m.marks[0].duration - 8.0).abs() < 1e-5);
        assert!((m.marks[0].damage_bonus_fraction - 0.5).abs() < 1e-5);
    }

    #[test]
    fn apply_no_refresh_shorter_duration() {
        let mut m = Mark::new();
        m.apply(MarkEntry::new(1, 0.5, 8.0));
        m.apply(MarkEntry::new(1, 0.3, 2.0));
        assert!((m.marks[0].duration - 8.0).abs() < 1e-5);
    }

    #[test]
    fn tick_removes_expired() {
        let mut m = Mark::new();
        m.apply(MarkEntry::new(1, 0.5, 1.0));
        m.tick(1.1);
        assert!(!m.is_marked());
        assert!(m.just_unmarked);
    }

    #[test]
    fn damage_multiplier_sums_bonuses() {
        let mut m = Mark::new();
        m.apply(MarkEntry::new(1, 0.25, 5.0));
        m.apply(MarkEntry::new(2, 0.25, 5.0));
        assert!((m.damage_multiplier() - 1.5).abs() < 1e-5);
    }

    #[test]
    fn remove_by_kind() {
        let mut m = Mark::new();
        m.apply(MarkEntry::new(1, 0.5, 5.0));
        m.apply(MarkEntry::new(2, 0.5, 5.0));
        m.remove(1);
        assert!(!m.has_kind(1));
        assert!(m.has_kind(2));
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut m = Mark::new();
        m.enabled = false;
        m.apply(MarkEntry::new(1, 0.5, 5.0));
        assert!(!m.is_marked());
    }

    #[test]
    fn damage_multiplier_one_when_no_marks() {
        let m = Mark::new();
        assert!((m.damage_multiplier() - 1.0).abs() < 1e-5);
    }
}
