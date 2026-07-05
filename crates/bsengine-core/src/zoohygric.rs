use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoohygric {
    pub moisture: f32,
    pub max_moisture: f32,
    pub hydrate_rate: f32,
    pub just_hydrated: bool,
    pub just_desiccated: bool,
    pub enabled: bool,
}

impl Zoohygric {
    pub fn new(max_moisture: f32, hydrate_rate: f32) -> Self {
        Self {
            moisture: 0.0,
            max_moisture: max_moisture.max(0.1),
            hydrate_rate: hydrate_rate.max(0.0),
            just_hydrated: false,
            just_desiccated: false,
            enabled: true,
        }
    }

    pub fn hydrate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.moisture < self.max_moisture;
        self.moisture = (self.moisture + amount).min(self.max_moisture);
        if was_below && self.moisture >= self.max_moisture {
            self.just_hydrated = true;
        }
    }

    pub fn desiccate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.moisture <= 0.0 {
            return;
        }
        self.moisture = (self.moisture - amount).max(0.0);
        if self.moisture <= 0.0 {
            self.just_desiccated = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_hydrated = false;
        self.just_desiccated = false;
        if self.enabled && self.hydrate_rate > 0.0 && self.moisture < self.max_moisture {
            let was_below = self.moisture < self.max_moisture;
            self.moisture = (self.moisture + self.hydrate_rate * dt).min(self.max_moisture);
            if was_below && self.moisture >= self.max_moisture {
                self.just_hydrated = true;
            }
        }
    }
}

impl Default for Zoohygric {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
