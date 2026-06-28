use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Welp {
    pub distress: f32,
    pub max_distress: f32,
    pub panic_rate: f32,
    pub just_overwhelmed: bool,
    pub just_calm: bool,
    pub enabled: bool,
}

impl Default for Welp {
    fn default() -> Self {
        Self {
            distress: 0.0,
            max_distress: 100.0,
            panic_rate: 1.0,
            just_overwhelmed: false,
            just_calm: false,
            enabled: true,
        }
    }
}

impl Welp {
    pub fn panic(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_overwhelmed = false;
        self.just_calm = false;
        let prev = self.distress;
        self.distress = (self.distress + amount).clamp(0.0, self.max_distress);
        if self.distress >= self.max_distress && prev < self.max_distress {
            self.just_overwhelmed = true;
        }
    }

    pub fn calm(&mut self, amount: f32) {
        if !self.enabled || self.distress <= 0.0 {
            return;
        }
        self.just_overwhelmed = false;
        self.just_calm = false;
        let prev = self.distress;
        self.distress = (self.distress - amount).max(0.0);
        if self.distress <= 0.0 && prev > 0.0 {
            self.just_calm = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.distress >= self.max_distress {
            return;
        }
        self.panic(self.panic_rate * dt);
    }

    pub fn is_overwhelmed(&self) -> bool {
        self.enabled && self.distress >= self.max_distress
    }

    pub fn is_calm(&self) -> bool {
        self.distress <= 0.0
    }

    pub fn distress_fraction(&self) -> f32 {
        if self.max_distress <= 0.0 {
            return 0.0;
        }
        self.distress / self.max_distress
    }

    pub fn effective_panic(&self, scale: f32) -> f32 {
        self.distress_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn welp() -> Welp {
        Welp {
            distress: 0.0,
            max_distress: 100.0,
            panic_rate: 10.0,
            just_overwhelmed: false,
            just_calm: false,
            enabled: true,
        }
    }

    #[test]
    fn default_distress_zero() {
        let w = Welp::default();
        assert_eq!(w.distress, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Welp::default().enabled);
    }

    #[test]
    fn panic_increases_distress() {
        let mut w = welp();
        w.panic(30.0);
        assert_eq!(w.distress, 30.0);
    }

    #[test]
    fn panic_clamps_at_max() {
        let mut w = welp();
        w.panic(200.0);
        assert_eq!(w.distress, 100.0);
    }

    #[test]
    fn panic_no_op_when_disabled() {
        let mut w = welp();
        w.enabled = false;
        w.panic(50.0);
        assert_eq!(w.distress, 0.0);
    }

    #[test]
    fn panic_sets_just_overwhelmed_at_max() {
        let mut w = welp();
        w.panic(100.0);
        assert!(w.just_overwhelmed);
    }

    #[test]
    fn panic_no_just_overwhelmed_if_already_max() {
        let mut w = welp();
        w.distress = 100.0;
        w.panic(1.0);
        assert!(!w.just_overwhelmed);
    }

    #[test]
    fn calm_decreases_distress() {
        let mut w = welp();
        w.distress = 60.0;
        w.calm(20.0);
        assert_eq!(w.distress, 40.0);
    }

    #[test]
    fn calm_clamps_at_zero() {
        let mut w = welp();
        w.distress = 30.0;
        w.calm(200.0);
        assert_eq!(w.distress, 0.0);
    }

    #[test]
    fn calm_no_op_when_disabled() {
        let mut w = welp();
        w.distress = 50.0;
        w.enabled = false;
        w.calm(10.0);
        assert_eq!(w.distress, 50.0);
    }

    #[test]
    fn calm_no_op_when_already_calm() {
        let mut w = welp();
        w.calm(10.0);
        assert_eq!(w.distress, 0.0);
    }

    #[test]
    fn calm_sets_just_calm_at_zero() {
        let mut w = welp();
        w.distress = 10.0;
        w.calm(10.0);
        assert!(w.just_calm);
    }

    #[test]
    fn calm_no_just_calm_if_already_calm() {
        let mut w = welp();
        w.calm(1.0);
        assert!(!w.just_calm);
    }

    #[test]
    fn tick_increases_distress() {
        let mut w = welp();
        w.tick(1.0);
        assert_eq!(w.distress, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = welp();
        w.tick(2.0);
        assert_eq!(w.distress, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = welp();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.distress, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_overwhelmed() {
        let mut w = welp();
        w.distress = 100.0;
        w.tick(1.0);
        assert_eq!(w.distress, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = welp();
        w.panic_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.distress, 0.0);
    }

    #[test]
    fn is_overwhelmed_true_at_max() {
        let mut w = welp();
        w.distress = 100.0;
        assert!(w.is_overwhelmed());
    }

    #[test]
    fn is_overwhelmed_false_below_max() {
        let mut w = welp();
        w.distress = 50.0;
        assert!(!w.is_overwhelmed());
    }

    #[test]
    fn is_overwhelmed_false_when_disabled() {
        let mut w = welp();
        w.distress = 100.0;
        w.enabled = false;
        assert!(!w.is_overwhelmed());
    }

    #[test]
    fn is_calm_true_at_zero() {
        let w = welp();
        assert!(w.is_calm());
    }

    #[test]
    fn is_calm_false_above_zero() {
        let mut w = welp();
        w.distress = 1.0;
        assert!(!w.is_calm());
    }

    #[test]
    fn distress_fraction_zero_when_calm() {
        let w = welp();
        assert_eq!(w.distress_fraction(), 0.0);
    }

    #[test]
    fn distress_fraction_one_at_max() {
        let mut w = welp();
        w.distress = 100.0;
        assert_eq!(w.distress_fraction(), 1.0);
    }

    #[test]
    fn distress_fraction_half_at_midpoint() {
        let mut w = welp();
        w.distress = 50.0;
        assert_eq!(w.distress_fraction(), 0.5);
    }

    #[test]
    fn distress_fraction_zero_when_max_zero() {
        let mut w = welp();
        w.max_distress = 0.0;
        assert_eq!(w.distress_fraction(), 0.0);
    }

    #[test]
    fn effective_panic_scales() {
        let mut w = welp();
        w.distress = 50.0;
        assert_eq!(w.effective_panic(2.0), 1.0);
    }

    #[test]
    fn effective_panic_zero_when_calm() {
        let w = welp();
        assert_eq!(w.effective_panic(10.0), 0.0);
    }

    #[test]
    fn just_overwhelmed_cleared_on_next_panic() {
        let mut w = welp();
        w.panic(100.0);
        assert!(w.just_overwhelmed);
        w.panic(1.0);
        assert!(!w.just_overwhelmed);
    }

    #[test]
    fn just_calm_cleared_on_next_calm() {
        let mut w = welp();
        w.distress = 10.0;
        w.calm(10.0);
        assert!(w.just_calm);
        w.distress = 10.0;
        w.calm(1.0);
        assert!(!w.just_calm);
    }
}
