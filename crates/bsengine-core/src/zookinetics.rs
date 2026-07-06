use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zookinetics {
    pub momentum: f32,
    pub max_momentum: f32,
    pub accelerate_rate: f32,
    pub just_accelerated: bool,
    pub just_decelerated: bool,
    pub enabled: bool,
}

impl Zookinetics {
    pub fn new(max_momentum: f32, accelerate_rate: f32) -> Self {
        Self {
            momentum: 0.0,
            max_momentum: max_momentum.max(0.1),
            accelerate_rate: accelerate_rate.max(0.0),
            just_accelerated: false,
            just_decelerated: false,
            enabled: true,
        }
    }

    pub fn accelerate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.momentum < self.max_momentum;
        self.momentum = (self.momentum + amount).min(self.max_momentum);
        if was_below && self.momentum >= self.max_momentum {
            self.just_accelerated = true;
        }
    }

    pub fn decelerate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.momentum <= 0.0 {
            return;
        }
        self.momentum = (self.momentum - amount).max(0.0);
        if self.momentum <= 0.0 {
            self.just_decelerated = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_accelerated = false;
        self.just_decelerated = false;
        if self.enabled && self.accelerate_rate > 0.0 && self.momentum < self.max_momentum {
            let was_below = self.momentum < self.max_momentum;
            self.momentum = (self.momentum + self.accelerate_rate * dt).min(self.max_momentum);
            if was_below && self.momentum >= self.max_momentum {
                self.just_accelerated = true;
            }
        }
    }
}

impl Default for Zookinetics {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
