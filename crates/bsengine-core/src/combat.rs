use bevy_ecs::prelude::{Component, Entity};

/// Stance that affects animation and ability availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatStance {
    Neutral,
    Aggressive,
    Defensive,
    Fleeing,
}

/// Tracks whether an entity is actively engaged in combat and with whom.
///
/// Game systems enter/exit combat by calling `enter(entity)` / `exit_all()`.
/// `tick(dt)` counts down `cooldown_timer`; when it reaches 0 the entity is
/// no longer in combat (if `auto_exit` is enabled).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Combat {
    /// Whether the entity is currently considered in-combat.
    pub in_combat: bool,
    /// Entities engaged with this one.
    pub engaged_with: Vec<Entity>,
    /// Stance affecting animation/ability selection.
    pub stance: CombatStance,
    /// Seconds after the last combat event before auto-exit. 0 = no auto-exit.
    pub exit_delay: f32,
    /// Counts down from `exit_delay` each tick; reset on every `enter`.
    pub exit_timer: f32,
    /// When true, `tick` automatically clears `in_combat` when `exit_timer` expires.
    pub auto_exit: bool,
    /// Total number of kills attributed to this entity.
    pub kill_count: u32,
    pub enabled: bool,
}

impl Combat {
    pub fn new() -> Self {
        Self {
            in_combat: false,
            engaged_with: Vec::new(),
            stance: CombatStance::Neutral,
            exit_delay: 5.0,
            exit_timer: 0.0,
            auto_exit: true,
            kill_count: 0,
            enabled: true,
        }
    }

    pub fn with_exit_delay(mut self, seconds: f32) -> Self {
        self.exit_delay = seconds.max(0.0);
        self
    }

    pub fn with_stance(mut self, stance: CombatStance) -> Self {
        self.stance = stance;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Register an engagement with `opponent`. Resets exit timer.
    pub fn enter(&mut self, opponent: Entity) {
        self.in_combat = true;
        self.exit_timer = self.exit_delay;
        if !self.engaged_with.contains(&opponent) {
            self.engaged_with.push(opponent);
        }
    }

    /// Immediately exit combat and clear all engagements.
    pub fn exit_all(&mut self) {
        self.in_combat = false;
        self.exit_timer = 0.0;
        self.engaged_with.clear();
        self.stance = CombatStance::Neutral;
    }

    /// Remove a single opponent. Exits combat if no opponents remain.
    pub fn disengage(&mut self, opponent: Entity) {
        self.engaged_with.retain(|&e| e != opponent);
        if self.engaged_with.is_empty() {
            self.in_combat = false;
            self.exit_timer = 0.0;
        }
    }

    /// Advance exit timer. Returns `true` when combat state transitions to out-of-combat.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled || !self.in_combat || !self.auto_exit || self.exit_delay <= 0.0 {
            return false;
        }
        self.exit_timer -= dt;
        if self.exit_timer <= 0.0 {
            self.exit_all();
            return true;
        }
        false
    }

    pub fn register_kill(&mut self) {
        self.kill_count += 1;
    }

    pub fn is_engaged_with(&self, entity: Entity) -> bool {
        self.engaged_with.contains(&entity)
    }
}

impl Default for Combat {
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
    fn enter_sets_in_combat() {
        let es = entities(1);
        let mut c = Combat::new();
        c.enter(es[0]);
        assert!(c.in_combat);
        assert!(c.is_engaged_with(es[0]));
    }

    #[test]
    fn exit_all_clears_state() {
        let es = entities(2);
        let mut c = Combat::new();
        c.enter(es[0]);
        c.enter(es[1]);
        c.exit_all();
        assert!(!c.in_combat);
        assert!(c.engaged_with.is_empty());
    }

    #[test]
    fn disengage_last_exits_combat() {
        let es = entities(1);
        let mut c = Combat::new();
        c.enter(es[0]);
        c.disengage(es[0]);
        assert!(!c.in_combat);
    }

    #[test]
    fn tick_auto_exits_after_delay() {
        let es = entities(1);
        let mut c = Combat::new().with_exit_delay(1.0);
        c.enter(es[0]);
        assert!(!c.tick(0.5));
        assert!(c.tick(0.6));
        assert!(!c.in_combat);
    }

    #[test]
    fn enter_resets_exit_timer() {
        let es = entities(2);
        let mut c = Combat::new().with_exit_delay(2.0);
        c.enter(es[0]);
        c.tick(1.5);
        c.enter(es[1]);
        assert!((c.exit_timer - 2.0).abs() < 0.001);
    }
}
