use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoosporangia {
    pub spore: f32,
    pub max_spore: f32,
    pub sporulate_rate: f32,
    pub just_sporulated: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Zoosporangia {
    pub fn new(max_spore: f32, sporulate_rate: f32) -> Self {
        Self {
            spore: 0.0,
            max_spore: max_spore.max(0.1),
            sporulate_rate: sporulate_rate.max(0.0),
            just_sporulated: false,
            just_depleted: false,
            enabled: true,
        }
    }

    pub fn sporulate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.spore < self.max_spore;
        self.spore = (self.spore + amount).min(self.max_spore);
        if was_below && self.spore >= self.max_spore {
            self.just_sporulated = true;
        }
    }

    pub fn deplete(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.spore <= 0.0 {
            return;
        }
        self.spore = (self.spore - amount).max(0.0);
        if self.spore <= 0.0 {
            self.just_depleted = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_sporulated = false;
        self.just_depleted = false;
        if self.enabled && self.sporulate_rate > 0.0 && self.spore < self.max_spore {
            let was_below = self.spore < self.max_spore;
            self.spore = (self.spore + self.sporulate_rate * dt).min(self.max_spore);
            if was_below && self.spore >= self.max_spore {
                self.just_sporulated = true;
            }
        }
    }
}

impl Default for Zoosporangia {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
