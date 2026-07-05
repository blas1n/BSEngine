use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoosociology {
    pub bond: f32,
    pub max_bond: f32,
    pub affiliate_rate: f32,
    pub just_affiliated: bool,
    pub just_severed: bool,
    pub enabled: bool,
}

impl Zoosociology {
    pub fn new(max_bond: f32, affiliate_rate: f32) -> Self {
        Self {
            bond: 0.0,
            max_bond: max_bond.max(0.1),
            affiliate_rate: affiliate_rate.max(0.0),
            just_affiliated: false,
            just_severed: false,
            enabled: true,
        }
    }

    pub fn affiliate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.bond < self.max_bond;
        self.bond = (self.bond + amount).min(self.max_bond);
        if was_below && self.bond >= self.max_bond {
            self.just_affiliated = true;
        }
    }

    pub fn sever(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.bond <= 0.0 {
            return;
        }
        self.bond = (self.bond - amount).max(0.0);
        if self.bond <= 0.0 {
            self.just_severed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_affiliated = false;
        self.just_severed = false;
        if self.enabled && self.affiliate_rate > 0.0 && self.bond < self.max_bond {
            let was_below = self.bond < self.max_bond;
            self.bond = (self.bond + self.affiliate_rate * dt).min(self.max_bond);
            if was_below && self.bond >= self.max_bond {
                self.just_affiliated = true;
            }
        }
    }
}

impl Default for Zoosociology {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
