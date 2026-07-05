use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoomass {
    pub biomass: f32,
    pub max_biomass: f32,
    pub accumulate_rate: f32,
    pub just_accumulated: bool,
    pub just_decomposed: bool,
    pub enabled: bool,
}

impl Zoomass {
    pub fn new(max_biomass: f32, accumulate_rate: f32) -> Self {
        Self {
            biomass: 0.0,
            max_biomass: max_biomass.max(0.1),
            accumulate_rate: accumulate_rate.max(0.0),
            just_accumulated: false,
            just_decomposed: false,
            enabled: true,
        }
    }

    pub fn accumulate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.biomass < self.max_biomass;
        self.biomass = (self.biomass + amount).min(self.max_biomass);
        if was_below && self.biomass >= self.max_biomass {
            self.just_accumulated = true;
        }
    }

    pub fn decompose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.biomass <= 0.0 {
            return;
        }
        self.biomass = (self.biomass - amount).max(0.0);
        if self.biomass <= 0.0 {
            self.just_decomposed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_accumulated = false;
        self.just_decomposed = false;
        if self.enabled && self.accumulate_rate > 0.0 && self.biomass < self.max_biomass {
            let was_below = self.biomass < self.max_biomass;
            self.biomass = (self.biomass + self.accumulate_rate * dt).min(self.max_biomass);
            if was_below && self.biomass >= self.max_biomass {
                self.just_accumulated = true;
            }
        }
    }
}

impl Default for Zoomass {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
