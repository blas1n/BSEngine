use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoophotism {
    pub luminance: f32,
    pub max_luminance: f32,
    pub illuminate_rate: f32,
    pub just_illuminated: bool,
    pub just_dimmed: bool,
    pub enabled: bool,
}

impl Zoophotism {
    pub fn new(max_luminance: f32, illuminate_rate: f32) -> Self {
        Self {
            luminance: 0.0,
            max_luminance: max_luminance.max(0.1),
            illuminate_rate: illuminate_rate.max(0.0),
            just_illuminated: false,
            just_dimmed: false,
            enabled: true,
        }
    }

    pub fn illuminate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.luminance < self.max_luminance;
        self.luminance = (self.luminance + amount).min(self.max_luminance);
        if was_below && self.luminance >= self.max_luminance {
            self.just_illuminated = true;
        }
    }

    pub fn dim(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.luminance <= 0.0 {
            return;
        }
        self.luminance = (self.luminance - amount).max(0.0);
        if self.luminance <= 0.0 {
            self.just_dimmed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_illuminated = false;
        self.just_dimmed = false;
        if self.enabled && self.illuminate_rate > 0.0 && self.luminance < self.max_luminance {
            let was_below = self.luminance < self.max_luminance;
            self.luminance = (self.luminance + self.illuminate_rate * dt).min(self.max_luminance);
            if was_below && self.luminance >= self.max_luminance {
                self.just_illuminated = true;
            }
        }
    }
}

impl Default for Zoophotism {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
