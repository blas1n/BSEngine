use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoosymbiont {
    pub harmony: f32,
    pub max_harmony: f32,
    pub mutualize_rate: f32,
    pub just_mutualized: bool,
    pub just_expelled: bool,
    pub enabled: bool,
}

impl Zoosymbiont {
    pub fn new(max_harmony: f32, mutualize_rate: f32) -> Self {
        Self {
            harmony: 0.0,
            max_harmony: max_harmony.max(0.1),
            mutualize_rate: mutualize_rate.max(0.0),
            just_mutualized: false,
            just_expelled: false,
            enabled: true,
        }
    }

    pub fn mutualize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.harmony < self.max_harmony;
        self.harmony = (self.harmony + amount).min(self.max_harmony);
        if was_below && self.harmony >= self.max_harmony {
            self.just_mutualized = true;
        }
    }

    pub fn expel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.harmony <= 0.0 {
            return;
        }
        self.harmony = (self.harmony - amount).max(0.0);
        if self.harmony <= 0.0 {
            self.just_expelled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_mutualized = false;
        self.just_expelled = false;
        if self.enabled && self.mutualize_rate > 0.0 && self.harmony < self.max_harmony {
            let was_below = self.harmony < self.max_harmony;
            self.harmony = (self.harmony + self.mutualize_rate * dt).min(self.max_harmony);
            if was_below && self.harmony >= self.max_harmony {
                self.just_mutualized = true;
            }
        }
    }
}

impl Default for Zoosymbiont {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
