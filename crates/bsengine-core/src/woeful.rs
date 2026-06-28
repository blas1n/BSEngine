use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Woeful {
    pub sorrow: f32,
    pub max_sorrow: f32,
    pub grieve_rate: f32,
    pub just_grieving: bool,
    pub just_eased: bool,
    pub enabled: bool,
}

impl Default for Woeful {
    fn default() -> Self {
        Self {
            sorrow: 0.0,
            max_sorrow: 100.0,
            grieve_rate: 1.0,
            just_grieving: false,
            just_eased: false,
            enabled: true,
        }
    }
}

impl Woeful {
    pub fn grieve(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_grieving = false;
        self.just_eased = false;
        let prev = self.sorrow;
        self.sorrow = (self.sorrow + amount).clamp(0.0, self.max_sorrow);
        if self.sorrow >= self.max_sorrow && prev < self.max_sorrow {
            self.just_grieving = true;
        }
    }

    pub fn ease(&mut self, amount: f32) {
        if !self.enabled || self.sorrow <= 0.0 {
            return;
        }
        self.just_grieving = false;
        self.just_eased = false;
        let prev = self.sorrow;
        self.sorrow = (self.sorrow - amount).max(0.0);
        if self.sorrow <= 0.0 && prev > 0.0 {
            self.just_eased = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.sorrow >= self.max_sorrow {
            return;
        }
        self.grieve(self.grieve_rate * dt);
    }

    pub fn is_grieving(&self) -> bool {
        self.enabled && self.sorrow >= self.max_sorrow
    }

    pub fn is_eased(&self) -> bool {
        self.sorrow <= 0.0
    }

    pub fn sorrow_fraction(&self) -> f32 {
        if self.max_sorrow <= 0.0 {
            return 0.0;
        }
        self.sorrow / self.max_sorrow
    }

    pub fn effective_misery(&self, scale: f32) -> f32 {
        self.sorrow_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn woeful() -> Woeful {
        Woeful {
            sorrow: 0.0,
            max_sorrow: 100.0,
            grieve_rate: 10.0,
            just_grieving: false,
            just_eased: false,
            enabled: true,
        }
    }

    #[test]
    fn default_sorrow_zero() {
        let w = Woeful::default();
        assert_eq!(w.sorrow, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Woeful::default().enabled);
    }

    #[test]
    fn grieve_increases_sorrow() {
        let mut w = woeful();
        w.grieve(30.0);
        assert_eq!(w.sorrow, 30.0);
    }

    #[test]
    fn grieve_clamps_at_max() {
        let mut w = woeful();
        w.grieve(200.0);
        assert_eq!(w.sorrow, 100.0);
    }

    #[test]
    fn grieve_no_op_when_disabled() {
        let mut w = woeful();
        w.enabled = false;
        w.grieve(50.0);
        assert_eq!(w.sorrow, 0.0);
    }

    #[test]
    fn grieve_sets_just_grieving_at_max() {
        let mut w = woeful();
        w.grieve(100.0);
        assert!(w.just_grieving);
    }

    #[test]
    fn grieve_no_just_grieving_if_already_max() {
        let mut w = woeful();
        w.sorrow = 100.0;
        w.grieve(1.0);
        assert!(!w.just_grieving);
    }

    #[test]
    fn ease_decreases_sorrow() {
        let mut w = woeful();
        w.sorrow = 60.0;
        w.ease(20.0);
        assert_eq!(w.sorrow, 40.0);
    }

    #[test]
    fn ease_clamps_at_zero() {
        let mut w = woeful();
        w.sorrow = 30.0;
        w.ease(200.0);
        assert_eq!(w.sorrow, 0.0);
    }

    #[test]
    fn ease_no_op_when_disabled() {
        let mut w = woeful();
        w.sorrow = 50.0;
        w.enabled = false;
        w.ease(10.0);
        assert_eq!(w.sorrow, 50.0);
    }

    #[test]
    fn ease_no_op_when_already_eased() {
        let mut w = woeful();
        w.ease(10.0);
        assert_eq!(w.sorrow, 0.0);
    }

    #[test]
    fn ease_sets_just_eased_at_zero() {
        let mut w = woeful();
        w.sorrow = 10.0;
        w.ease(10.0);
        assert!(w.just_eased);
    }

    #[test]
    fn ease_no_just_eased_if_already_zero() {
        let mut w = woeful();
        w.ease(1.0);
        assert!(!w.just_eased);
    }

    #[test]
    fn tick_increases_sorrow() {
        let mut w = woeful();
        w.tick(1.0);
        assert_eq!(w.sorrow, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = woeful();
        w.tick(2.0);
        assert_eq!(w.sorrow, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = woeful();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.sorrow, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_grieving() {
        let mut w = woeful();
        w.sorrow = 100.0;
        w.tick(1.0);
        assert_eq!(w.sorrow, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = woeful();
        w.grieve_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.sorrow, 0.0);
    }

    #[test]
    fn is_grieving_true_at_max() {
        let mut w = woeful();
        w.sorrow = 100.0;
        assert!(w.is_grieving());
    }

    #[test]
    fn is_grieving_false_below_max() {
        let mut w = woeful();
        w.sorrow = 50.0;
        assert!(!w.is_grieving());
    }

    #[test]
    fn is_grieving_false_when_disabled() {
        let mut w = woeful();
        w.sorrow = 100.0;
        w.enabled = false;
        assert!(!w.is_grieving());
    }

    #[test]
    fn is_eased_true_at_zero() {
        let w = woeful();
        assert!(w.is_eased());
    }

    #[test]
    fn is_eased_false_above_zero() {
        let mut w = woeful();
        w.sorrow = 1.0;
        assert!(!w.is_eased());
    }

    #[test]
    fn sorrow_fraction_zero_when_eased() {
        let w = woeful();
        assert_eq!(w.sorrow_fraction(), 0.0);
    }

    #[test]
    fn sorrow_fraction_one_at_max() {
        let mut w = woeful();
        w.sorrow = 100.0;
        assert_eq!(w.sorrow_fraction(), 1.0);
    }

    #[test]
    fn sorrow_fraction_half_at_midpoint() {
        let mut w = woeful();
        w.sorrow = 50.0;
        assert_eq!(w.sorrow_fraction(), 0.5);
    }

    #[test]
    fn sorrow_fraction_zero_when_max_zero() {
        let mut w = woeful();
        w.max_sorrow = 0.0;
        assert_eq!(w.sorrow_fraction(), 0.0);
    }

    #[test]
    fn effective_misery_scales() {
        let mut w = woeful();
        w.sorrow = 50.0;
        assert_eq!(w.effective_misery(2.0), 1.0);
    }

    #[test]
    fn effective_misery_zero_when_eased() {
        let w = woeful();
        assert_eq!(w.effective_misery(10.0), 0.0);
    }

    #[test]
    fn just_grieving_cleared_on_next_grieve() {
        let mut w = woeful();
        w.grieve(100.0);
        assert!(w.just_grieving);
        w.grieve(1.0);
        assert!(!w.just_grieving);
    }

    #[test]
    fn just_eased_cleared_on_next_ease() {
        let mut w = woeful();
        w.sorrow = 10.0;
        w.ease(10.0);
        assert!(w.just_eased);
        w.sorrow = 10.0;
        w.ease(1.0);
        assert!(!w.just_eased);
    }
}
