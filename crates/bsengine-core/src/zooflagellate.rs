use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooflagellate {
    pub thrust: f32,
    pub max_thrust: f32,
    pub propel_rate: f32,
    pub just_propelled: bool,
    pub just_stalled: bool,
    pub enabled: bool,
}

impl Zooflagellate {
    pub fn new(max_thrust: f32, propel_rate: f32) -> Self {
        Self {
            thrust: 0.0,
            max_thrust: max_thrust.max(0.1),
            propel_rate: propel_rate.max(0.0),
            just_propelled: false,
            just_stalled: false,
            enabled: true,
        }
    }

    pub fn propel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.thrust < self.max_thrust;
        self.thrust = (self.thrust + amount).min(self.max_thrust);
        if was_below && self.thrust >= self.max_thrust {
            self.just_propelled = true;
        }
    }

    pub fn stall(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.thrust <= 0.0 {
            return;
        }
        self.thrust = (self.thrust - amount).max(0.0);
        if self.thrust <= 0.0 {
            self.just_stalled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_propelled = false;
        self.just_stalled = false;
        if self.enabled && self.propel_rate > 0.0 && self.thrust < self.max_thrust {
            let was_below = self.thrust < self.max_thrust;
            self.thrust = (self.thrust + self.propel_rate * dt).min(self.max_thrust);
            if was_below && self.thrust >= self.max_thrust {
                self.just_propelled = true;
            }
        }
    }
}

impl Default for Zooflagellate {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
