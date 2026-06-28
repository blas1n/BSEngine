use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Woodsy {
    pub rusticity: f32,
    pub max_rusticity: f32,
    pub wild_rate: f32,
    pub just_feral: bool,
    pub just_tamed: bool,
    pub enabled: bool,
}

impl Default for Woodsy {
    fn default() -> Self {
        Self {
            rusticity: 0.0,
            max_rusticity: 100.0,
            wild_rate: 1.0,
            just_feral: false,
            just_tamed: false,
            enabled: true,
        }
    }
}

impl Woodsy {
    pub fn go_wild(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_feral = false;
        self.just_tamed = false;
        let prev = self.rusticity;
        self.rusticity = (self.rusticity + amount).clamp(0.0, self.max_rusticity);
        if self.rusticity >= self.max_rusticity && prev < self.max_rusticity {
            self.just_feral = true;
        }
    }

    pub fn tame(&mut self, amount: f32) {
        if !self.enabled || self.rusticity <= 0.0 {
            return;
        }
        self.just_feral = false;
        self.just_tamed = false;
        let prev = self.rusticity;
        self.rusticity = (self.rusticity - amount).max(0.0);
        if self.rusticity <= 0.0 && prev > 0.0 {
            self.just_tamed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.rusticity >= self.max_rusticity {
            return;
        }
        self.go_wild(self.wild_rate * dt);
    }

    pub fn is_feral(&self) -> bool {
        self.enabled && self.rusticity >= self.max_rusticity
    }
    pub fn is_tamed(&self) -> bool {
        self.rusticity <= 0.0
    }

    pub fn rusticity_fraction(&self) -> f32 {
        if self.max_rusticity <= 0.0 {
            return 0.0;
        }
        self.rusticity / self.max_rusticity
    }

    pub fn effective_wild(&self, scale: f32) -> f32 {
        self.rusticity_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn woodsy() -> Woodsy {
        Woodsy {
            rusticity: 0.0,
            max_rusticity: 100.0,
            wild_rate: 10.0,
            just_feral: false,
            just_tamed: false,
            enabled: true,
        }
    }

    #[test]
    fn default_rusticity_zero() {
        assert_eq!(Woodsy::default().rusticity, 0.0);
    }
    #[test]
    fn default_enabled() {
        assert!(Woodsy::default().enabled);
    }
    #[test]
    fn go_wild_increases_rusticity() {
        let mut w = woodsy();
        w.go_wild(30.0);
        assert_eq!(w.rusticity, 30.0);
    }
    #[test]
    fn go_wild_clamps_at_max() {
        let mut w = woodsy();
        w.go_wild(200.0);
        assert_eq!(w.rusticity, 100.0);
    }
    #[test]
    fn go_wild_no_op_when_disabled() {
        let mut w = woodsy();
        w.enabled = false;
        w.go_wild(50.0);
        assert_eq!(w.rusticity, 0.0);
    }
    #[test]
    fn go_wild_sets_just_feral_at_max() {
        let mut w = woodsy();
        w.go_wild(100.0);
        assert!(w.just_feral);
    }
    #[test]
    fn go_wild_no_just_feral_if_already_max() {
        let mut w = woodsy();
        w.rusticity = 100.0;
        w.go_wild(1.0);
        assert!(!w.just_feral);
    }
    #[test]
    fn tame_decreases_rusticity() {
        let mut w = woodsy();
        w.rusticity = 60.0;
        w.tame(20.0);
        assert_eq!(w.rusticity, 40.0);
    }
    #[test]
    fn tame_clamps_at_zero() {
        let mut w = woodsy();
        w.rusticity = 30.0;
        w.tame(200.0);
        assert_eq!(w.rusticity, 0.0);
    }
    #[test]
    fn tame_no_op_when_disabled() {
        let mut w = woodsy();
        w.rusticity = 50.0;
        w.enabled = false;
        w.tame(10.0);
        assert_eq!(w.rusticity, 50.0);
    }
    #[test]
    fn tame_no_op_when_already_tamed() {
        let mut w = woodsy();
        w.tame(10.0);
        assert_eq!(w.rusticity, 0.0);
    }
    #[test]
    fn tame_sets_just_tamed_at_zero() {
        let mut w = woodsy();
        w.rusticity = 10.0;
        w.tame(10.0);
        assert!(w.just_tamed);
    }
    #[test]
    fn tame_no_just_tamed_if_already_zero() {
        let mut w = woodsy();
        w.tame(1.0);
        assert!(!w.just_tamed);
    }
    #[test]
    fn tick_increases_rusticity() {
        let mut w = woodsy();
        w.tick(1.0);
        assert_eq!(w.rusticity, 10.0);
    }
    #[test]
    fn tick_scales_with_dt() {
        let mut w = woodsy();
        w.tick(2.0);
        assert_eq!(w.rusticity, 20.0);
    }
    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = woodsy();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.rusticity, 0.0);
    }
    #[test]
    fn tick_no_op_when_already_feral() {
        let mut w = woodsy();
        w.rusticity = 100.0;
        w.tick(1.0);
        assert_eq!(w.rusticity, 100.0);
    }
    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = woodsy();
        w.wild_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.rusticity, 0.0);
    }
    #[test]
    fn is_feral_true_at_max() {
        let mut w = woodsy();
        w.rusticity = 100.0;
        assert!(w.is_feral());
    }
    #[test]
    fn is_feral_false_below_max() {
        let mut w = woodsy();
        w.rusticity = 50.0;
        assert!(!w.is_feral());
    }
    #[test]
    fn is_feral_false_when_disabled() {
        let mut w = woodsy();
        w.rusticity = 100.0;
        w.enabled = false;
        assert!(!w.is_feral());
    }
    #[test]
    fn is_tamed_true_at_zero() {
        let w = woodsy();
        assert!(w.is_tamed());
    }
    #[test]
    fn is_tamed_false_above_zero() {
        let mut w = woodsy();
        w.rusticity = 1.0;
        assert!(!w.is_tamed());
    }
    #[test]
    fn rusticity_fraction_zero_when_tamed() {
        let w = woodsy();
        assert_eq!(w.rusticity_fraction(), 0.0);
    }
    #[test]
    fn rusticity_fraction_one_at_max() {
        let mut w = woodsy();
        w.rusticity = 100.0;
        assert_eq!(w.rusticity_fraction(), 1.0);
    }
    #[test]
    fn rusticity_fraction_half_at_midpoint() {
        let mut w = woodsy();
        w.rusticity = 50.0;
        assert_eq!(w.rusticity_fraction(), 0.5);
    }
    #[test]
    fn rusticity_fraction_zero_when_max_zero() {
        let mut w = woodsy();
        w.max_rusticity = 0.0;
        assert_eq!(w.rusticity_fraction(), 0.0);
    }
    #[test]
    fn effective_wild_scales() {
        let mut w = woodsy();
        w.rusticity = 50.0;
        assert_eq!(w.effective_wild(2.0), 1.0);
    }
    #[test]
    fn effective_wild_zero_when_tamed() {
        let w = woodsy();
        assert_eq!(w.effective_wild(10.0), 0.0);
    }
    #[test]
    fn just_feral_cleared_on_next_go_wild() {
        let mut w = woodsy();
        w.go_wild(100.0);
        assert!(w.just_feral);
        w.go_wild(1.0);
        assert!(!w.just_feral);
    }
    #[test]
    fn just_tamed_cleared_on_next_tame() {
        let mut w = woodsy();
        w.rusticity = 10.0;
        w.tame(10.0);
        assert!(w.just_tamed);
        w.rusticity = 10.0;
        w.tame(1.0);
        assert!(!w.just_tamed);
    }
}
