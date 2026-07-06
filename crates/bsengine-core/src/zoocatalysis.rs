use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoocatalysis {
    pub rate: f32,
    pub max_rate: f32,
    pub activate_rate: f32,
    pub just_activated: bool,
    pub just_inhibited: bool,
    pub enabled: bool,
}

impl Zoocatalysis {
    pub fn new(max_rate: f32, activate_rate: f32) -> Self {
        Self {
            rate: 0.0,
            max_rate: max_rate.max(0.1),
            activate_rate: activate_rate.max(0.0),
            just_activated: false,
            just_inhibited: false,
            enabled: true,
        }
    }

    pub fn activate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.rate < self.max_rate;
        self.rate = (self.rate + amount).min(self.max_rate);
        if was_below && self.rate >= self.max_rate {
            self.just_activated = true;
        }
    }

    pub fn inhibit(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.rate <= 0.0 {
            return;
        }
        self.rate = (self.rate - amount).max(0.0);
        if self.rate <= 0.0 {
            self.just_inhibited = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_activated = false;
        self.just_inhibited = false;
        if self.enabled && self.activate_rate > 0.0 && self.rate < self.max_rate {
            let was_below = self.rate < self.max_rate;
            self.rate = (self.rate + self.activate_rate * dt).min(self.max_rate);
            if was_below && self.rate >= self.max_rate {
                self.just_activated = true;
            }
        }
    }
}

impl Default for Zoocatalysis {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
