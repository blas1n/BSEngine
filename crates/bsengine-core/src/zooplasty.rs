use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooplasty {
    pub graft: f32,
    pub max_graft: f32,
    pub fuse_rate: f32,
    pub just_fused: bool,
    pub just_rejected: bool,
    pub enabled: bool,
}

impl Zooplasty {
    pub fn new(max_graft: f32, fuse_rate: f32) -> Self {
        Self {
            graft: 0.0,
            max_graft: max_graft.max(0.1),
            fuse_rate: fuse_rate.max(0.0),
            just_fused: false,
            just_rejected: false,
            enabled: true,
        }
    }

    pub fn fuse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.graft < self.max_graft;
        self.graft = (self.graft + amount).min(self.max_graft);
        if was_below && self.graft >= self.max_graft {
            self.just_fused = true;
        }
    }

    pub fn reject(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.graft <= 0.0 {
            return;
        }
        self.graft = (self.graft - amount).max(0.0);
        if self.graft <= 0.0 {
            self.just_rejected = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_fused = false;
        self.just_rejected = false;
        if self.enabled && self.fuse_rate > 0.0 && self.graft < self.max_graft {
            let was_below = self.graft < self.max_graft;
            self.graft = (self.graft + self.fuse_rate * dt).min(self.max_graft);
            if was_below && self.graft >= self.max_graft {
                self.just_fused = true;
            }
        }
    }
}

impl Default for Zooplasty {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
