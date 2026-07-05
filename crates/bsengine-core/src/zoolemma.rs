use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoolemma {
    pub integrity: f32,
    pub max_integrity: f32,
    pub repair_rate: f32,
    pub just_repaired: bool,
    pub just_breached: bool,
    pub enabled: bool,
}

impl Zoolemma {
    pub fn new(max_integrity: f32, repair_rate: f32) -> Self {
        Self {
            integrity: 0.0,
            max_integrity: max_integrity.max(0.1),
            repair_rate: repair_rate.max(0.0),
            just_repaired: false,
            just_breached: false,
            enabled: true,
        }
    }

    pub fn repair(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.integrity < self.max_integrity;
        self.integrity = (self.integrity + amount).min(self.max_integrity);
        if was_below && self.integrity >= self.max_integrity {
            self.just_repaired = true;
        }
    }

    pub fn breach(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.integrity <= 0.0 {
            return;
        }
        self.integrity = (self.integrity - amount).max(0.0);
        if self.integrity <= 0.0 {
            self.just_breached = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_repaired = false;
        self.just_breached = false;
        if self.enabled && self.repair_rate > 0.0 && self.integrity < self.max_integrity {
            let was_below = self.integrity < self.max_integrity;
            self.integrity = (self.integrity + self.repair_rate * dt).min(self.max_integrity);
            if was_below && self.integrity >= self.max_integrity {
                self.just_repaired = true;
            }
        }
    }
}

impl Default for Zoolemma {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
