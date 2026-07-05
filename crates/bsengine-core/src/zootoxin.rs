use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zootoxin {
    pub toxin: f32,
    pub max_toxin: f32,
    pub secrete_rate: f32,
    pub just_secreted: bool,
    pub just_neutralized: bool,
    pub enabled: bool,
}

impl Zootoxin {
    pub fn new(max_toxin: f32, secrete_rate: f32) -> Self {
        Self {
            toxin: 0.0,
            max_toxin: max_toxin.max(0.1),
            secrete_rate: secrete_rate.max(0.0),
            just_secreted: false,
            just_neutralized: false,
            enabled: true,
        }
    }

    pub fn secrete(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.toxin < self.max_toxin;
        self.toxin = (self.toxin + amount).min(self.max_toxin);
        if was_below && self.toxin >= self.max_toxin {
            self.just_secreted = true;
        }
    }

    pub fn neutralize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.toxin <= 0.0 {
            return;
        }
        self.toxin = (self.toxin - amount).max(0.0);
        if self.toxin <= 0.0 {
            self.just_neutralized = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_secreted = false;
        self.just_neutralized = false;
        if self.enabled && self.secrete_rate > 0.0 && self.toxin < self.max_toxin {
            let was_below = self.toxin < self.max_toxin;
            self.toxin = (self.toxin + self.secrete_rate * dt).min(self.max_toxin);
            if was_below && self.toxin >= self.max_toxin {
                self.just_secreted = true;
            }
        }
    }
}

impl Default for Zootoxin {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
