use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoochromy {
    pub pigment: f32,
    pub max_pigment: f32,
    pub tint_rate: f32,
    pub just_tinted: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Zoochromy {
    pub fn new(max_pigment: f32, tint_rate: f32) -> Self {
        Self {
            pigment: 0.0,
            max_pigment: max_pigment.max(0.1),
            tint_rate: tint_rate.max(0.0),
            just_tinted: false,
            just_faded: false,
            enabled: true,
        }
    }

    pub fn tint(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.pigment < self.max_pigment;
        self.pigment = (self.pigment + amount).min(self.max_pigment);
        if was_below && self.pigment >= self.max_pigment {
            self.just_tinted = true;
        }
    }

    pub fn fade(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pigment <= 0.0 {
            return;
        }
        self.pigment = (self.pigment - amount).max(0.0);
        if self.pigment <= 0.0 {
            self.just_faded = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_tinted = false;
        self.just_faded = false;
        if self.enabled && self.tint_rate > 0.0 && self.pigment < self.max_pigment {
            let was_below = self.pigment < self.max_pigment;
            self.pigment = (self.pigment + self.tint_rate * dt).min(self.max_pigment);
            if was_below && self.pigment >= self.max_pigment {
                self.just_tinted = true;
            }
        }
    }
}

impl Default for Zoochromy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
