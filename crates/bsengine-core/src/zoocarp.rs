use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoocarp {
    pub propagule: f32,
    pub max_propagule: f32,
    pub release_rate: f32,
    pub just_released: bool,
    pub just_aborted: bool,
    pub enabled: bool,
}

impl Zoocarp {
    pub fn new(max_propagule: f32, release_rate: f32) -> Self {
        Self {
            propagule: 0.0,
            max_propagule: max_propagule.max(0.1),
            release_rate: release_rate.max(0.0),
            just_released: false,
            just_aborted: false,
            enabled: true,
        }
    }

    pub fn release(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.propagule < self.max_propagule;
        self.propagule = (self.propagule + amount).min(self.max_propagule);
        if was_below && self.propagule >= self.max_propagule {
            self.just_released = true;
        }
    }

    pub fn abort(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.propagule <= 0.0 {
            return;
        }
        self.propagule = (self.propagule - amount).max(0.0);
        if self.propagule <= 0.0 {
            self.just_aborted = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_released = false;
        self.just_aborted = false;
        if self.enabled && self.release_rate > 0.0 && self.propagule < self.max_propagule {
            let was_below = self.propagule < self.max_propagule;
            self.propagule = (self.propagule + self.release_rate * dt).min(self.max_propagule);
            if was_below && self.propagule >= self.max_propagule {
                self.just_released = true;
            }
        }
    }
}

impl Default for Zoocarp {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
