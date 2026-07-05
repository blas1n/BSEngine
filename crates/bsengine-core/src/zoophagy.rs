use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoophagy {
    pub prey: f32,
    pub max_prey: f32,
    pub hunt_rate: f32,
    pub just_hunted: bool,
    pub just_fled: bool,
    pub enabled: bool,
}

impl Zoophagy {
    pub fn new(max_prey: f32, hunt_rate: f32) -> Self {
        Self {
            prey: 0.0,
            max_prey: max_prey.max(0.1),
            hunt_rate: hunt_rate.max(0.0),
            just_hunted: false,
            just_fled: false,
            enabled: true,
        }
    }

    pub fn hunt(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.prey < self.max_prey;
        self.prey = (self.prey + amount).min(self.max_prey);
        if was_below && self.prey >= self.max_prey {
            self.just_hunted = true;
        }
    }

    pub fn flee(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.prey <= 0.0 {
            return;
        }
        self.prey = (self.prey - amount).max(0.0);
        if self.prey <= 0.0 {
            self.just_fled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_hunted = false;
        self.just_fled = false;
        if self.enabled && self.hunt_rate > 0.0 && self.prey < self.max_prey {
            let was_below = self.prey < self.max_prey;
            self.prey = (self.prey + self.hunt_rate * dt).min(self.max_prey);
            if was_below && self.prey >= self.max_prey {
                self.just_hunted = true;
            }
        }
    }
}

impl Default for Zoophagy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
