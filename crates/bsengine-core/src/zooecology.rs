use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooecology {
    pub habitat: f32,
    pub max_habitat: f32,
    pub settle_rate: f32,
    pub just_settled: bool,
    pub just_displaced: bool,
    pub enabled: bool,
}

impl Zooecology {
    pub fn new(max_habitat: f32, settle_rate: f32) -> Self {
        Self {
            habitat: 0.0,
            max_habitat: max_habitat.max(0.1),
            settle_rate: settle_rate.max(0.0),
            just_settled: false,
            just_displaced: false,
            enabled: true,
        }
    }

    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.habitat < self.max_habitat;
        self.habitat = (self.habitat + amount).min(self.max_habitat);
        if was_below && self.habitat >= self.max_habitat {
            self.just_settled = true;
        }
    }

    pub fn displace(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.habitat <= 0.0 {
            return;
        }
        self.habitat = (self.habitat - amount).max(0.0);
        if self.habitat <= 0.0 {
            self.just_displaced = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_settled = false;
        self.just_displaced = false;
        if self.enabled && self.settle_rate > 0.0 && self.habitat < self.max_habitat {
            let was_below = self.habitat < self.max_habitat;
            self.habitat = (self.habitat + self.settle_rate * dt).min(self.max_habitat);
            if was_below && self.habitat >= self.max_habitat {
                self.just_settled = true;
            }
        }
    }
}

impl Default for Zooecology {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
