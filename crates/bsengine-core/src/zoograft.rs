use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoograft {
    pub viability: f32,
    pub max_viability: f32,
    pub integrate_rate: f32,
    pub just_integrated: bool,
    pub just_rejected: bool,
    pub enabled: bool,
}

impl Zoograft {
    pub fn new(max_viability: f32, integrate_rate: f32) -> Self {
        Self {
            viability: 0.0,
            max_viability: max_viability.max(0.1),
            integrate_rate: integrate_rate.max(0.0),
            just_integrated: false,
            just_rejected: false,
            enabled: true,
        }
    }

    pub fn integrate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.viability < self.max_viability;
        self.viability = (self.viability + amount).min(self.max_viability);
        if was_below && self.viability >= self.max_viability {
            self.just_integrated = true;
        }
    }

    pub fn reject(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.viability <= 0.0 {
            return;
        }
        self.viability = (self.viability - amount).max(0.0);
        if self.viability <= 0.0 {
            self.just_rejected = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_integrated = false;
        self.just_rejected = false;
        if self.enabled && self.integrate_rate > 0.0 && self.viability < self.max_viability {
            let was_below = self.viability < self.max_viability;
            self.viability = (self.viability + self.integrate_rate * dt).min(self.max_viability);
            if was_below && self.viability >= self.max_viability {
                self.just_integrated = true;
            }
        }
    }
}

impl Default for Zoograft {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
