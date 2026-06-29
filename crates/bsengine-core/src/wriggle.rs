use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wriggle {
    pub squirm: f32,
    pub max_squirm: f32,
    pub writhe_rate: f32,
    pub just_contorted: bool,
    pub just_stilled: bool,
    pub enabled: bool,
}

impl Default for Wriggle {
    fn default() -> Self {
        Self {
            squirm: 0.0,
            max_squirm: 100.0,
            writhe_rate: 1.0,
            just_contorted: false,
            just_stilled: false,
            enabled: true,
        }
    }
}

impl Wriggle {
    pub fn writhe(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_contorted = false;
        self.just_stilled = false;
        let prev = self.squirm;
        self.squirm = (self.squirm + amount).clamp(0.0, self.max_squirm);
        if self.squirm >= self.max_squirm && prev < self.max_squirm {
            self.just_contorted = true;
        }
    }

    pub fn still(&mut self, amount: f32) {
        if !self.enabled || self.squirm <= 0.0 {
            return;
        }
        self.just_contorted = false;
        self.just_stilled = false;
        let prev = self.squirm;
        self.squirm = (self.squirm - amount).max(0.0);
        if self.squirm <= 0.0 && prev > 0.0 {
            self.just_stilled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.squirm >= self.max_squirm {
            return;
        }
        self.writhe(self.writhe_rate * dt);
    }

    pub fn is_contorted(&self) -> bool {
        self.enabled && self.squirm >= self.max_squirm
    }
    pub fn is_stilled(&self) -> bool {
        self.squirm <= 0.0
    }

    pub fn squirm_fraction(&self) -> f32 {
        if self.max_squirm <= 0.0 {
            return 0.0;
        }
        self.squirm / self.max_squirm
    }

    pub fn effective_thrash(&self, scale: f32) -> f32 {
        self.squirm_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wriggle() -> Wriggle {
        Wriggle {
            squirm: 0.0,
            max_squirm: 100.0,
            writhe_rate: 10.0,
            just_contorted: false,
            just_stilled: false,
            enabled: true,
        }
    }

    #[test]
    fn default_squirm_zero() {
        assert_eq!(Wriggle::default().squirm, 0.0);
    }
    #[test]
    fn default_enabled() {
        assert!(Wriggle::default().enabled);
    }
    #[test]
    fn writhe_increases_squirm() {
        let mut w = wriggle();
        w.writhe(30.0);
        assert_eq!(w.squirm, 30.0);
    }
    #[test]
    fn writhe_clamps_at_max() {
        let mut w = wriggle();
        w.writhe(200.0);
        assert_eq!(w.squirm, 100.0);
    }
    #[test]
    fn writhe_no_op_when_disabled() {
        let mut w = wriggle();
        w.enabled = false;
        w.writhe(50.0);
        assert_eq!(w.squirm, 0.0);
    }
    #[test]
    fn writhe_sets_just_contorted_at_max() {
        let mut w = wriggle();
        w.writhe(100.0);
        assert!(w.just_contorted);
    }
    #[test]
    fn writhe_no_just_contorted_if_already_max() {
        let mut w = wriggle();
        w.squirm = 100.0;
        w.writhe(1.0);
        assert!(!w.just_contorted);
    }
    #[test]
    fn still_decreases_squirm() {
        let mut w = wriggle();
        w.squirm = 60.0;
        w.still(20.0);
        assert_eq!(w.squirm, 40.0);
    }
    #[test]
    fn still_clamps_at_zero() {
        let mut w = wriggle();
        w.squirm = 30.0;
        w.still(200.0);
        assert_eq!(w.squirm, 0.0);
    }
    #[test]
    fn still_no_op_when_disabled() {
        let mut w = wriggle();
        w.squirm = 50.0;
        w.enabled = false;
        w.still(10.0);
        assert_eq!(w.squirm, 50.0);
    }
    #[test]
    fn still_no_op_when_already_stilled() {
        let mut w = wriggle();
        w.still(10.0);
        assert_eq!(w.squirm, 0.0);
    }
    #[test]
    fn still_sets_just_stilled_at_zero() {
        let mut w = wriggle();
        w.squirm = 10.0;
        w.still(10.0);
        assert!(w.just_stilled);
    }
    #[test]
    fn still_no_just_stilled_if_already_zero() {
        let mut w = wriggle();
        w.still(1.0);
        assert!(!w.just_stilled);
    }
    #[test]
    fn tick_increases_squirm() {
        let mut w = wriggle();
        w.tick(1.0);
        assert_eq!(w.squirm, 10.0);
    }
    #[test]
    fn tick_scales_with_dt() {
        let mut w = wriggle();
        w.tick(2.0);
        assert_eq!(w.squirm, 20.0);
    }
    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wriggle();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.squirm, 0.0);
    }
    #[test]
    fn tick_no_op_when_already_contorted() {
        let mut w = wriggle();
        w.squirm = 100.0;
        w.tick(1.0);
        assert_eq!(w.squirm, 100.0);
    }
    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wriggle();
        w.writhe_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.squirm, 0.0);
    }
    #[test]
    fn is_contorted_true_at_max() {
        let mut w = wriggle();
        w.squirm = 100.0;
        assert!(w.is_contorted());
    }
    #[test]
    fn is_contorted_false_below_max() {
        let mut w = wriggle();
        w.squirm = 50.0;
        assert!(!w.is_contorted());
    }
    #[test]
    fn is_contorted_false_when_disabled() {
        let mut w = wriggle();
        w.squirm = 100.0;
        w.enabled = false;
        assert!(!w.is_contorted());
    }
    #[test]
    fn is_stilled_true_at_zero() {
        let w = wriggle();
        assert!(w.is_stilled());
    }
    #[test]
    fn is_stilled_false_above_zero() {
        let mut w = wriggle();
        w.squirm = 1.0;
        assert!(!w.is_stilled());
    }
    #[test]
    fn squirm_fraction_zero_when_stilled() {
        let w = wriggle();
        assert_eq!(w.squirm_fraction(), 0.0);
    }
    #[test]
    fn squirm_fraction_one_at_max() {
        let mut w = wriggle();
        w.squirm = 100.0;
        assert_eq!(w.squirm_fraction(), 1.0);
    }
    #[test]
    fn squirm_fraction_half_at_midpoint() {
        let mut w = wriggle();
        w.squirm = 50.0;
        assert_eq!(w.squirm_fraction(), 0.5);
    }
    #[test]
    fn squirm_fraction_zero_when_max_zero() {
        let mut w = wriggle();
        w.max_squirm = 0.0;
        assert_eq!(w.squirm_fraction(), 0.0);
    }
    #[test]
    fn effective_thrash_scales() {
        let mut w = wriggle();
        w.squirm = 50.0;
        assert_eq!(w.effective_thrash(2.0), 1.0);
    }
    #[test]
    fn effective_thrash_zero_when_stilled() {
        let w = wriggle();
        assert_eq!(w.effective_thrash(10.0), 0.0);
    }
    #[test]
    fn just_contorted_cleared_on_next_writhe() {
        let mut w = wriggle();
        w.writhe(100.0);
        assert!(w.just_contorted);
        w.writhe(1.0);
        assert!(!w.just_contorted);
    }
    #[test]
    fn just_stilled_cleared_on_next_still() {
        let mut w = wriggle();
        w.squirm = 10.0;
        w.still(10.0);
        assert!(w.just_stilled);
        w.squirm = 10.0;
        w.still(1.0);
        assert!(!w.just_stilled);
    }
}
