use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoocoenosis {
    pub colony: f32,
    pub max_colony: f32,
    pub assemble_rate: f32,
    pub just_assembled: bool,
    pub just_dissolved: bool,
    pub enabled: bool,
}

impl Zoocoenosis {
    pub fn new(max_colony: f32, assemble_rate: f32) -> Self {
        Self {
            colony: 0.0,
            max_colony: max_colony.max(0.1),
            assemble_rate: assemble_rate.max(0.0),
            just_assembled: false,
            just_dissolved: false,
            enabled: true,
        }
    }

    pub fn assemble(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.colony < self.max_colony;
        self.colony = (self.colony + amount).min(self.max_colony);
        if was_below && self.colony >= self.max_colony {
            self.just_assembled = true;
        }
    }

    pub fn dissolve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.colony <= 0.0 {
            return;
        }
        self.colony = (self.colony - amount).max(0.0);
        if self.colony <= 0.0 {
            self.just_dissolved = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_assembled = false;
        self.just_dissolved = false;
        if self.enabled && self.assemble_rate > 0.0 && self.colony < self.max_colony {
            let was_below = self.colony < self.max_colony;
            self.colony = (self.colony + self.assemble_rate * dt).min(self.max_colony);
            if was_below && self.colony >= self.max_colony {
                self.just_assembled = true;
            }
        }
    }
}

impl Default for Zoocoenosis {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
