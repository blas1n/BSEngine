use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoogenic {
    pub vitality: f32,
    pub max_vitality: f32,
    pub generate_rate: f32,
    pub just_generated: bool,
    pub just_suppressed: bool,
    pub enabled: bool,
}

impl Zoogenic {
    pub fn new(max_vitality: f32, generate_rate: f32) -> Self {
        Self {
            vitality: 0.0,
            max_vitality: max_vitality.max(0.1),
            generate_rate: generate_rate.max(0.0),
            just_generated: false,
            just_suppressed: false,
            enabled: true,
        }
    }

    pub fn generate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.vitality < self.max_vitality;
        self.vitality = (self.vitality + amount).min(self.max_vitality);
        if was_below && self.vitality >= self.max_vitality {
            self.just_generated = true;
        }
    }

    pub fn suppress(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.vitality <= 0.0 {
            return;
        }
        self.vitality = (self.vitality - amount).max(0.0);
        if self.vitality <= 0.0 {
            self.just_suppressed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_generated = false;
        self.just_suppressed = false;
        if self.enabled && self.generate_rate > 0.0 && self.vitality < self.max_vitality {
            let was_below = self.vitality < self.max_vitality;
            self.vitality = (self.vitality + self.generate_rate * dt).min(self.max_vitality);
            if was_below && self.vitality >= self.max_vitality {
                self.just_generated = true;
            }
        }
    }
}

impl Default for Zoogenic {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
