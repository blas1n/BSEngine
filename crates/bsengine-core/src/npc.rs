use bevy_ecs::prelude::Component;

/// Broad category that drives AI behavior selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcRole {
    /// Passive: wanders, flees on threat.
    Civilian,
    /// Engages hostiles on sight; patrols when idle.
    Guard,
    /// Stays near a home point; attacks if provoked.
    Creature,
    /// Provides quests, dialogue, or trade.
    Vendor,
    /// Script-controlled for cutscenes.
    Scripted,
}

/// Current high-level AI behavior state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcState {
    Idle,
    Patrolling,
    Investigating,
    Alerted,
    Engaging,
    Fleeing,
    Interacting,
    Dead,
}

/// Marker and metadata component for non-player characters.
///
/// The NPC AI system selects behavior based on `role` and transitions
/// `state` in response to game events. Systems that query for NPCs
/// use this component as the primary filter.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Npc {
    pub role: NpcRole,
    pub state: NpcState,
    /// Display name shown above the character or in UI.
    pub display_name: String,
    /// Optional unique identifier used to key dialogue/quest data.
    pub template_id: Option<String>,
    /// Faction the NPC belongs to (matches `Faction.id`).
    pub faction_id: u32,
    /// Alert level in [0, 1]. 0 = unaware, 1 = fully alerted.
    pub alert: f32,
    /// Rate at which `alert` decays per second when no threat is visible.
    pub alert_decay: f32,
    pub enabled: bool,
}

impl Npc {
    pub fn new(role: NpcRole, display_name: impl Into<String>) -> Self {
        Self {
            role,
            state: NpcState::Idle,
            display_name: display_name.into(),
            template_id: None,
            faction_id: 0,
            alert: 0.0,
            alert_decay: 0.2,
            enabled: true,
        }
    }

    pub fn civilian(name: impl Into<String>) -> Self {
        Self::new(NpcRole::Civilian, name)
    }

    pub fn guard(name: impl Into<String>) -> Self {
        Self::new(NpcRole::Guard, name)
    }

    pub fn vendor(name: impl Into<String>) -> Self {
        Self::new(NpcRole::Vendor, name)
    }

    pub fn with_template(mut self, id: impl Into<String>) -> Self {
        self.template_id = Some(id.into());
        self
    }

    pub fn with_faction(mut self, faction_id: u32) -> Self {
        self.faction_id = faction_id;
        self
    }

    pub fn with_alert_decay(mut self, rate: f32) -> Self {
        self.alert_decay = rate.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Raise alert by `amount`, clamped to [0, 1].
    pub fn raise_alert(&mut self, amount: f32) {
        self.alert = (self.alert + amount).clamp(0.0, 1.0);
    }

    /// Decay alert toward 0 over `dt` seconds.
    pub fn tick_alert(&mut self, dt: f32) {
        self.alert = (self.alert - self.alert_decay * dt).max(0.0);
    }

    pub fn is_fully_alerted(&self) -> bool {
        self.alert >= 1.0
    }

    pub fn is_hostile_state(&self) -> bool {
        matches!(self.state, NpcState::Alerted | NpcState::Engaging)
    }

    pub fn is_alive(&self) -> bool {
        self.state != NpcState::Dead
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_raises_and_clamps() {
        let mut n = Npc::guard("Guard");
        n.raise_alert(0.7);
        assert!((n.alert - 0.7).abs() < 0.001);
        n.raise_alert(0.5);
        assert!((n.alert - 1.0).abs() < 0.001);
    }

    #[test]
    fn alert_decays_over_time() {
        let mut n = Npc::guard("Guard").with_alert_decay(1.0);
        n.raise_alert(1.0);
        n.tick_alert(0.4);
        assert!((n.alert - 0.6).abs() < 0.001);
    }

    #[test]
    fn alert_does_not_go_below_zero() {
        let mut n = Npc::civilian("Villager");
        n.tick_alert(10.0);
        assert_eq!(n.alert, 0.0);
    }

    #[test]
    fn is_fully_alerted_at_max() {
        let mut n = Npc::guard("Guard");
        n.raise_alert(1.0);
        assert!(n.is_fully_alerted());
    }

    #[test]
    fn is_alive_until_dead() {
        let mut n = Npc::creature("Wolf");
        assert!(n.is_alive());
        n.state = NpcState::Dead;
        assert!(!n.is_alive());
    }

    impl Npc {
        fn creature(name: impl Into<String>) -> Self {
            Self::new(NpcRole::Creature, name)
        }
    }
}
