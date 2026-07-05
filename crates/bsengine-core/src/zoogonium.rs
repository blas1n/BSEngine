use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoogonium {
    pub spore: f32,
    pub max_spore: f32,
    pub produce_rate: f32,
    pub just_produced: bool,
    pub just_consumed: bool,
    pub enabled: bool,
}

impl Zoogonium {
    pub fn new(max_spore: f32, produce_rate: f32) -> Self {
        Self {
            spore: 0.0,
            max_spore: max_spore.max(0.1),
            produce_rate: produce_rate.max(0.0),
            just_produced: false,
            just_consumed: false,
            enabled: true,
        }
    }

    pub fn produce(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.spore < self.max_spore;
        self.spore = (self.spore + amount).min(self.max_spore);
        if was_below && self.spore >= self.max_spore {
            self.just_produced = true;
        }
    }

    pub fn consume(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.spore <= 0.0 {
            return;
        }
        self.spore = (self.spore - amount).max(0.0);
        if self.spore <= 0.0 {
            self.just_consumed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_produced = false;
        self.just_consumed = false;
        if self.enabled && self.produce_rate > 0.0 && self.spore < self.max_spore {
            let was_below = self.spore < self.max_spore;
            self.spore = (self.spore + self.produce_rate * dt).min(self.max_spore);
            if was_below && self.spore >= self.max_spore {
                self.just_produced = true;
            }
        }
    }
}

impl Default for Zoogonium {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
