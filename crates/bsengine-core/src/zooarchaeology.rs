use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooarchaeology {
    pub artifact: f32,
    pub max_artifact: f32,
    pub excavate_rate: f32,
    pub just_excavated: bool,
    pub just_buried: bool,
    pub enabled: bool,
}

impl Zooarchaeology {
    pub fn new(max_artifact: f32, excavate_rate: f32) -> Self {
        Self {
            artifact: 0.0,
            max_artifact: max_artifact.max(0.1),
            excavate_rate: excavate_rate.max(0.0),
            just_excavated: false,
            just_buried: false,
            enabled: true,
        }
    }

    pub fn excavate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.artifact < self.max_artifact;
        self.artifact = (self.artifact + amount).min(self.max_artifact);
        if was_below && self.artifact >= self.max_artifact {
            self.just_excavated = true;
        }
    }

    pub fn bury(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.artifact <= 0.0 {
            return;
        }
        self.artifact = (self.artifact - amount).max(0.0);
        if self.artifact <= 0.0 {
            self.just_buried = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_excavated = false;
        self.just_buried = false;
        if self.enabled && self.excavate_rate > 0.0 && self.artifact < self.max_artifact {
            let was_below = self.artifact < self.max_artifact;
            self.artifact = (self.artifact + self.excavate_rate * dt).min(self.max_artifact);
            if was_below && self.artifact >= self.max_artifact {
                self.just_excavated = true;
            }
        }
    }
}

impl Default for Zooarchaeology {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
