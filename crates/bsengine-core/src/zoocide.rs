use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoocide {
    pub toll: f32,
    pub max_toll: f32,
    pub kill_rate: f32,
    pub just_exterminated: bool,
    pub just_replenished: bool,
    pub enabled: bool,
}

impl Zoocide {
    pub fn new(max_toll: f32, kill_rate: f32) -> Self {
        Self {
            toll: 0.0,
            max_toll: max_toll.max(0.1),
            kill_rate: kill_rate.max(0.0),
            just_exterminated: false,
            just_replenished: false,
            enabled: true,
        }
    }

    pub fn exterminate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.toll < self.max_toll;
        self.toll = (self.toll + amount).min(self.max_toll);
        if was_below && self.toll >= self.max_toll {
            self.just_exterminated = true;
        }
    }

    pub fn replenish(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.toll <= 0.0 {
            return;
        }
        self.toll = (self.toll - amount).max(0.0);
        if self.toll <= 0.0 {
            self.just_replenished = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_exterminated = false;
        self.just_replenished = false;
        if self.enabled && self.kill_rate > 0.0 && self.toll < self.max_toll {
            let was_below = self.toll < self.max_toll;
            self.toll = (self.toll + self.kill_rate * dt).min(self.max_toll);
            if was_below && self.toll >= self.max_toll {
                self.just_exterminated = true;
            }
        }
    }
}

impl Default for Zoocide {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
