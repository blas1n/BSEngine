use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoogenesis {
    pub genesis: f32,
    pub max_genesis: f32,
    pub emerge_rate: f32,
    pub just_emerged: bool,
    pub just_extinct: bool,
    pub enabled: bool,
}

impl Zoogenesis {
    pub fn new(max_genesis: f32, emerge_rate: f32) -> Self {
        Self {
            genesis: 0.0,
            max_genesis: max_genesis.max(0.1),
            emerge_rate: emerge_rate.max(0.0),
            just_emerged: false,
            just_extinct: false,
            enabled: true,
        }
    }

    pub fn emerge(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.genesis < self.max_genesis;
        self.genesis = (self.genesis + amount).min(self.max_genesis);
        if was_below && self.genesis >= self.max_genesis {
            self.just_emerged = true;
        }
    }

    pub fn extinguish(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.genesis <= 0.0 {
            return;
        }
        self.genesis = (self.genesis - amount).max(0.0);
        if self.genesis <= 0.0 {
            self.just_extinct = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_emerged = false;
        self.just_extinct = false;
        if self.enabled && self.emerge_rate > 0.0 && self.genesis < self.max_genesis {
            let was_below = self.genesis < self.max_genesis;
            self.genesis = (self.genesis + self.emerge_rate * dt).min(self.max_genesis);
            if was_below && self.genesis >= self.max_genesis {
                self.just_emerged = true;
            }
        }
    }
}

impl Default for Zoogenesis {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
