use bevy_ecs::prelude::Component;

/// Completion state of the quest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuestState {
    /// Quest has been assigned but the player hasn't met requirements yet.
    #[default]
    Active,
    /// All objectives met; awaiting hand-in with the quest-giver.
    ReadyToComplete,
    /// Quest has been handed in and rewards granted.
    Completed,
    /// Quest was abandoned before completion.
    Abandoned,
}

/// A single tracked objective within the quest (e.g. "kill 5 wolves", "collect 3 herbs").
#[derive(Debug, Clone, PartialEq)]
pub struct QuestObjective {
    pub description: String,
    pub current: u32,
    pub required: u32,
    pub completed: bool,
}

impl QuestObjective {
    pub fn new(description: impl Into<String>, required: u32) -> Self {
        Self {
            description: description.into(),
            current: 0,
            required: required.max(1),
            completed: false,
        }
    }

    pub fn progress(&mut self, amount: u32) -> bool {
        if self.completed {
            return false;
        }
        self.current = (self.current + amount).min(self.required);
        self.completed = self.current >= self.required;
        self.completed
    }

    pub fn is_done(&self) -> bool {
        self.completed
    }
}

/// A quest attached to an entity (player, party member, or quest-giver NPC).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Quest {
    pub id: String,
    pub title: String,
    pub state: QuestState,
    pub objectives: Vec<QuestObjective>,
    pub xp_reward: f32,
    pub enabled: bool,
}

impl Quest {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            state: QuestState::Active,
            objectives: Vec::new(),
            xp_reward: 0.0,
            enabled: true,
        }
    }

    pub fn with_objective(mut self, objective: QuestObjective) -> Self {
        self.objectives.push(objective);
        self
    }

    pub fn with_xp_reward(mut self, xp: f32) -> Self {
        self.xp_reward = xp.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns `true` when all objectives are complete.
    pub fn all_objectives_done(&self) -> bool {
        !self.objectives.is_empty() && self.objectives.iter().all(|o| o.is_done())
    }

    /// Advance the objective at `index` by `amount`. Updates quest state if all objectives complete.
    /// Returns `true` if the quest just became ready to complete.
    pub fn progress_objective(&mut self, index: usize, amount: u32) -> bool {
        if !self.enabled || self.state != QuestState::Active {
            return false;
        }
        if let Some(obj) = self.objectives.get_mut(index) {
            obj.progress(amount);
        }
        if self.all_objectives_done() {
            self.state = QuestState::ReadyToComplete;
            return true;
        }
        false
    }

    /// Mark the quest as completed and grant rewards (caller applies xp_reward).
    pub fn complete(&mut self) -> bool {
        if self.state == QuestState::ReadyToComplete {
            self.state = QuestState::Completed;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_objective_progress() {
        let mut obj = QuestObjective::new("kill wolves", 3);
        assert!(!obj.progress(2));
        assert!(obj.progress(1));
        assert!(obj.is_done());
    }

    #[test]
    fn quest_ready_when_all_done() {
        let mut q = Quest::new("q1", "Hunt").with_objective(QuestObjective::new("slay 1 boar", 1));
        assert!(q.progress_objective(0, 1));
        assert_eq!(q.state, QuestState::ReadyToComplete);
    }

    #[test]
    fn quest_complete() {
        let mut q = Quest::new("q1", "Hunt").with_objective(QuestObjective::new("slay", 1));
        q.progress_objective(0, 1);
        assert!(q.complete());
        assert_eq!(q.state, QuestState::Completed);
    }

    #[test]
    fn quest_disabled_ignores_progress() {
        let mut q = Quest::new("q1", "Hunt")
            .with_objective(QuestObjective::new("slay", 1))
            .disabled();
        assert!(!q.progress_objective(0, 1));
        assert_eq!(q.state, QuestState::Active);
    }

    #[test]
    fn quest_all_objectives_done() {
        let mut q = Quest::new("q1", "Gather")
            .with_objective(QuestObjective::new("herbs", 2))
            .with_objective(QuestObjective::new("water", 1));
        q.progress_objective(0, 2);
        assert!(!q.all_objectives_done());
        q.progress_objective(1, 1);
        assert!(q.all_objectives_done());
    }
}
