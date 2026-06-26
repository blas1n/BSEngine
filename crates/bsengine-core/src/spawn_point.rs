use bevy_ecs::prelude::Component;

/// A tagged world-space location where entities can be spawned.
/// The spawn system reads this component to find available spawn positions.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SpawnPoint {
    /// Logical category tag, e.g. `"player"`, `"enemy"`, `"item"`.
    pub tag: String,
    /// Optional team index. `None` = shared (any team may spawn here).
    pub team: Option<u32>,
    /// When `false` the spawn system skips this point entirely.
    pub enabled: bool,
}

impl SpawnPoint {
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            team: None,
            enabled: true,
        }
    }

    pub fn for_team(mut self, team: u32) -> Self {
        self.team = Some(team);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns `true` if this spawn point is available to the given team.
    /// Shared points (no team set) are available to all teams.
    pub fn available_for(&self, team: u32) -> bool {
        self.enabled && self.team.map_or(true, |t| t == team)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_point_defaults() {
        let sp = SpawnPoint::new("player");
        assert_eq!(sp.tag, "player");
        assert!(sp.team.is_none());
        assert!(sp.enabled);
    }

    #[test]
    fn shared_available_to_all_teams() {
        let sp = SpawnPoint::new("item");
        assert!(sp.available_for(0));
        assert!(sp.available_for(1));
        assert!(sp.available_for(99));
    }

    #[test]
    fn team_spawn_point_restricted() {
        let sp = SpawnPoint::new("base").for_team(1);
        assert!(!sp.available_for(0));
        assert!(sp.available_for(1));
    }

    #[test]
    fn disabled_unavailable() {
        let sp = SpawnPoint::new("any").disabled();
        assert!(!sp.available_for(0));
    }
}
