use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Worst {
    pub extremity: f32,
    pub max_extremity: f32,
    pub worsen_rate: f32,
    pub just_peaked: bool,
    pub just_eased: bool,
    pub enabled: bool,
}

impl Default for Worst {
    fn default() -> Self {
        Self {
            extremity: 0.0,
            max_extremity: 100.0,
            worsen_rate: 1.0,
            just_peaked: false,
            just_eased: false,
            enabled: true,
        }
    }
}

impl Worst {
    pub fn worsen(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_peaked = false;
        self.just_eased = false;
        let prev = self.extremity;
        self.extremity = (self.extremity + amount).clamp(0.0, self.max_extremity);
        if self.extremity >= self.max_extremity && prev < self.max_extremity {
            self.just_peaked = true;
        }
    }

    pub fn ease(&mut self, amount: f32) {
        if !self.enabled || self.extremity <= 0.0 {
            return;
        }
        self.just_peaked = false;
        self.just_eased = false;
        let prev = self.extremity;
        self.extremity = (self.extremity - amount).max(0.0);
        if self.extremity <= 0.0 && prev > 0.0 {
            self.just_eased = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.extremity >= self.max_extremity {
            return;
        }
        self.worsen(self.worsen_rate * dt);
    }

    pub fn is_peaked(&self) -> bool {
        self.enabled && self.extremity >= self.max_extremity
    }
    pub fn is_eased(&self) -> bool {
        self.extremity <= 0.0
    }

    pub fn extremity_fraction(&self) -> f32 {
        if self.max_extremity <= 0.0 {
            return 0.0;
        }
        self.extremity / self.max_extremity
    }

    pub fn effective_severity(&self, scale: f32) -> f32 {
        self.extremity_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inst() -> Worst {
        Worst {
            extremity: 0.0,
            max_extremity: 100.0,
            worsen_rate: 10.0,
            just_peaked: false,
            just_eased: false,
            enabled: true,
        }
    }
    #[test]
    fn default_extremity_zero() {
        assert_eq!(Worst::default().extremity, 0.0);
    }
    #[test]
    fn default_enabled() {
        assert!(Worst::default().enabled);
    }
    #[test]
    fn ADD_increases() {
        let mut w = inst();
        w.worsen(30.0);
        assert_eq!(w.extremity, 30.0);
    }
    #[test]
    fn ADD_clamps_at_max() {
        let mut w = inst();
        w.worsen(200.0);
        assert_eq!(w.extremity, 100.0);
    }
    #[test]
    fn ADD_no_op_when_disabled() {
        let mut w = inst();
        w.enabled = false;
        w.worsen(50.0);
        assert_eq!(w.extremity, 0.0);
    }
    #[test]
    fn ADD_sets_just_peaked_at_max() {
        let mut w = inst();
        w.worsen(100.0);
        assert!(w.just_peaked);
    }
    #[test]
    fn ADD_no_just_peaked_if_already_max() {
        let mut w = inst();
        w.extremity = 100.0;
        w.worsen(1.0);
        assert!(!w.just_peaked);
    }
    #[test]
    fn REMOVE_decreases() {
        let mut w = inst();
        w.extremity = 60.0;
        w.ease(20.0);
        assert_eq!(w.extremity, 40.0);
    }
    #[test]
    fn REMOVE_clamps_at_zero() {
        let mut w = inst();
        w.extremity = 30.0;
        w.ease(200.0);
        assert_eq!(w.extremity, 0.0);
    }
    #[test]
    fn REMOVE_no_op_when_disabled() {
        let mut w = inst();
        w.extremity = 50.0;
        w.enabled = false;
        w.ease(10.0);
        assert_eq!(w.extremity, 50.0);
    }
    #[test]
    fn REMOVE_no_op_when_already_zero() {
        let mut w = inst();
        w.ease(10.0);
        assert_eq!(w.extremity, 0.0);
    }
    #[test]
    fn REMOVE_sets_just_eased_at_zero() {
        let mut w = inst();
        w.extremity = 10.0;
        w.ease(10.0);
        assert!(w.just_eased);
    }
    #[test]
    fn REMOVE_no_just_eased_if_already_zero() {
        let mut w = inst();
        w.ease(1.0);
        assert!(!w.just_eased);
    }
    #[test]
    fn tick_increases() {
        let mut w = inst();
        w.tick(1.0);
        assert_eq!(w.extremity, 10.0);
    }
    #[test]
    fn tick_scales_with_dt() {
        let mut w = inst();
        w.tick(2.0);
        assert_eq!(w.extremity, 20.0);
    }
    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = inst();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.extremity, 0.0);
    }
    #[test]
    fn tick_no_op_at_max() {
        let mut w = inst();
        w.extremity = 100.0;
        w.tick(1.0);
        assert_eq!(w.extremity, 100.0);
    }
    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = inst();
        w.worsen_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.extremity, 0.0);
    }
    #[test]
    fn is_peaked_true_at_max() {
        let mut w = inst();
        w.extremity = 100.0;
        assert!(w.is_peaked());
    }
    #[test]
    fn is_peaked_false_below_max() {
        let mut w = inst();
        w.extremity = 50.0;
        assert!(!w.is_peaked());
    }
    #[test]
    fn is_peaked_false_when_disabled() {
        let mut w = inst();
        w.extremity = 100.0;
        w.enabled = false;
        assert!(!w.is_peaked());
    }
    #[test]
    fn is_eased_true_at_zero() {
        let w = inst();
        assert!(w.is_eased());
    }
    #[test]
    fn is_eased_false_above_zero() {
        let mut w = inst();
        w.extremity = 1.0;
        assert!(!w.is_eased());
    }
    #[test]
    fn extremity_fraction_zero_when_zero() {
        let w = inst();
        assert_eq!(w.extremity_fraction(), 0.0);
    }
    #[test]
    fn extremity_fraction_one_at_max() {
        let mut w = inst();
        w.extremity = 100.0;
        assert_eq!(w.extremity_fraction(), 1.0);
    }
    #[test]
    fn extremity_fraction_half_at_midpoint() {
        let mut w = inst();
        w.extremity = 50.0;
        assert_eq!(w.extremity_fraction(), 0.5);
    }
    #[test]
    fn extremity_fraction_zero_when_max_zero() {
        let mut w = inst();
        w.max_extremity = 0.0;
        assert_eq!(w.extremity_fraction(), 0.0);
    }
    #[test]
    fn effective_severity_scales() {
        let mut w = inst();
        w.extremity = 50.0;
        assert_eq!(w.effective_severity(2.0), 1.0);
    }
    #[test]
    fn effective_severity_zero_when_zero() {
        let w = inst();
        assert_eq!(w.effective_severity(10.0), 0.0);
    }
    #[test]
    fn just_peaked_cleared_on_next_ADD() {
        let mut w = inst();
        w.worsen(100.0);
        assert!(w.just_peaked);
        w.worsen(1.0);
        assert!(!w.just_peaked);
    }
    #[test]
    fn just_eased_cleared_on_next_REMOVE() {
        let mut w = inst();
        w.extremity = 10.0;
        w.ease(10.0);
        assert!(w.just_eased);
        w.extremity = 10.0;
        w.ease(1.0);
        assert!(!w.just_eased);
    }
}
