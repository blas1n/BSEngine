use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooculture {
    pub breed: f32,
    pub max_breed: f32,
    pub cultivate_rate: f32,
    pub just_cultivated: bool,
    pub just_abandoned: bool,
    pub enabled: bool,
}

impl Zooculture {
    pub fn new(max_breed: f32, cultivate_rate: f32) -> Self {
        Self {
            breed: 0.0,
            max_breed: max_breed.max(0.1),
            cultivate_rate: cultivate_rate.max(0.0),
            just_cultivated: false,
            just_abandoned: false,
            enabled: true,
        }
    }

    pub fn cultivate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.breed < self.max_breed;
        self.breed = (self.breed + amount).min(self.max_breed);
        if was_below && self.breed >= self.max_breed {
            self.just_cultivated = true;
        }
    }

    pub fn abandon(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.breed <= 0.0 {
            return;
        }
        self.breed = (self.breed - amount).max(0.0);
        if self.breed <= 0.0 {
            self.just_abandoned = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_cultivated = false;
        self.just_abandoned = false;
        if self.enabled && self.cultivate_rate > 0.0 && self.breed < self.max_breed {
            let was_below = self.breed < self.max_breed;
            self.breed = (self.breed + self.cultivate_rate * dt).min(self.max_breed);
            if was_below && self.breed >= self.max_breed {
                self.just_cultivated = true;
            }
        }
    }
}

impl Default for Zooculture {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
