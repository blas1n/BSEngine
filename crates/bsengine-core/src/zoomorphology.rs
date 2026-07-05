use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoomorphology {
    pub form: f32,
    pub max_form: f32,
    pub develop_rate: f32,
    pub just_developed: bool,
    pub just_regressed: bool,
    pub enabled: bool,
}

impl Zoomorphology {
    pub fn new(max_form: f32, develop_rate: f32) -> Self {
        Self {
            form: 0.0,
            max_form: max_form.max(0.1),
            develop_rate: develop_rate.max(0.0),
            just_developed: false,
            just_regressed: false,
            enabled: true,
        }
    }

    pub fn develop(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.form < self.max_form;
        self.form = (self.form + amount).min(self.max_form);
        if was_below && self.form >= self.max_form {
            self.just_developed = true;
        }
    }

    pub fn regress(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.form <= 0.0 {
            return;
        }
        self.form = (self.form - amount).max(0.0);
        if self.form <= 0.0 {
            self.just_regressed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_developed = false;
        self.just_regressed = false;
        if self.enabled && self.develop_rate > 0.0 && self.form < self.max_form {
            let was_below = self.form < self.max_form;
            self.form = (self.form + self.develop_rate * dt).min(self.max_form);
            if was_below && self.form >= self.max_form {
                self.just_developed = true;
            }
        }
    }
}

impl Default for Zoomorphology {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
