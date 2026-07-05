use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoodeme {
    pub population: f32,
    pub max_population: f32,
    pub aggregate_rate: f32,
    pub just_aggregated: bool,
    pub just_dispersed: bool,
    pub enabled: bool,
}

impl Zoodeme {
    pub fn new(max_population: f32, aggregate_rate: f32) -> Self {
        Self {
            population: 0.0,
            max_population: max_population.max(0.1),
            aggregate_rate: aggregate_rate.max(0.0),
            just_aggregated: false,
            just_dispersed: false,
            enabled: true,
        }
    }

    pub fn aggregate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.population < self.max_population;
        self.population = (self.population + amount).min(self.max_population);
        if was_below && self.population >= self.max_population {
            self.just_aggregated = true;
        }
    }

    pub fn disperse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.population <= 0.0 {
            return;
        }
        self.population = (self.population - amount).max(0.0);
        if self.population <= 0.0 {
            self.just_dispersed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_aggregated = false;
        self.just_dispersed = false;
        if self.enabled && self.aggregate_rate > 0.0 && self.population < self.max_population {
            let was_below = self.population < self.max_population;
            self.population = (self.population + self.aggregate_rate * dt).min(self.max_population);
            if was_below && self.population >= self.max_population {
                self.just_aggregated = true;
            }
        }
    }
}

impl Default for Zoodeme {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
