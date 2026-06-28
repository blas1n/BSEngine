use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wash {
    pub grime: f32,
    pub max_grime: f32,
    pub soil_rate: f32,
    pub just_clean: bool,
    pub just_grimy: bool,
    pub enabled: bool,
}

impl Default for Wash {
    fn default() -> Self {
        Self {
            grime: 0.0,
            max_grime: 100.0,
            soil_rate: 1.0,
            just_clean: false,
            just_grimy: false,
            enabled: true,
        }
    }
}

impl Wash {
    pub fn soil(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_clean = false;
        self.just_grimy = false;
        let prev = self.grime;
        self.grime = (self.grime + amount).clamp(0.0, self.max_grime);
        if self.grime >= self.max_grime && prev < self.max_grime {
            self.just_grimy = true;
        }
    }

    pub fn cleanse(&mut self, amount: f32) {
        if !self.enabled || self.grime <= 0.0 {
            return;
        }
        self.just_clean = false;
        self.just_grimy = false;
        let prev = self.grime;
        self.grime = (self.grime - amount).max(0.0);
        if self.grime <= 0.0 && prev > 0.0 {
            self.just_clean = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.grime >= self.max_grime {
            return;
        }
        self.soil(self.soil_rate * dt);
    }

    pub fn is_grimy(&self) -> bool {
        self.enabled && self.grime >= self.max_grime
    }

    pub fn is_clean(&self) -> bool {
        self.grime <= 0.0
    }

    pub fn grime_fraction(&self) -> f32 {
        if self.max_grime <= 0.0 {
            return 0.0;
        }
        self.grime / self.max_grime
    }

    pub fn effective_filth(&self, scale: f32) -> f32 {
        self.grime_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wash() -> Wash {
        Wash {
            grime: 0.0,
            max_grime: 100.0,
            soil_rate: 10.0,
            just_clean: false,
            just_grimy: false,
            enabled: true,
        }
    }

    #[test]
    fn default_grime_zero() {
        let w = Wash::default();
        assert_eq!(w.grime, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wash::default().enabled);
    }

    #[test]
    fn soil_increases_grime() {
        let mut w = wash();
        w.soil(30.0);
        assert_eq!(w.grime, 30.0);
    }

    #[test]
    fn soil_clamps_at_max() {
        let mut w = wash();
        w.soil(200.0);
        assert_eq!(w.grime, 100.0);
    }

    #[test]
    fn soil_no_op_when_disabled() {
        let mut w = wash();
        w.enabled = false;
        w.soil(50.0);
        assert_eq!(w.grime, 0.0);
    }

    #[test]
    fn soil_sets_just_grimy_at_max() {
        let mut w = wash();
        w.soil(100.0);
        assert!(w.just_grimy);
    }

    #[test]
    fn soil_no_just_grimy_if_already_max() {
        let mut w = wash();
        w.grime = 100.0;
        w.soil(1.0);
        assert!(!w.just_grimy);
    }

    #[test]
    fn cleanse_decreases_grime() {
        let mut w = wash();
        w.grime = 60.0;
        w.cleanse(20.0);
        assert_eq!(w.grime, 40.0);
    }

    #[test]
    fn cleanse_clamps_at_zero() {
        let mut w = wash();
        w.grime = 30.0;
        w.cleanse(200.0);
        assert_eq!(w.grime, 0.0);
    }

    #[test]
    fn cleanse_no_op_when_disabled() {
        let mut w = wash();
        w.grime = 50.0;
        w.enabled = false;
        w.cleanse(10.0);
        assert_eq!(w.grime, 50.0);
    }

    #[test]
    fn cleanse_no_op_when_already_clean() {
        let mut w = wash();
        w.cleanse(10.0);
        assert_eq!(w.grime, 0.0);
    }

    #[test]
    fn cleanse_sets_just_clean_at_zero() {
        let mut w = wash();
        w.grime = 10.0;
        w.cleanse(10.0);
        assert!(w.just_clean);
    }

    #[test]
    fn cleanse_no_just_clean_if_already_clean() {
        let mut w = wash();
        w.cleanse(1.0);
        assert!(!w.just_clean);
    }

    #[test]
    fn tick_increases_grime() {
        let mut w = wash();
        w.tick(1.0);
        assert_eq!(w.grime, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wash();
        w.tick(2.0);
        assert_eq!(w.grime, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wash();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.grime, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_grimy() {
        let mut w = wash();
        w.grime = 100.0;
        w.tick(1.0);
        assert_eq!(w.grime, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wash();
        w.soil_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.grime, 0.0);
    }

    #[test]
    fn is_grimy_true_at_max() {
        let mut w = wash();
        w.grime = 100.0;
        assert!(w.is_grimy());
    }

    #[test]
    fn is_grimy_false_below_max() {
        let mut w = wash();
        w.grime = 50.0;
        assert!(!w.is_grimy());
    }

    #[test]
    fn is_grimy_false_when_disabled() {
        let mut w = wash();
        w.grime = 100.0;
        w.enabled = false;
        assert!(!w.is_grimy());
    }

    #[test]
    fn is_clean_true_at_zero() {
        let w = wash();
        assert!(w.is_clean());
    }

    #[test]
    fn is_clean_false_above_zero() {
        let mut w = wash();
        w.grime = 1.0;
        assert!(!w.is_clean());
    }

    #[test]
    fn grime_fraction_zero_when_clean() {
        let w = wash();
        assert_eq!(w.grime_fraction(), 0.0);
    }

    #[test]
    fn grime_fraction_one_at_max() {
        let mut w = wash();
        w.grime = 100.0;
        assert_eq!(w.grime_fraction(), 1.0);
    }

    #[test]
    fn grime_fraction_half_at_midpoint() {
        let mut w = wash();
        w.grime = 50.0;
        assert_eq!(w.grime_fraction(), 0.5);
    }

    #[test]
    fn grime_fraction_zero_when_max_zero() {
        let mut w = wash();
        w.max_grime = 0.0;
        assert_eq!(w.grime_fraction(), 0.0);
    }

    #[test]
    fn effective_filth_scales() {
        let mut w = wash();
        w.grime = 50.0;
        assert_eq!(w.effective_filth(2.0), 1.0);
    }

    #[test]
    fn effective_filth_zero_when_clean() {
        let w = wash();
        assert_eq!(w.effective_filth(10.0), 0.0);
    }

    #[test]
    fn just_grimy_cleared_on_next_soil() {
        let mut w = wash();
        w.soil(100.0);
        assert!(w.just_grimy);
        w.soil(1.0);
        assert!(!w.just_grimy);
    }

    #[test]
    fn just_clean_cleared_on_next_cleanse() {
        let mut w = wash();
        w.grime = 10.0;
        w.cleanse(10.0);
        assert!(w.just_clean);
        w.grime = 10.0;
        w.cleanse(1.0);
        assert!(!w.just_clean);
    }
}
