use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoochemistry {
    pub compound: f32,
    pub max_compound: f32,
    pub brew_rate: f32,
    pub just_brewed: bool,
    pub just_neutralized: bool,
    pub enabled: bool,
}

impl Zoochemistry {
    pub fn new(max_compound: f32, brew_rate: f32) -> Self {
        Self {
            compound: 0.0,
            max_compound: max_compound.max(0.1),
            brew_rate: brew_rate.max(0.0),
            just_brewed: false,
            just_neutralized: false,
            enabled: true,
        }
    }

    pub fn brew(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.compound < self.max_compound;
        self.compound = (self.compound + amount).min(self.max_compound);
        if was_below && self.compound >= self.max_compound {
            self.just_brewed = true;
        }
    }

    pub fn neutralize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.compound <= 0.0 {
            return;
        }
        self.compound = (self.compound - amount).max(0.0);
        if self.compound <= 0.0 {
            self.just_neutralized = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_brewed = false;
        self.just_neutralized = false;
        if self.enabled && self.brew_rate > 0.0 && self.compound < self.max_compound {
            let was_below = self.compound < self.max_compound;
            self.compound = (self.compound + self.brew_rate * dt).min(self.max_compound);
            if was_below && self.compound >= self.max_compound {
                self.just_brewed = true;
            }
        }
    }
}

impl Default for Zoochemistry {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
