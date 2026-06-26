use bevy_ecs::prelude::{Component, Entity};

/// A single threat entry: who generated it and how much.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreatEntry {
    pub source: Entity,
    pub threat: f32,
}

/// Aggro (threat) table on an AI entity.
///
/// The AI system reads `current_target` each frame to decide who to attack.
/// Call `add_threat(source, amount)` when a source deals damage or heals allies;
/// call `tick(dt)` to decay threat over time.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Aggro {
    /// Threat table, sorted descending by threat on each update.
    pub entries: Vec<ThreatEntry>,
    /// Entity currently being focused. `None` = no target.
    pub current_target: Option<Entity>,
    /// Threat decay per second (fraction of current threat, 0.0 = no decay).
    pub decay_rate: f32,
    /// Threat threshold below which an entry is pruned from the table.
    pub prune_threshold: f32,
    /// If `true`, always attacks the highest-threat target (re-evaluates every tick).
    pub auto_retarget: bool,
    pub enabled: bool,
}

impl Aggro {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            current_target: None,
            decay_rate: 0.1,
            prune_threshold: 0.01,
            auto_retarget: true,
            enabled: true,
        }
    }

    pub fn with_decay_rate(mut self, rate: f32) -> Self {
        self.decay_rate = rate.max(0.0);
        self
    }

    pub fn with_prune_threshold(mut self, threshold: f32) -> Self {
        self.prune_threshold = threshold.max(0.0);
        self
    }

    pub fn manual_targeting(mut self) -> Self {
        self.auto_retarget = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Add `amount` threat from `source`. Creates a new entry if not already present.
    pub fn add_threat(&mut self, source: Entity, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        if let Some(entry) = self.entries.iter_mut().find(|e| e.source == source) {
            entry.threat += amount;
        } else {
            self.entries.push(ThreatEntry {
                source,
                threat: amount,
            });
        }
        self.entries
            .sort_unstable_by(|a, b| b.threat.partial_cmp(&a.threat).unwrap());
        if self.auto_retarget {
            self.current_target = self.entries.first().map(|e| e.source);
        }
    }

    /// Remove all threat from `source`.
    pub fn remove_threat(&mut self, source: Entity) {
        self.entries.retain(|e| e.source != source);
        if self.current_target == Some(source) {
            self.current_target = self.entries.first().map(|e| e.source);
        }
    }

    /// Decay all threat values and prune entries below `prune_threshold`.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }
        for entry in &mut self.entries {
            entry.threat *= 1.0 - self.decay_rate * dt;
        }
        self.entries.retain(|e| e.threat >= self.prune_threshold);
        if self.auto_retarget {
            self.current_target = self.entries.first().map(|e| e.source);
        }
    }

    /// Total threat from `source`, or 0.0 if not present.
    pub fn threat_of(&self, source: Entity) -> f32 {
        self.entries
            .iter()
            .find(|e| e.source == source)
            .map_or(0.0, |e| e.threat)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_target = None;
    }
}

impl Default for Aggro {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::World;

    fn spawn_entities(n: usize) -> Vec<Entity> {
        let mut world = World::new();
        (0..n).map(|_| world.spawn_empty().id()).collect()
    }

    #[test]
    fn add_threat_creates_entry() {
        let entities = spawn_entities(1);
        let mut a = Aggro::new();
        a.add_threat(entities[0], 50.0);
        assert_eq!(a.threat_of(entities[0]), 50.0);
        assert_eq!(a.current_target, Some(entities[0]));
    }

    #[test]
    fn highest_threat_is_current_target() {
        let entities = spawn_entities(2);
        let mut a = Aggro::new();
        a.add_threat(entities[0], 30.0);
        a.add_threat(entities[1], 80.0);
        assert_eq!(a.current_target, Some(entities[1]));
    }

    #[test]
    fn remove_threat_retargets() {
        let entities = spawn_entities(2);
        let mut a = Aggro::new();
        a.add_threat(entities[0], 40.0);
        a.add_threat(entities[1], 70.0);
        a.remove_threat(entities[1]);
        assert_eq!(a.current_target, Some(entities[0]));
    }

    #[test]
    fn tick_decays_and_prunes() {
        let entities = spawn_entities(1);
        let mut a = Aggro::new().with_decay_rate(1.0).with_prune_threshold(0.5);
        a.add_threat(entities[0], 1.0);
        // After 2 seconds at 100% decay/s the threat should drop below threshold
        a.tick(2.0);
        assert!(a.entries.is_empty());
        assert_eq!(a.current_target, None);
    }

    #[test]
    fn disabled_tick_does_not_decay() {
        let entities = spawn_entities(1);
        let mut a = Aggro::new().with_decay_rate(1.0).disabled();
        a.add_threat(entities[0], 100.0);
        a.tick(10.0);
        assert!(!a.entries.is_empty());
    }
}
