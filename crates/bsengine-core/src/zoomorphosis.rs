use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoomorphosis {
    pub form: f32,
    pub max_form: f32,
    pub morph_rate: f32,
    pub just_morphed: bool,
    pub just_reverted: bool,
    pub enabled: bool,
}

impl Zoomorphosis {
    pub fn new(max_form: f32, morph_rate: f32) -> Self {
        Self {
            form: 0.0,
            max_form: max_form.max(0.1),
            morph_rate: morph_rate.max(0.0),
            just_morphed: false,
            just_reverted: false,
            enabled: true,
        }
    }

    pub fn morph(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.form < self.max_form;
        self.form = (self.form + amount).min(self.max_form);
        if was_below && self.form >= self.max_form {
            self.just_morphed = true;
        }
    }

    pub fn revert(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.form <= 0.0 {
            return;
        }
        self.form = (self.form - amount).max(0.0);
        if self.form <= 0.0 {
            self.just_reverted = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_morphed = false;
        self.just_reverted = false;
        if self.enabled && self.morph_rate > 0.0 && self.form < self.max_form {
            let was_below = self.form < self.max_form;
            self.form = (self.form + self.morph_rate * dt).min(self.max_form);
            if was_below && self.form >= self.max_form {
                self.just_morphed = true;
            }
        }
    }
}

impl Default for Zoomorphosis {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
