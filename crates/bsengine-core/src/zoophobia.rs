use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoophobia {
    pub dread: f32,
    pub max_dread: f32,
    pub fear_rate: f32,
    pub just_panicked: bool,
    pub just_calmed: bool,
    pub enabled: bool,
}

impl Zoophobia {
    pub fn new(max_dread: f32, fear_rate: f32) -> Self {
        Self {
            dread: 0.0,
            max_dread: max_dread.max(0.1),
            fear_rate: fear_rate.max(0.0),
            just_panicked: false,
            just_calmed: false,
            enabled: true,
        }
    }

    pub fn frighten(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.dread < self.max_dread;
        self.dread = (self.dread + amount).min(self.max_dread);
        if was_below && self.dread >= self.max_dread {
            self.just_panicked = true;
        }
    }

    pub fn soothe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.dread <= 0.0 {
            return;
        }
        self.dread = (self.dread - amount).max(0.0);
        if self.dread <= 0.0 {
            self.just_calmed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_panicked = false;
        self.just_calmed = false;
        if self.enabled && self.fear_rate > 0.0 && self.dread < self.max_dread {
            let was_below = self.dread < self.max_dread;
            self.dread = (self.dread + self.fear_rate * dt).min(self.max_dread);
            if was_below && self.dread >= self.max_dread {
                self.just_panicked = true;
            }
        }
    }
}

impl Default for Zoophobia {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
