use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoodendrium {
    pub branch: f32,
    pub max_branch: f32,
    pub extend_rate: f32,
    pub just_extended: bool,
    pub just_pruned: bool,
    pub enabled: bool,
}

impl Zoodendrium {
    pub fn new(max_branch: f32, extend_rate: f32) -> Self {
        Self {
            branch: 0.0,
            max_branch: max_branch.max(0.1),
            extend_rate: extend_rate.max(0.0),
            just_extended: false,
            just_pruned: false,
            enabled: true,
        }
    }

    pub fn extend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.branch < self.max_branch;
        self.branch = (self.branch + amount).min(self.max_branch);
        if was_below && self.branch >= self.max_branch {
            self.just_extended = true;
        }
    }

    pub fn prune(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.branch <= 0.0 {
            return;
        }
        self.branch = (self.branch - amount).max(0.0);
        if self.branch <= 0.0 {
            self.just_pruned = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_extended = false;
        self.just_pruned = false;
        if self.enabled && self.extend_rate > 0.0 && self.branch < self.max_branch {
            let was_below = self.branch < self.max_branch;
            self.branch = (self.branch + self.extend_rate * dt).min(self.max_branch);
            if was_below && self.branch >= self.max_branch {
                self.just_extended = true;
            }
        }
    }
}

impl Default for Zoodendrium {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
