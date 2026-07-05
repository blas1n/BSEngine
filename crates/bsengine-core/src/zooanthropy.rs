use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooanthropy {
    pub delusion: f32,
    pub max_delusion: f32,
    pub impart_rate: f32,
    pub just_imparted: bool,
    pub just_dispelled: bool,
    pub enabled: bool,
}

impl Zooanthropy {
    pub fn new(max_delusion: f32, impart_rate: f32) -> Self {
        Self {
            delusion: 0.0,
            max_delusion: max_delusion.max(0.1),
            impart_rate: impart_rate.max(0.0),
            just_imparted: false,
            just_dispelled: false,
            enabled: true,
        }
    }

    pub fn impart(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.delusion < self.max_delusion;
        self.delusion = (self.delusion + amount).min(self.max_delusion);
        if was_below && self.delusion >= self.max_delusion {
            self.just_imparted = true;
        }
    }

    pub fn dispel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.delusion <= 0.0 {
            return;
        }
        self.delusion = (self.delusion - amount).max(0.0);
        if self.delusion <= 0.0 {
            self.just_dispelled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_imparted = false;
        self.just_dispelled = false;
        if self.enabled && self.impart_rate > 0.0 && self.delusion < self.max_delusion {
            let was_below = self.delusion < self.max_delusion;
            self.delusion = (self.delusion + self.impart_rate * dt).min(self.max_delusion);
            if was_below && self.delusion >= self.max_delusion {
                self.just_imparted = true;
            }
        }
    }
}

impl Default for Zooanthropy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
