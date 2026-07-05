use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zootheism {
    pub devotion: f32,
    pub max_devotion: f32,
    pub consecrate_rate: f32,
    pub just_consecrated: bool,
    pub just_desecrated: bool,
    pub enabled: bool,
}

impl Zootheism {
    pub fn new(max_devotion: f32, consecrate_rate: f32) -> Self {
        Self {
            devotion: 0.0,
            max_devotion: max_devotion.max(0.1),
            consecrate_rate: consecrate_rate.max(0.0),
            just_consecrated: false,
            just_desecrated: false,
            enabled: true,
        }
    }

    pub fn consecrate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.devotion < self.max_devotion;
        self.devotion = (self.devotion + amount).min(self.max_devotion);
        if was_below && self.devotion >= self.max_devotion {
            self.just_consecrated = true;
        }
    }

    pub fn desecrate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.devotion <= 0.0 {
            return;
        }
        self.devotion = (self.devotion - amount).max(0.0);
        if self.devotion <= 0.0 {
            self.just_desecrated = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_consecrated = false;
        self.just_desecrated = false;
        if self.enabled && self.consecrate_rate > 0.0 && self.devotion < self.max_devotion {
            let was_below = self.devotion < self.max_devotion;
            self.devotion = (self.devotion + self.consecrate_rate * dt).min(self.max_devotion);
            if was_below && self.devotion >= self.max_devotion {
                self.just_consecrated = true;
            }
        }
    }
}

impl Default for Zootheism {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
