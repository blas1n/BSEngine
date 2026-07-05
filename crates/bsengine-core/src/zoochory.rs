use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoochory {
    pub seed: f32,
    pub max_seed: f32,
    pub disperse_rate: f32,
    pub just_dispersed: bool,
    pub just_contained: bool,
    pub enabled: bool,
}

impl Zoochory {
    pub fn new(max_seed: f32, disperse_rate: f32) -> Self {
        Self {
            seed: 0.0,
            max_seed: max_seed.max(0.1),
            disperse_rate: disperse_rate.max(0.0),
            just_dispersed: false,
            just_contained: false,
            enabled: true,
        }
    }

    pub fn disperse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.seed < self.max_seed;
        self.seed = (self.seed + amount).min(self.max_seed);
        if was_below && self.seed >= self.max_seed {
            self.just_dispersed = true;
        }
    }

    pub fn contain(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.seed <= 0.0 {
            return;
        }
        self.seed = (self.seed - amount).max(0.0);
        if self.seed <= 0.0 {
            self.just_contained = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_dispersed = false;
        self.just_contained = false;
        if self.enabled && self.disperse_rate > 0.0 && self.seed < self.max_seed {
            let was_below = self.seed < self.max_seed;
            self.seed = (self.seed + self.disperse_rate * dt).min(self.max_seed);
            if was_below && self.seed >= self.max_seed {
                self.just_dispersed = true;
            }
        }
    }
}

impl Default for Zoochory {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
