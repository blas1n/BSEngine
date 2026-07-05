use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooparasite {
    pub host: f32,
    pub max_host: f32,
    pub latch_rate: f32,
    pub just_latched: bool,
    pub just_expelled: bool,
    pub enabled: bool,
}

impl Zooparasite {
    pub fn new(max_host: f32, latch_rate: f32) -> Self {
        Self {
            host: 0.0,
            max_host: max_host.max(0.1),
            latch_rate: latch_rate.max(0.0),
            just_latched: false,
            just_expelled: false,
            enabled: true,
        }
    }

    pub fn latch(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.host < self.max_host;
        self.host = (self.host + amount).min(self.max_host);
        if was_below && self.host >= self.max_host {
            self.just_latched = true;
        }
    }

    pub fn expel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.host <= 0.0 {
            return;
        }
        self.host = (self.host - amount).max(0.0);
        if self.host <= 0.0 {
            self.just_expelled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_latched = false;
        self.just_expelled = false;
        if self.enabled && self.latch_rate > 0.0 && self.host < self.max_host {
            let was_below = self.host < self.max_host;
            self.host = (self.host + self.latch_rate * dt).min(self.max_host);
            if was_below && self.host >= self.max_host {
                self.just_latched = true;
            }
        }
    }
}

impl Default for Zooparasite {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
