use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wasp {
    pub sting: f32,
    pub max_sting: f32,
    pub venom_rate: f32,
    pub just_venomous: bool,
    pub just_spent: bool,
    pub enabled: bool,
}

impl Default for Wasp {
    fn default() -> Self {
        Self {
            sting: 0.0,
            max_sting: 100.0,
            venom_rate: 1.0,
            just_venomous: false,
            just_spent: false,
            enabled: true,
        }
    }
}

impl Wasp {
    pub fn inject(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_venomous = false;
        self.just_spent = false;
        let prev = self.sting;
        self.sting = (self.sting + amount).clamp(0.0, self.max_sting);
        if self.sting >= self.max_sting && prev < self.max_sting {
            self.just_venomous = true;
        }
    }

    pub fn neutralize(&mut self, amount: f32) {
        if !self.enabled || self.sting <= 0.0 {
            return;
        }
        self.just_venomous = false;
        self.just_spent = false;
        let prev = self.sting;
        self.sting = (self.sting - amount).max(0.0);
        if self.sting <= 0.0 && prev > 0.0 {
            self.just_spent = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.sting >= self.max_sting {
            return;
        }
        self.inject(self.venom_rate * dt);
    }

    pub fn is_venomous(&self) -> bool {
        self.enabled && self.sting >= self.max_sting
    }

    pub fn is_spent(&self) -> bool {
        self.sting <= 0.0
    }

    pub fn sting_fraction(&self) -> f32 {
        if self.max_sting <= 0.0 {
            return 0.0;
        }
        self.sting / self.max_sting
    }

    pub fn effective_toxin(&self, scale: f32) -> f32 {
        self.sting_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wasp() -> Wasp {
        Wasp {
            sting: 0.0,
            max_sting: 100.0,
            venom_rate: 10.0,
            just_venomous: false,
            just_spent: false,
            enabled: true,
        }
    }

    #[test]
    fn default_sting_zero() {
        let w = Wasp::default();
        assert_eq!(w.sting, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wasp::default().enabled);
    }

    #[test]
    fn inject_increases_sting() {
        let mut w = wasp();
        w.inject(30.0);
        assert_eq!(w.sting, 30.0);
    }

    #[test]
    fn inject_clamps_at_max() {
        let mut w = wasp();
        w.inject(200.0);
        assert_eq!(w.sting, 100.0);
    }

    #[test]
    fn inject_no_op_when_disabled() {
        let mut w = wasp();
        w.enabled = false;
        w.inject(50.0);
        assert_eq!(w.sting, 0.0);
    }

    #[test]
    fn inject_sets_just_venomous_at_max() {
        let mut w = wasp();
        w.inject(100.0);
        assert!(w.just_venomous);
    }

    #[test]
    fn inject_no_just_venomous_if_already_max() {
        let mut w = wasp();
        w.sting = 100.0;
        w.inject(1.0);
        assert!(!w.just_venomous);
    }

    #[test]
    fn neutralize_decreases_sting() {
        let mut w = wasp();
        w.sting = 60.0;
        w.neutralize(20.0);
        assert_eq!(w.sting, 40.0);
    }

    #[test]
    fn neutralize_clamps_at_zero() {
        let mut w = wasp();
        w.sting = 30.0;
        w.neutralize(200.0);
        assert_eq!(w.sting, 0.0);
    }

    #[test]
    fn neutralize_no_op_when_disabled() {
        let mut w = wasp();
        w.sting = 50.0;
        w.enabled = false;
        w.neutralize(10.0);
        assert_eq!(w.sting, 50.0);
    }

    #[test]
    fn neutralize_no_op_when_already_spent() {
        let mut w = wasp();
        w.neutralize(10.0);
        assert_eq!(w.sting, 0.0);
    }

    #[test]
    fn neutralize_sets_just_spent_at_zero() {
        let mut w = wasp();
        w.sting = 10.0;
        w.neutralize(10.0);
        assert!(w.just_spent);
    }

    #[test]
    fn neutralize_no_just_spent_if_already_zero() {
        let mut w = wasp();
        w.neutralize(1.0);
        assert!(!w.just_spent);
    }

    #[test]
    fn tick_increases_sting() {
        let mut w = wasp();
        w.tick(1.0);
        assert_eq!(w.sting, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wasp();
        w.tick(2.0);
        assert_eq!(w.sting, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wasp();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.sting, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_venomous() {
        let mut w = wasp();
        w.sting = 100.0;
        w.tick(1.0);
        assert_eq!(w.sting, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wasp();
        w.venom_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.sting, 0.0);
    }

    #[test]
    fn is_venomous_true_at_max() {
        let mut w = wasp();
        w.sting = 100.0;
        assert!(w.is_venomous());
    }

    #[test]
    fn is_venomous_false_below_max() {
        let mut w = wasp();
        w.sting = 50.0;
        assert!(!w.is_venomous());
    }

    #[test]
    fn is_venomous_false_when_disabled() {
        let mut w = wasp();
        w.sting = 100.0;
        w.enabled = false;
        assert!(!w.is_venomous());
    }

    #[test]
    fn is_spent_true_at_zero() {
        let w = wasp();
        assert!(w.is_spent());
    }

    #[test]
    fn is_spent_false_above_zero() {
        let mut w = wasp();
        w.sting = 1.0;
        assert!(!w.is_spent());
    }

    #[test]
    fn sting_fraction_zero_when_spent() {
        let w = wasp();
        assert_eq!(w.sting_fraction(), 0.0);
    }

    #[test]
    fn sting_fraction_one_at_max() {
        let mut w = wasp();
        w.sting = 100.0;
        assert_eq!(w.sting_fraction(), 1.0);
    }

    #[test]
    fn sting_fraction_half_at_midpoint() {
        let mut w = wasp();
        w.sting = 50.0;
        assert_eq!(w.sting_fraction(), 0.5);
    }

    #[test]
    fn sting_fraction_zero_when_max_zero() {
        let mut w = wasp();
        w.max_sting = 0.0;
        assert_eq!(w.sting_fraction(), 0.0);
    }

    #[test]
    fn effective_toxin_scales() {
        let mut w = wasp();
        w.sting = 50.0;
        assert_eq!(w.effective_toxin(2.0), 1.0);
    }

    #[test]
    fn effective_toxin_zero_when_spent() {
        let w = wasp();
        assert_eq!(w.effective_toxin(10.0), 0.0);
    }

    #[test]
    fn just_venomous_cleared_on_next_inject() {
        let mut w = wasp();
        w.inject(100.0);
        assert!(w.just_venomous);
        w.inject(1.0);
        assert!(!w.just_venomous);
    }

    #[test]
    fn just_spent_cleared_on_next_neutralize() {
        let mut w = wasp();
        w.sting = 10.0;
        w.neutralize(10.0);
        assert!(w.just_spent);
        w.sting = 10.0;
        w.neutralize(1.0);
        assert!(!w.just_spent);
    }
}
