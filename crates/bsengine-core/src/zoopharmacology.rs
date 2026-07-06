use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoopharmacology {
    pub potency: f32,
    pub max_potency: f32,
    pub dose_rate: f32,
    pub just_dosed: bool,
    pub just_purged: bool,
    pub enabled: bool,
}

impl Zoopharmacology {
    pub fn new(max_potency: f32, dose_rate: f32) -> Self {
        Self {
            potency: 0.0,
            max_potency: max_potency.max(0.1),
            dose_rate: dose_rate.max(0.0),
            just_dosed: false,
            just_purged: false,
            enabled: true,
        }
    }

    pub fn dose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.potency < self.max_potency;
        self.potency = (self.potency + amount).min(self.max_potency);
        if was_below && self.potency >= self.max_potency {
            self.just_dosed = true;
        }
    }

    pub fn purge(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.potency <= 0.0 {
            return;
        }
        self.potency = (self.potency - amount).max(0.0);
        if self.potency <= 0.0 {
            self.just_purged = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_dosed = false;
        self.just_purged = false;
        if self.enabled && self.dose_rate > 0.0 && self.potency < self.max_potency {
            let was_below = self.potency < self.max_potency;
            self.potency = (self.potency + self.dose_rate * dt).min(self.max_potency);
            if was_below && self.potency >= self.max_potency {
                self.just_dosed = true;
            }
        }
    }
}

impl Default for Zoopharmacology {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
