use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoopery {
    pub trial: f32,
    pub max_trial: f32,
    pub probe_rate: f32,
    pub just_probed: bool,
    pub just_halted: bool,
    pub enabled: bool,
}

impl Zoopery {
    pub fn new(max_trial: f32, probe_rate: f32) -> Self {
        Self {
            trial: 0.0,
            max_trial: max_trial.max(0.1),
            probe_rate: probe_rate.max(0.0),
            just_probed: false,
            just_halted: false,
            enabled: true,
        }
    }

    pub fn probe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.trial < self.max_trial;
        self.trial = (self.trial + amount).min(self.max_trial);
        if was_below && self.trial >= self.max_trial {
            self.just_probed = true;
        }
    }

    pub fn halt(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.trial <= 0.0 {
            return;
        }
        self.trial = (self.trial - amount).max(0.0);
        if self.trial <= 0.0 {
            self.just_halted = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_probed = false;
        self.just_halted = false;
        if self.enabled && self.probe_rate > 0.0 && self.trial < self.max_trial {
            let was_below = self.trial < self.max_trial;
            self.trial = (self.trial + self.probe_rate * dt).min(self.max_trial);
            if was_below && self.trial >= self.max_trial {
                self.just_probed = true;
            }
        }
    }
}

impl Default for Zoopery {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
