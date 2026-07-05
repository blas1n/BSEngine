use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoomancy {
    pub portent: f32,
    pub max_portent: f32,
    pub augur_rate: f32,
    pub just_augured: bool,
    pub just_dispelled: bool,
    pub enabled: bool,
}

impl Zoomancy {
    pub fn new(max_portent: f32, augur_rate: f32) -> Self {
        Self {
            portent: 0.0,
            max_portent: max_portent.max(0.1),
            augur_rate: augur_rate.max(0.0),
            just_augured: false,
            just_dispelled: false,
            enabled: true,
        }
    }

    pub fn augur(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.portent < self.max_portent;
        self.portent = (self.portent + amount).min(self.max_portent);
        if was_below && self.portent >= self.max_portent {
            self.just_augured = true;
        }
    }

    pub fn dispel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.portent <= 0.0 {
            return;
        }
        self.portent = (self.portent - amount).max(0.0);
        if self.portent <= 0.0 {
            self.just_dispelled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_augured = false;
        self.just_dispelled = false;
        if self.enabled && self.augur_rate > 0.0 && self.portent < self.max_portent {
            let was_below = self.portent < self.max_portent;
            self.portent = (self.portent + self.augur_rate * dt).min(self.max_portent);
            if was_below && self.portent >= self.max_portent {
                self.just_augured = true;
            }
        }
    }
}

impl Default for Zoomancy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
