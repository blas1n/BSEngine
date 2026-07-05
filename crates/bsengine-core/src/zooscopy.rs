use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooscopy {
    pub omen: f32,
    pub max_omen: f32,
    pub divine_rate: f32,
    pub just_revealed: bool,
    pub just_obscured: bool,
    pub enabled: bool,
}

impl Zooscopy {
    pub fn new(max_omen: f32, divine_rate: f32) -> Self {
        Self {
            omen: 0.0,
            max_omen: max_omen.max(0.1),
            divine_rate: divine_rate.max(0.0),
            just_revealed: false,
            just_obscured: false,
            enabled: true,
        }
    }

    pub fn divine(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.omen < self.max_omen;
        self.omen = (self.omen + amount).min(self.max_omen);
        if was_below && self.omen >= self.max_omen {
            self.just_revealed = true;
        }
    }

    pub fn obscure(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.omen <= 0.0 {
            return;
        }
        self.omen = (self.omen - amount).max(0.0);
        if self.omen <= 0.0 {
            self.just_obscured = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_revealed = false;
        self.just_obscured = false;
        if self.enabled && self.divine_rate > 0.0 && self.omen < self.max_omen {
            let was_below = self.omen < self.max_omen;
            self.omen = (self.omen + self.divine_rate * dt).min(self.max_omen);
            if was_below && self.omen >= self.max_omen {
                self.just_revealed = true;
            }
        }
    }
}

impl Default for Zooscopy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
