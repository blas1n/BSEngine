use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoomenology {
    pub phase: f32,
    pub max_phase: f32,
    pub cycle_rate: f32,
    pub just_peaked: bool,
    pub just_troughed: bool,
    pub enabled: bool,
}

impl Zoomenology {
    pub fn new(max_phase: f32, cycle_rate: f32) -> Self {
        Self {
            phase: 0.0,
            max_phase: max_phase.max(0.1),
            cycle_rate: cycle_rate.max(0.0),
            just_peaked: false,
            just_troughed: false,
            enabled: true,
        }
    }

    pub fn peak(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.phase < self.max_phase;
        self.phase = (self.phase + amount).min(self.max_phase);
        if was_below && self.phase >= self.max_phase {
            self.just_peaked = true;
        }
    }

    pub fn trough(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.phase <= 0.0 {
            return;
        }
        self.phase = (self.phase - amount).max(0.0);
        if self.phase <= 0.0 {
            self.just_troughed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_troughed = false;
        if self.enabled && self.cycle_rate > 0.0 && self.phase < self.max_phase {
            let was_below = self.phase < self.max_phase;
            self.phase = (self.phase + self.cycle_rate * dt).min(self.max_phase);
            if was_below && self.phase >= self.max_phase {
                self.just_peaked = true;
            }
        }
    }
}

impl Default for Zoomenology {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
