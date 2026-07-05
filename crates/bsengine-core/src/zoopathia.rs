use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoopathia {
    pub malady: f32,
    pub max_malady: f32,
    pub ail_rate: f32,
    pub just_ailed: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Zoopathia {
    pub fn new(max_malady: f32, ail_rate: f32) -> Self {
        Self {
            malady: 0.0,
            max_malady: max_malady.max(0.1),
            ail_rate: ail_rate.max(0.0),
            just_ailed: false,
            just_recovered: false,
            enabled: true,
        }
    }

    pub fn ail(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.malady < self.max_malady;
        self.malady = (self.malady + amount).min(self.max_malady);
        if was_below && self.malady >= self.max_malady {
            self.just_ailed = true;
        }
    }

    pub fn recover(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.malady <= 0.0 {
            return;
        }
        self.malady = (self.malady - amount).max(0.0);
        if self.malady <= 0.0 {
            self.just_recovered = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_ailed = false;
        self.just_recovered = false;
        if self.enabled && self.ail_rate > 0.0 && self.malady < self.max_malady {
            let was_below = self.malady < self.max_malady;
            self.malady = (self.malady + self.ail_rate * dt).min(self.max_malady);
            if was_below && self.malady >= self.max_malady {
                self.just_ailed = true;
            }
        }
    }
}

impl Default for Zoopathia {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
