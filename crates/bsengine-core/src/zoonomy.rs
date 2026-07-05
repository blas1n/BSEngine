use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoonomy {
    pub norm: f32,
    pub max_norm: f32,
    pub regulate_rate: f32,
    pub just_regulated: bool,
    pub just_disrupted: bool,
    pub enabled: bool,
}

impl Zoonomy {
    pub fn new(max_norm: f32, regulate_rate: f32) -> Self {
        Self {
            norm: 0.0,
            max_norm: max_norm.max(0.1),
            regulate_rate: regulate_rate.max(0.0),
            just_regulated: false,
            just_disrupted: false,
            enabled: true,
        }
    }

    pub fn regulate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.norm < self.max_norm;
        self.norm = (self.norm + amount).min(self.max_norm);
        if was_below && self.norm >= self.max_norm {
            self.just_regulated = true;
        }
    }

    pub fn disrupt(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.norm <= 0.0 {
            return;
        }
        self.norm = (self.norm - amount).max(0.0);
        if self.norm <= 0.0 {
            self.just_disrupted = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_regulated = false;
        self.just_disrupted = false;
        if self.enabled && self.regulate_rate > 0.0 && self.norm < self.max_norm {
            let was_below = self.norm < self.max_norm;
            self.norm = (self.norm + self.regulate_rate * dt).min(self.max_norm);
            if was_below && self.norm >= self.max_norm {
                self.just_regulated = true;
            }
        }
    }
}

impl Default for Zoonomy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
