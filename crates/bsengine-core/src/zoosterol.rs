use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoosterol {
    pub sterol: f32,
    pub max_sterol: f32,
    pub synthesize_rate: f32,
    pub just_synthesized: bool,
    pub just_catabolized: bool,
    pub enabled: bool,
}

impl Zoosterol {
    pub fn new(max_sterol: f32, synthesize_rate: f32) -> Self {
        Self {
            sterol: 0.0,
            max_sterol: max_sterol.max(0.1),
            synthesize_rate: synthesize_rate.max(0.0),
            just_synthesized: false,
            just_catabolized: false,
            enabled: true,
        }
    }

    pub fn synthesize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.sterol < self.max_sterol;
        self.sterol = (self.sterol + amount).min(self.max_sterol);
        if was_below && self.sterol >= self.max_sterol {
            self.just_synthesized = true;
        }
    }

    pub fn catabolize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.sterol <= 0.0 {
            return;
        }
        self.sterol = (self.sterol - amount).max(0.0);
        if self.sterol <= 0.0 {
            self.just_catabolized = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_synthesized = false;
        self.just_catabolized = false;
        if self.enabled && self.synthesize_rate > 0.0 && self.sterol < self.max_sterol {
            let was_below = self.sterol < self.max_sterol;
            self.sterol = (self.sterol + self.synthesize_rate * dt).min(self.max_sterol);
            if was_below && self.sterol >= self.max_sterol {
                self.just_synthesized = true;
            }
        }
    }
}

impl Default for Zoosterol {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
