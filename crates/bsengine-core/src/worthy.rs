use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Worthy {
    pub merit: f32,
    pub max_merit: f32,
    pub earn_rate: f32,
    pub just_deserving: bool,
    pub just_unworthy: bool,
    pub enabled: bool,
}

impl Default for Worthy {
    fn default() -> Self {
        Self {
            merit: 0.0,
            max_merit: 100.0,
            earn_rate: 1.0,
            just_deserving: false,
            just_unworthy: false,
            enabled: true,
        }
    }
}

impl Worthy {
    pub fn earn(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_deserving = false;
        self.just_unworthy = false;
        let prev = self.merit;
        self.merit = (self.merit + amount).clamp(0.0, self.max_merit);
        if self.merit >= self.max_merit && prev < self.max_merit {
            self.just_deserving = true;
        }
    }

    pub fn demean(&mut self, amount: f32) {
        if !self.enabled || self.merit <= 0.0 {
            return;
        }
        self.just_deserving = false;
        self.just_unworthy = false;
        let prev = self.merit;
        self.merit = (self.merit - amount).max(0.0);
        if self.merit <= 0.0 && prev > 0.0 {
            self.just_unworthy = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.merit >= self.max_merit {
            return;
        }
        self.earn(self.earn_rate * dt);
    }

    pub fn is_deserving(&self) -> bool {
        self.enabled && self.merit >= self.max_merit
    }

    pub fn is_unworthy(&self) -> bool {
        self.merit <= 0.0
    }

    pub fn merit_fraction(&self) -> f32 {
        if self.max_merit <= 0.0 {
            return 0.0;
        }
        self.merit / self.max_merit
    }

    pub fn effective_honor(&self, scale: f32) -> f32 {
        self.merit_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worthy() -> Worthy {
        Worthy {
            merit: 0.0,
            max_merit: 100.0,
            earn_rate: 10.0,
            just_deserving: false,
            just_unworthy: false,
            enabled: true,
        }
    }

    #[test]
    fn default_merit_zero() {
        let w = Worthy::default();
        assert_eq!(w.merit, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Worthy::default().enabled);
    }

    #[test]
    fn earn_increases_merit() {
        let mut w = worthy();
        w.earn(30.0);
        assert_eq!(w.merit, 30.0);
    }

    #[test]
    fn earn_clamps_at_max() {
        let mut w = worthy();
        w.earn(200.0);
        assert_eq!(w.merit, 100.0);
    }

    #[test]
    fn earn_no_op_when_disabled() {
        let mut w = worthy();
        w.enabled = false;
        w.earn(50.0);
        assert_eq!(w.merit, 0.0);
    }

    #[test]
    fn earn_sets_just_deserving_at_max() {
        let mut w = worthy();
        w.earn(100.0);
        assert!(w.just_deserving);
    }

    #[test]
    fn earn_no_just_deserving_if_already_max() {
        let mut w = worthy();
        w.merit = 100.0;
        w.earn(1.0);
        assert!(!w.just_deserving);
    }

    #[test]
    fn demean_decreases_merit() {
        let mut w = worthy();
        w.merit = 60.0;
        w.demean(20.0);
        assert_eq!(w.merit, 40.0);
    }

    #[test]
    fn demean_clamps_at_zero() {
        let mut w = worthy();
        w.merit = 30.0;
        w.demean(200.0);
        assert_eq!(w.merit, 0.0);
    }

    #[test]
    fn demean_no_op_when_disabled() {
        let mut w = worthy();
        w.merit = 50.0;
        w.enabled = false;
        w.demean(10.0);
        assert_eq!(w.merit, 50.0);
    }

    #[test]
    fn demean_no_op_when_already_unworthy() {
        let mut w = worthy();
        w.demean(10.0);
        assert_eq!(w.merit, 0.0);
    }

    #[test]
    fn demean_sets_just_unworthy_at_zero() {
        let mut w = worthy();
        w.merit = 10.0;
        w.demean(10.0);
        assert!(w.just_unworthy);
    }

    #[test]
    fn demean_no_just_unworthy_if_already_zero() {
        let mut w = worthy();
        w.demean(1.0);
        assert!(!w.just_unworthy);
    }

    #[test]
    fn tick_increases_merit() {
        let mut w = worthy();
        w.tick(1.0);
        assert_eq!(w.merit, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = worthy();
        w.tick(2.0);
        assert_eq!(w.merit, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = worthy();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.merit, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_deserving() {
        let mut w = worthy();
        w.merit = 100.0;
        w.tick(1.0);
        assert_eq!(w.merit, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = worthy();
        w.earn_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.merit, 0.0);
    }

    #[test]
    fn is_deserving_true_at_max() {
        let mut w = worthy();
        w.merit = 100.0;
        assert!(w.is_deserving());
    }

    #[test]
    fn is_deserving_false_below_max() {
        let mut w = worthy();
        w.merit = 50.0;
        assert!(!w.is_deserving());
    }

    #[test]
    fn is_deserving_false_when_disabled() {
        let mut w = worthy();
        w.merit = 100.0;
        w.enabled = false;
        assert!(!w.is_deserving());
    }

    #[test]
    fn is_unworthy_true_at_zero() {
        let w = worthy();
        assert!(w.is_unworthy());
    }

    #[test]
    fn is_unworthy_false_above_zero() {
        let mut w = worthy();
        w.merit = 1.0;
        assert!(!w.is_unworthy());
    }

    #[test]
    fn merit_fraction_zero_when_unworthy() {
        let w = worthy();
        assert_eq!(w.merit_fraction(), 0.0);
    }

    #[test]
    fn merit_fraction_one_at_max() {
        let mut w = worthy();
        w.merit = 100.0;
        assert_eq!(w.merit_fraction(), 1.0);
    }

    #[test]
    fn merit_fraction_half_at_midpoint() {
        let mut w = worthy();
        w.merit = 50.0;
        assert_eq!(w.merit_fraction(), 0.5);
    }

    #[test]
    fn merit_fraction_zero_when_max_zero() {
        let mut w = worthy();
        w.max_merit = 0.0;
        assert_eq!(w.merit_fraction(), 0.0);
    }

    #[test]
    fn effective_honor_scales() {
        let mut w = worthy();
        w.merit = 50.0;
        assert_eq!(w.effective_honor(2.0), 1.0);
    }

    #[test]
    fn effective_honor_zero_when_unworthy() {
        let w = worthy();
        assert_eq!(w.effective_honor(10.0), 0.0);
    }

    #[test]
    fn just_deserving_cleared_on_next_earn() {
        let mut w = worthy();
        w.earn(100.0);
        assert!(w.just_deserving);
        w.earn(1.0);
        assert!(!w.just_deserving);
    }

    #[test]
    fn just_unworthy_cleared_on_next_demean() {
        let mut w = worthy();
        w.merit = 10.0;
        w.demean(10.0);
        assert!(w.just_unworthy);
        w.merit = 10.0;
        w.demean(1.0);
        assert!(!w.just_unworthy);
    }
}
