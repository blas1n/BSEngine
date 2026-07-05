use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zookinesis {
    pub motion: f32,
    pub max_motion: f32,
    pub mobilize_rate: f32,
    pub just_mobilized: bool,
    pub just_immobilized: bool,
    pub enabled: bool,
}

impl Zookinesis {
    pub fn new(max_motion: f32, mobilize_rate: f32) -> Self {
        Self {
            motion: 0.0,
            max_motion: max_motion.max(0.1),
            mobilize_rate: mobilize_rate.max(0.0),
            just_mobilized: false,
            just_immobilized: false,
            enabled: true,
        }
    }

    pub fn mobilize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.motion < self.max_motion;
        self.motion = (self.motion + amount).min(self.max_motion);
        if was_below && self.motion >= self.max_motion {
            self.just_mobilized = true;
        }
    }

    pub fn immobilize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.motion <= 0.0 {
            return;
        }
        self.motion = (self.motion - amount).max(0.0);
        if self.motion <= 0.0 {
            self.just_immobilized = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_mobilized = false;
        self.just_immobilized = false;
        if self.enabled && self.mobilize_rate > 0.0 && self.motion < self.max_motion {
            let was_below = self.motion < self.max_motion;
            self.motion = (self.motion + self.mobilize_rate * dt).min(self.max_motion);
            if was_below && self.motion >= self.max_motion {
                self.just_mobilized = true;
            }
        }
    }
}

impl Default for Zookinesis {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
