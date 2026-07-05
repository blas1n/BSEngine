use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooplasma {
    pub density: f32,
    pub max_density: f32,
    pub condense_rate: f32,
    pub just_condensed: bool,
    pub just_dispersed: bool,
    pub enabled: bool,
}

impl Zooplasma {
    pub fn new(max_density: f32, condense_rate: f32) -> Self {
        Self {
            density: 0.0,
            max_density: max_density.max(0.1),
            condense_rate: condense_rate.max(0.0),
            just_condensed: false,
            just_dispersed: false,
            enabled: true,
        }
    }

    pub fn condense(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.density < self.max_density;
        self.density = (self.density + amount).min(self.max_density);
        if was_below && self.density >= self.max_density {
            self.just_condensed = true;
        }
    }

    pub fn disperse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.density <= 0.0 {
            return;
        }
        self.density = (self.density - amount).max(0.0);
        if self.density <= 0.0 {
            self.just_dispersed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_condensed = false;
        self.just_dispersed = false;
        if self.enabled && self.condense_rate > 0.0 && self.density < self.max_density {
            let was_below = self.density < self.max_density;
            self.density = (self.density + self.condense_rate * dt).min(self.max_density);
            if was_below && self.density >= self.max_density {
                self.just_condensed = true;
            }
        }
    }
}

impl Default for Zooplasma {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
