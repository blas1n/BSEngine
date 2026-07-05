use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zootherapy {
    pub relief: f32,
    pub max_relief: f32,
    pub heal_rate: f32,
    pub just_healed: bool,
    pub just_relapsed: bool,
    pub enabled: bool,
}

impl Zootherapy {
    pub fn new(max_relief: f32, heal_rate: f32) -> Self {
        Self {
            relief: 0.0,
            max_relief: max_relief.max(0.1),
            heal_rate: heal_rate.max(0.0),
            just_healed: false,
            just_relapsed: false,
            enabled: true,
        }
    }

    pub fn heal(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.relief < self.max_relief;
        self.relief = (self.relief + amount).min(self.max_relief);
        if was_below && self.relief >= self.max_relief {
            self.just_healed = true;
        }
    }

    pub fn relapse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.relief <= 0.0 {
            return;
        }
        self.relief = (self.relief - amount).max(0.0);
        if self.relief <= 0.0 {
            self.just_relapsed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_healed = false;
        self.just_relapsed = false;
        if self.enabled && self.heal_rate > 0.0 && self.relief < self.max_relief {
            let was_below = self.relief < self.max_relief;
            self.relief = (self.relief + self.heal_rate * dt).min(self.max_relief);
            if was_below && self.relief >= self.max_relief {
                self.just_healed = true;
            }
        }
    }
}

impl Default for Zootherapy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
