use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrinkle {
    pub crease: f32,
    pub max_crease: f32,
    pub age_rate: f32,
    pub just_creased: bool,
    pub just_smooth: bool,
    pub enabled: bool,
}

impl Default for Wrinkle {
    fn default() -> Self {
        Self {
            crease: 0.0,
            max_crease: 100.0,
            age_rate: 1.0,
            just_creased: false,
            just_smooth: false,
            enabled: true,
        }
    }
}

impl Wrinkle {
    pub fn age(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_creased = false;
        self.just_smooth = false;
        let prev = self.crease;
        self.crease = (self.crease + amount).clamp(0.0, self.max_crease);
        if self.crease >= self.max_crease && prev < self.max_crease {
            self.just_creased = true;
        }
    }

    pub fn smooth(&mut self, amount: f32) {
        if !self.enabled || self.crease <= 0.0 {
            return;
        }
        self.just_creased = false;
        self.just_smooth = false;
        let prev = self.crease;
        self.crease = (self.crease - amount).max(0.0);
        if self.crease <= 0.0 && prev > 0.0 {
            self.just_smooth = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.crease >= self.max_crease {
            return;
        }
        self.age(self.age_rate * dt);
    }

    pub fn is_creased(&self) -> bool {
        self.enabled && self.crease >= self.max_crease
    }

    pub fn is_smooth(&self) -> bool {
        self.crease <= 0.0
    }

    pub fn crease_fraction(&self) -> f32 {
        if self.max_crease <= 0.0 {
            return 0.0;
        }
        self.crease / self.max_crease
    }

    pub fn effective_wear(&self, scale: f32) -> f32 {
        self.crease_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrinkle() -> Wrinkle {
        Wrinkle {
            crease: 0.0,
            max_crease: 100.0,
            age_rate: 10.0,
            just_creased: false,
            just_smooth: false,
            enabled: true,
        }
    }

    #[test]
    fn default_crease_zero() {
        let w = Wrinkle::default();
        assert_eq!(w.crease, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wrinkle::default().enabled);
    }

    #[test]
    fn age_increases_crease() {
        let mut w = wrinkle();
        w.age(30.0);
        assert_eq!(w.crease, 30.0);
    }

    #[test]
    fn age_clamps_at_max() {
        let mut w = wrinkle();
        w.age(200.0);
        assert_eq!(w.crease, 100.0);
    }

    #[test]
    fn age_no_op_when_disabled() {
        let mut w = wrinkle();
        w.enabled = false;
        w.age(50.0);
        assert_eq!(w.crease, 0.0);
    }

    #[test]
    fn age_sets_just_creased_at_max() {
        let mut w = wrinkle();
        w.age(100.0);
        assert!(w.just_creased);
    }

    #[test]
    fn age_no_just_creased_if_already_max() {
        let mut w = wrinkle();
        w.crease = 100.0;
        w.age(1.0);
        assert!(!w.just_creased);
    }

    #[test]
    fn smooth_decreases_crease() {
        let mut w = wrinkle();
        w.crease = 60.0;
        w.smooth(20.0);
        assert_eq!(w.crease, 40.0);
    }

    #[test]
    fn smooth_clamps_at_zero() {
        let mut w = wrinkle();
        w.crease = 30.0;
        w.smooth(200.0);
        assert_eq!(w.crease, 0.0);
    }

    #[test]
    fn smooth_no_op_when_disabled() {
        let mut w = wrinkle();
        w.crease = 50.0;
        w.enabled = false;
        w.smooth(10.0);
        assert_eq!(w.crease, 50.0);
    }

    #[test]
    fn smooth_no_op_when_already_smooth() {
        let mut w = wrinkle();
        w.smooth(10.0);
        assert_eq!(w.crease, 0.0);
    }

    #[test]
    fn smooth_sets_just_smooth_at_zero() {
        let mut w = wrinkle();
        w.crease = 10.0;
        w.smooth(10.0);
        assert!(w.just_smooth);
    }

    #[test]
    fn smooth_no_just_smooth_if_already_zero() {
        let mut w = wrinkle();
        w.smooth(1.0);
        assert!(!w.just_smooth);
    }

    #[test]
    fn tick_increases_crease() {
        let mut w = wrinkle();
        w.tick(1.0);
        assert_eq!(w.crease, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wrinkle();
        w.tick(2.0);
        assert_eq!(w.crease, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wrinkle();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.crease, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_creased() {
        let mut w = wrinkle();
        w.crease = 100.0;
        w.tick(1.0);
        assert_eq!(w.crease, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wrinkle();
        w.age_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.crease, 0.0);
    }

    #[test]
    fn is_creased_true_at_max() {
        let mut w = wrinkle();
        w.crease = 100.0;
        assert!(w.is_creased());
    }

    #[test]
    fn is_creased_false_below_max() {
        let mut w = wrinkle();
        w.crease = 50.0;
        assert!(!w.is_creased());
    }

    #[test]
    fn is_creased_false_when_disabled() {
        let mut w = wrinkle();
        w.crease = 100.0;
        w.enabled = false;
        assert!(!w.is_creased());
    }

    #[test]
    fn is_smooth_true_at_zero() {
        let w = wrinkle();
        assert!(w.is_smooth());
    }

    #[test]
    fn is_smooth_false_above_zero() {
        let mut w = wrinkle();
        w.crease = 1.0;
        assert!(!w.is_smooth());
    }

    #[test]
    fn crease_fraction_zero_when_smooth() {
        let w = wrinkle();
        assert_eq!(w.crease_fraction(), 0.0);
    }

    #[test]
    fn crease_fraction_one_at_max() {
        let mut w = wrinkle();
        w.crease = 100.0;
        assert_eq!(w.crease_fraction(), 1.0);
    }

    #[test]
    fn crease_fraction_half_at_midpoint() {
        let mut w = wrinkle();
        w.crease = 50.0;
        assert_eq!(w.crease_fraction(), 0.5);
    }

    #[test]
    fn crease_fraction_zero_when_max_zero() {
        let mut w = wrinkle();
        w.max_crease = 0.0;
        assert_eq!(w.crease_fraction(), 0.0);
    }

    #[test]
    fn effective_wear_scales() {
        let mut w = wrinkle();
        w.crease = 50.0;
        assert_eq!(w.effective_wear(2.0), 1.0);
    }

    #[test]
    fn effective_wear_zero_when_smooth() {
        let w = wrinkle();
        assert_eq!(w.effective_wear(10.0), 0.0);
    }

    #[test]
    fn just_creased_cleared_on_next_age() {
        let mut w = wrinkle();
        w.age(100.0);
        assert!(w.just_creased);
        w.age(1.0);
        assert!(!w.just_creased);
    }

    #[test]
    fn just_smooth_cleared_on_next_smooth() {
        let mut w = wrinkle();
        w.crease = 10.0;
        w.smooth(10.0);
        assert!(w.just_smooth);
        w.crease = 10.0;
        w.smooth(1.0);
        assert!(!w.just_smooth);
    }
}
