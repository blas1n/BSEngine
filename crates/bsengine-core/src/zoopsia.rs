use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoopsia {
    pub vision: f32,
    pub max_vision: f32,
    pub perceive_rate: f32,
    pub just_perceived: bool,
    pub just_blinded: bool,
    pub enabled: bool,
}

impl Zoopsia {
    pub fn new(max_vision: f32, perceive_rate: f32) -> Self {
        Self {
            vision: 0.0,
            max_vision: max_vision.max(0.1),
            perceive_rate: perceive_rate.max(0.0),
            just_perceived: false,
            just_blinded: false,
            enabled: true,
        }
    }

    pub fn perceive(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.vision < self.max_vision;
        self.vision = (self.vision + amount).min(self.max_vision);
        if was_below && self.vision >= self.max_vision {
            self.just_perceived = true;
        }
    }

    pub fn blind(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.vision <= 0.0 {
            return;
        }
        self.vision = (self.vision - amount).max(0.0);
        if self.vision <= 0.0 {
            self.just_blinded = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_perceived = false;
        self.just_blinded = false;
        if self.enabled && self.perceive_rate > 0.0 && self.vision < self.max_vision {
            let was_below = self.vision < self.max_vision;
            self.vision = (self.vision + self.perceive_rate * dt).min(self.max_vision);
            if was_below && self.vision >= self.max_vision {
                self.just_perceived = true;
            }
        }
    }
}

impl Default for Zoopsia {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
