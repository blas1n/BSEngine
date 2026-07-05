use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoodynamics {
    pub momentum: f32,
    pub max_momentum: f32,
    pub drive_rate: f32,
    pub just_driven: bool,
    pub just_braked: bool,
    pub enabled: bool,
}

impl Zoodynamics {
    pub fn new(max_momentum: f32, drive_rate: f32) -> Self {
        Self {
            momentum: 0.0,
            max_momentum: max_momentum.max(0.1),
            drive_rate: drive_rate.max(0.0),
            just_driven: false,
            just_braked: false,
            enabled: true,
        }
    }

    pub fn drive(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.momentum < self.max_momentum;
        self.momentum = (self.momentum + amount).min(self.max_momentum);
        if was_below && self.momentum >= self.max_momentum {
            self.just_driven = true;
        }
    }

    pub fn brake(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.momentum <= 0.0 {
            return;
        }
        self.momentum = (self.momentum - amount).max(0.0);
        if self.momentum <= 0.0 {
            self.just_braked = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_driven = false;
        self.just_braked = false;
        if self.enabled && self.drive_rate > 0.0 && self.momentum < self.max_momentum {
            let was_below = self.momentum < self.max_momentum;
            self.momentum = (self.momentum + self.drive_rate * dt).min(self.max_momentum);
            if was_below && self.momentum >= self.max_momentum {
                self.just_driven = true;
            }
        }
    }
}

impl Default for Zoodynamics {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
