use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoobenthos {
    pub depth: f32,
    pub max_depth: f32,
    pub sink_rate: f32,
    pub just_settled: bool,
    pub just_suspended: bool,
    pub enabled: bool,
}

impl Zoobenthos {
    pub fn new(max_depth: f32, sink_rate: f32) -> Self {
        Self {
            depth: 0.0,
            max_depth: max_depth.max(0.1),
            sink_rate: sink_rate.max(0.0),
            just_settled: false,
            just_suspended: false,
            enabled: true,
        }
    }

    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.depth < self.max_depth;
        self.depth = (self.depth + amount).min(self.max_depth);
        if was_below && self.depth >= self.max_depth {
            self.just_settled = true;
        }
    }

    pub fn suspend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.depth <= 0.0 {
            return;
        }
        self.depth = (self.depth - amount).max(0.0);
        if self.depth <= 0.0 {
            self.just_suspended = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_settled = false;
        self.just_suspended = false;
        if self.enabled && self.sink_rate > 0.0 && self.depth < self.max_depth {
            let was_below = self.depth < self.max_depth;
            self.depth = (self.depth + self.sink_rate * dt).min(self.max_depth);
            if was_below && self.depth >= self.max_depth {
                self.just_settled = true;
            }
        }
    }
}

impl Default for Zoobenthos {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
