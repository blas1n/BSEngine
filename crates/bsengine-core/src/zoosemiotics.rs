use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoosemiotics {
    pub signal: f32,
    pub max_signal: f32,
    pub transmit_rate: f32,
    pub just_transmitted: bool,
    pub just_silenced: bool,
    pub enabled: bool,
}

impl Zoosemiotics {
    pub fn new(max_signal: f32, transmit_rate: f32) -> Self {
        Self {
            signal: 0.0,
            max_signal: max_signal.max(0.1),
            transmit_rate: transmit_rate.max(0.0),
            just_transmitted: false,
            just_silenced: false,
            enabled: true,
        }
    }

    pub fn transmit(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.signal < self.max_signal;
        self.signal = (self.signal + amount).min(self.max_signal);
        if was_below && self.signal >= self.max_signal {
            self.just_transmitted = true;
        }
    }

    pub fn silence(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.signal <= 0.0 {
            return;
        }
        self.signal = (self.signal - amount).max(0.0);
        if self.signal <= 0.0 {
            self.just_silenced = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_transmitted = false;
        self.just_silenced = false;
        if self.enabled && self.transmit_rate > 0.0 && self.signal < self.max_signal {
            let was_below = self.signal < self.max_signal;
            self.signal = (self.signal + self.transmit_rate * dt).min(self.max_signal);
            if was_below && self.signal >= self.max_signal {
                self.just_transmitted = true;
            }
        }
    }
}

impl Default for Zoosemiotics {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
