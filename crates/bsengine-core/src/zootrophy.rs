use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zootrophy {
    pub nutrient: f32,
    pub max_nutrient: f32,
    pub feed_rate: f32,
    pub just_fed: bool,
    pub just_starved: bool,
    pub enabled: bool,
}

impl Zootrophy {
    pub fn new(max_nutrient: f32, feed_rate: f32) -> Self {
        Self {
            nutrient: 0.0,
            max_nutrient: max_nutrient.max(0.1),
            feed_rate: feed_rate.max(0.0),
            just_fed: false,
            just_starved: false,
            enabled: true,
        }
    }

    pub fn feed(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.nutrient < self.max_nutrient;
        self.nutrient = (self.nutrient + amount).min(self.max_nutrient);
        if was_below && self.nutrient >= self.max_nutrient {
            self.just_fed = true;
        }
    }

    pub fn starve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.nutrient <= 0.0 {
            return;
        }
        self.nutrient = (self.nutrient - amount).max(0.0);
        if self.nutrient <= 0.0 {
            self.just_starved = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_fed = false;
        self.just_starved = false;
        if self.enabled && self.feed_rate > 0.0 && self.nutrient < self.max_nutrient {
            let was_below = self.nutrient < self.max_nutrient;
            self.nutrient = (self.nutrient + self.feed_rate * dt).min(self.max_nutrient);
            if was_below && self.nutrient >= self.max_nutrient {
                self.just_fed = true;
            }
        }
    }
}

impl Default for Zootrophy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
