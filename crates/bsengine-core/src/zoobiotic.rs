use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoobiotic {
    pub parasite: f32,
    pub max_parasite: f32,
    pub infest_rate: f32,
    pub just_infested: bool,
    pub just_cleansed: bool,
    pub enabled: bool,
}

impl Zoobiotic {
    pub fn new(max_parasite: f32, infest_rate: f32) -> Self {
        Self {
            parasite: 0.0,
            max_parasite: max_parasite.max(0.1),
            infest_rate: infest_rate.max(0.0),
            just_infested: false,
            just_cleansed: false,
            enabled: true,
        }
    }

    pub fn infest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.parasite < self.max_parasite;
        self.parasite = (self.parasite + amount).min(self.max_parasite);
        if was_below && self.parasite >= self.max_parasite {
            self.just_infested = true;
        }
    }

    pub fn cleanse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.parasite <= 0.0 {
            return;
        }
        self.parasite = (self.parasite - amount).max(0.0);
        if self.parasite <= 0.0 {
            self.just_cleansed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_infested = false;
        self.just_cleansed = false;
        if self.enabled && self.infest_rate > 0.0 && self.parasite < self.max_parasite {
            let was_below = self.parasite < self.max_parasite;
            self.parasite = (self.parasite + self.infest_rate * dt).min(self.max_parasite);
            if was_below && self.parasite >= self.max_parasite {
                self.just_infested = true;
            }
        }
    }
}

impl Default for Zoobiotic {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
