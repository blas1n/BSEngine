use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooecium {
    pub shell: f32,
    pub max_shell: f32,
    pub encase_rate: f32,
    pub just_encased: bool,
    pub just_exposed: bool,
    pub enabled: bool,
}

impl Zooecium {
    pub fn new(max_shell: f32, encase_rate: f32) -> Self {
        Self {
            shell: 0.0,
            max_shell: max_shell.max(0.1),
            encase_rate: encase_rate.max(0.0),
            just_encased: false,
            just_exposed: false,
            enabled: true,
        }
    }

    pub fn encase(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.shell < self.max_shell;
        self.shell = (self.shell + amount).min(self.max_shell);
        if was_below && self.shell >= self.max_shell {
            self.just_encased = true;
        }
    }

    pub fn expose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.shell <= 0.0 {
            return;
        }
        self.shell = (self.shell - amount).max(0.0);
        if self.shell <= 0.0 {
            self.just_exposed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.just_encased = false;
        self.just_exposed = false;
        if self.enabled && self.encase_rate > 0.0 && self.shell < self.max_shell {
            let was_below = self.shell < self.max_shell;
            self.shell = (self.shell + self.encase_rate * dt).min(self.max_shell);
            if was_below && self.shell >= self.max_shell {
                self.just_encased = true;
            }
        }
    }
}

impl Default for Zooecium {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}
