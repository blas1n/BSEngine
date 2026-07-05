use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoognomy {
    pub lore: f32,
    pub max_lore: f32,
    pub study_rate: f32,
    pub just_learned: bool,
    pub just_forgotten: bool,
    pub enabled: bool,
}

impl Zoognomy {
    pub fn new(max_lore: f32, study_rate: f32) -> Self {
        Self {
            lore: 0.0,
            max_lore: max_lore.max(0.1),
            study_rate: study_rate.max(0.0),
            just_learned: false,
            just_forgotten: false,
            enabled: true,
        }
    }

    pub fn learn(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.lore < self.max_lore;
        self.lore = (self.lore + amount).min(self.max_lore);
        if was_below && self.lore >= self.max_lore {
            self.just_learned = true;
        }
    }

    pub fn forget(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.lore <= 0.0 {
            return;
        }
        self.lore = (self.lore - amount).max(0.0);
        if self.lore <= 0.0 {
            self.just_forgotten = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_learned = false;
        self.just_forgotten = false;
        if self.enabled && self.study_rate > 0.0 && self.lore < self.max_lore {
            let was_below = self.lore < self.max_lore;
            self.lore = (self.lore + self.study_rate * dt).min(self.max_lore);
            if was_below && self.lore >= self.max_lore {
                self.just_learned = true;
            }
        }
    }
}

impl Default for Zoognomy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
