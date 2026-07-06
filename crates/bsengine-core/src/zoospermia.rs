use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoospermia {
    pub motility: f32,
    pub max_motility: f32,
    pub energize_rate: f32,
    pub just_energized: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Zoospermia {
    pub fn new(max_motility: f32, energize_rate: f32) -> Self {
        Self {
            motility: 0.0,
            max_motility: max_motility.max(0.1),
            energize_rate: energize_rate.max(0.0),
            just_energized: false,
            just_depleted: false,
            enabled: true,
        }
    }

    pub fn energize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.motility < self.max_motility;
        self.motility = (self.motility + amount).min(self.max_motility);
        if was_below && self.motility >= self.max_motility {
            self.just_energized = true;
        }
    }

    pub fn deplete(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.motility <= 0.0 {
            return;
        }
        self.motility = (self.motility - amount).max(0.0);
        if self.motility <= 0.0 {
            self.just_depleted = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_energized = false;
        self.just_depleted = false;
        if self.enabled && self.energize_rate > 0.0 && self.motility < self.max_motility {
            let was_below = self.motility < self.max_motility;
            self.motility = (self.motility + self.energize_rate * dt).min(self.max_motility);
            if was_below && self.motility >= self.max_motility {
                self.just_energized = true;
            }
        }
    }
}

impl Default for Zoospermia {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
