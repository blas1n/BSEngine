use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoopathology {
    pub lesion: f32,
    pub max_lesion: f32,
    pub infect_rate: f32,
    pub just_afflicted: bool,
    pub just_cured: bool,
    pub enabled: bool,
}

impl Zoopathology {
    pub fn new(max_lesion: f32, infect_rate: f32) -> Self {
        Self {
            lesion: 0.0,
            max_lesion: max_lesion.max(0.1),
            infect_rate: infect_rate.max(0.0),
            just_afflicted: false,
            just_cured: false,
            enabled: true,
        }
    }

    pub fn afflict(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.lesion < self.max_lesion;
        self.lesion = (self.lesion + amount).min(self.max_lesion);
        if was_below && self.lesion >= self.max_lesion {
            self.just_afflicted = true;
        }
    }

    pub fn cure(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.lesion <= 0.0 {
            return;
        }
        self.lesion = (self.lesion - amount).max(0.0);
        if self.lesion <= 0.0 {
            self.just_cured = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_afflicted = false;
        self.just_cured = false;
        if self.enabled && self.infect_rate > 0.0 && self.lesion < self.max_lesion {
            let was_below = self.lesion < self.max_lesion;
            self.lesion = (self.lesion + self.infect_rate * dt).min(self.max_lesion);
            if was_below && self.lesion >= self.max_lesion {
                self.just_afflicted = true;
            }
        }
    }
}

impl Default for Zoopathology {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
