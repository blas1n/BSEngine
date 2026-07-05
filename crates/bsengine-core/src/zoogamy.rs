use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoogamy {
    pub gamete: f32,
    pub max_gamete: f32,
    pub mate_rate: f32,
    pub just_mated: bool,
    pub just_sterile: bool,
    pub enabled: bool,
}

impl Zoogamy {
    pub fn new(max_gamete: f32, mate_rate: f32) -> Self {
        Self {
            gamete: 0.0,
            max_gamete: max_gamete.max(0.1),
            mate_rate: mate_rate.max(0.0),
            just_mated: false,
            just_sterile: false,
            enabled: true,
        }
    }

    pub fn mate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.gamete < self.max_gamete;
        self.gamete = (self.gamete + amount).min(self.max_gamete);
        if was_below && self.gamete >= self.max_gamete {
            self.just_mated = true;
        }
    }

    pub fn sterilize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.gamete <= 0.0 {
            return;
        }
        self.gamete = (self.gamete - amount).max(0.0);
        if self.gamete <= 0.0 {
            self.just_sterile = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_mated = false;
        self.just_sterile = false;
        if self.enabled && self.mate_rate > 0.0 && self.gamete < self.max_gamete {
            let was_below = self.gamete < self.max_gamete;
            self.gamete = (self.gamete + self.mate_rate * dt).min(self.max_gamete);
            if was_below && self.gamete >= self.max_gamete {
                self.just_mated = true;
            }
        }
    }
}

impl Default for Zoogamy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
