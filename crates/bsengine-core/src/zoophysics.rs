use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoophysics {
    pub force: f32,
    pub max_force: f32,
    pub channel_rate: f32,
    pub just_channeled: bool,
    pub just_scattered: bool,
    pub enabled: bool,
}

impl Zoophysics {
    pub fn new(max_force: f32, channel_rate: f32) -> Self {
        Self {
            force: 0.0,
            max_force: max_force.max(0.1),
            channel_rate: channel_rate.max(0.0),
            just_channeled: false,
            just_scattered: false,
            enabled: true,
        }
    }

    pub fn channel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.force < self.max_force;
        self.force = (self.force + amount).min(self.max_force);
        if was_below && self.force >= self.max_force {
            self.just_channeled = true;
        }
    }

    pub fn scatter(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.force <= 0.0 {
            return;
        }
        self.force = (self.force - amount).max(0.0);
        if self.force <= 0.0 {
            self.just_scattered = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_channeled = false;
        self.just_scattered = false;
        if self.enabled && self.channel_rate > 0.0 && self.force < self.max_force {
            let was_below = self.force < self.max_force;
            self.force = (self.force + self.channel_rate * dt).min(self.max_force);
            if was_below && self.force >= self.max_force {
                self.just_channeled = true;
            }
        }
    }
}

impl Default for Zoophysics {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
