use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoolite {
    pub fossil: f32,
    pub max_fossil: f32,
    pub petrify_rate: f32,
    pub just_petrified: bool,
    pub just_eroded: bool,
    pub enabled: bool,
}

impl Zoolite {
    pub fn new(max_fossil: f32, petrify_rate: f32) -> Self {
        Self {
            fossil: 0.0,
            max_fossil: max_fossil.max(0.1),
            petrify_rate: petrify_rate.max(0.0),
            just_petrified: false,
            just_eroded: false,
            enabled: true,
        }
    }

    pub fn petrify(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.fossil < self.max_fossil;
        self.fossil = (self.fossil + amount).min(self.max_fossil);
        if was_below && self.fossil >= self.max_fossil {
            self.just_petrified = true;
        }
    }

    pub fn erode(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fossil <= 0.0 {
            return;
        }
        self.fossil = (self.fossil - amount).max(0.0);
        if self.fossil <= 0.0 {
            self.just_eroded = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_petrified = false;
        self.just_eroded = false;
        if self.enabled && self.petrify_rate > 0.0 && self.fossil < self.max_fossil {
            let was_below = self.fossil < self.max_fossil;
            self.fossil = (self.fossil + self.petrify_rate * dt).min(self.max_fossil);
            if was_below && self.fossil >= self.max_fossil {
                self.just_petrified = true;
            }
        }
    }
}

impl Default for Zoolite {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
