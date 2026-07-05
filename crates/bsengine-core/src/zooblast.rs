use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooblast {
    pub blast: f32,
    pub max_blast: f32,
    pub proliferate_rate: f32,
    pub just_proliferated: bool,
    pub just_lysed: bool,
    pub enabled: bool,
}

impl Zooblast {
    pub fn new(max_blast: f32, proliferate_rate: f32) -> Self {
        Self {
            blast: 0.0,
            max_blast: max_blast.max(0.1),
            proliferate_rate: proliferate_rate.max(0.0),
            just_proliferated: false,
            just_lysed: false,
            enabled: true,
        }
    }

    pub fn proliferate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.blast < self.max_blast;
        self.blast = (self.blast + amount).min(self.max_blast);
        if was_below && self.blast >= self.max_blast {
            self.just_proliferated = true;
        }
    }

    pub fn lyse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.blast <= 0.0 {
            return;
        }
        self.blast = (self.blast - amount).max(0.0);
        if self.blast <= 0.0 {
            self.just_lysed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_proliferated = false;
        self.just_lysed = false;
        if self.enabled && self.proliferate_rate > 0.0 && self.blast < self.max_blast {
            let was_below = self.blast < self.max_blast;
            self.blast = (self.blast + self.proliferate_rate * dt).min(self.max_blast);
            if was_below && self.blast >= self.max_blast {
                self.just_proliferated = true;
            }
        }
    }
}

impl Default for Zooblast {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
