use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zootaxy {
    pub rank: f32,
    pub max_rank: f32,
    pub promote_rate: f32,
    pub just_promoted: bool,
    pub just_demoted: bool,
    pub enabled: bool,
}

impl Zootaxy {
    pub fn new(max_rank: f32, promote_rate: f32) -> Self {
        Self {
            rank: 0.0,
            max_rank: max_rank.max(0.1),
            promote_rate: promote_rate.max(0.0),
            just_promoted: false,
            just_demoted: false,
            enabled: true,
        }
    }

    pub fn promote(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.rank < self.max_rank;
        self.rank = (self.rank + amount).min(self.max_rank);
        if was_below && self.rank >= self.max_rank {
            self.just_promoted = true;
        }
    }

    pub fn demote(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.rank <= 0.0 {
            return;
        }
        self.rank = (self.rank - amount).max(0.0);
        if self.rank <= 0.0 {
            self.just_demoted = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_promoted = false;
        self.just_demoted = false;
        if self.enabled && self.promote_rate > 0.0 && self.rank < self.max_rank {
            let was_below = self.rank < self.max_rank;
            self.rank = (self.rank + self.promote_rate * dt).min(self.max_rank);
            if was_below && self.rank >= self.max_rank {
                self.just_promoted = true;
            }
        }
    }
}

impl Default for Zootaxy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
