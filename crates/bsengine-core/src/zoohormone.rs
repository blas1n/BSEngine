use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoohormone {
    pub level: f32,
    pub max_level: f32,
    pub secrete_rate: f32,
    pub just_secreted: bool,
    pub just_suppressed: bool,
    pub enabled: bool,
}

impl Zoohormone {
    pub fn new(max_level: f32, secrete_rate: f32) -> Self {
        Self {
            level: 0.0,
            max_level: max_level.max(0.1),
            secrete_rate: secrete_rate.max(0.0),
            just_secreted: false,
            just_suppressed: false,
            enabled: true,
        }
    }

    pub fn secrete(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.level < self.max_level;
        self.level = (self.level + amount).min(self.max_level);
        if was_below && self.level >= self.max_level {
            self.just_secreted = true;
        }
    }

    pub fn suppress(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.level <= 0.0 {
            return;
        }
        self.level = (self.level - amount).max(0.0);
        if self.level <= 0.0 {
            self.just_suppressed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_secreted = false;
        self.just_suppressed = false;
        if self.enabled && self.secrete_rate > 0.0 && self.level < self.max_level {
            let was_below = self.level < self.max_level;
            self.level = (self.level + self.secrete_rate * dt).min(self.max_level);
            if was_below && self.level >= self.max_level {
                self.just_secreted = true;
            }
        }
    }
}

impl Default for Zoohormone {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
