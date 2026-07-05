use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoopsychology {
    pub instinct: f32,
    pub max_instinct: f32,
    pub imprint_rate: f32,
    pub just_imprinted: bool,
    pub just_unlearned: bool,
    pub enabled: bool,
}

impl Zoopsychology {
    pub fn new(max_instinct: f32, imprint_rate: f32) -> Self {
        Self {
            instinct: 0.0,
            max_instinct: max_instinct.max(0.1),
            imprint_rate: imprint_rate.max(0.0),
            just_imprinted: false,
            just_unlearned: false,
            enabled: true,
        }
    }

    pub fn imprint(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.instinct < self.max_instinct;
        self.instinct = (self.instinct + amount).min(self.max_instinct);
        if was_below && self.instinct >= self.max_instinct {
            self.just_imprinted = true;
        }
    }

    pub fn unlearn(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.instinct <= 0.0 {
            return;
        }
        self.instinct = (self.instinct - amount).max(0.0);
        if self.instinct <= 0.0 {
            self.just_unlearned = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_imprinted = false;
        self.just_unlearned = false;
        if self.enabled && self.imprint_rate > 0.0 && self.instinct < self.max_instinct {
            let was_below = self.instinct < self.max_instinct;
            self.instinct = (self.instinct + self.imprint_rate * dt).min(self.max_instinct);
            if was_below && self.instinct >= self.max_instinct {
                self.just_imprinted = true;
            }
        }
    }
}

impl Default for Zoopsychology {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
