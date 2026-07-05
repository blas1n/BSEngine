use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoognosis {
    pub insight: f32,
    pub max_insight: f32,
    pub diagnose_rate: f32,
    pub just_diagnosed: bool,
    pub just_misled: bool,
    pub enabled: bool,
}

impl Zoognosis {
    pub fn new(max_insight: f32, diagnose_rate: f32) -> Self {
        Self {
            insight: 0.0,
            max_insight: max_insight.max(0.1),
            diagnose_rate: diagnose_rate.max(0.0),
            just_diagnosed: false,
            just_misled: false,
            enabled: true,
        }
    }

    pub fn diagnose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.insight < self.max_insight;
        self.insight = (self.insight + amount).min(self.max_insight);
        if was_below && self.insight >= self.max_insight {
            self.just_diagnosed = true;
        }
    }

    pub fn mislead(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.insight <= 0.0 {
            return;
        }
        self.insight = (self.insight - amount).max(0.0);
        if self.insight <= 0.0 {
            self.just_misled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_diagnosed = false;
        self.just_misled = false;
        if self.enabled && self.diagnose_rate > 0.0 && self.insight < self.max_insight {
            let was_below = self.insight < self.max_insight;
            self.insight = (self.insight + self.diagnose_rate * dt).min(self.max_insight);
            if was_below && self.insight >= self.max_insight {
                self.just_diagnosed = true;
            }
        }
    }
}

impl Default for Zoognosis {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
