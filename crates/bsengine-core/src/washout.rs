use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Washout {
    pub cleanse: f32,
    pub max_cleanse: f32,
    pub rinse_rate: f32,
    pub just_purged: bool,
    pub just_soiled: bool,
    pub enabled: bool,
}

impl Default for Washout {
    fn default() -> Self {
        Self {
            cleanse: 0.0,
            max_cleanse: 100.0,
            rinse_rate: 1.0,
            just_purged: false,
            just_soiled: false,
            enabled: true,
        }
    }
}

impl Washout {
    pub fn rinse(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_purged = false;
        self.just_soiled = false;
        let prev = self.cleanse;
        self.cleanse = (self.cleanse + amount).clamp(0.0, self.max_cleanse);
        if self.cleanse >= self.max_cleanse && prev < self.max_cleanse {
            self.just_purged = true;
        }
    }

    pub fn soil(&mut self, amount: f32) {
        if !self.enabled || self.cleanse <= 0.0 {
            return;
        }
        self.just_purged = false;
        self.just_soiled = false;
        let prev = self.cleanse;
        self.cleanse = (self.cleanse - amount).max(0.0);
        if self.cleanse <= 0.0 && prev > 0.0 {
            self.just_soiled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.cleanse >= self.max_cleanse {
            return;
        }
        self.rinse(self.rinse_rate * dt);
    }

    pub fn is_purged(&self) -> bool {
        self.enabled && self.cleanse >= self.max_cleanse
    }

    pub fn is_soiled(&self) -> bool {
        self.cleanse <= 0.0
    }

    pub fn cleanse_fraction(&self) -> f32 {
        if self.max_cleanse <= 0.0 {
            return 0.0;
        }
        self.cleanse / self.max_cleanse
    }

    pub fn effective_purity(&self, scale: f32) -> f32 {
        self.cleanse_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn washout() -> Washout {
        Washout {
            cleanse: 0.0,
            max_cleanse: 100.0,
            rinse_rate: 10.0,
            just_purged: false,
            just_soiled: false,
            enabled: true,
        }
    }

    #[test]
    fn default_cleanse_zero() {
        let w = Washout::default();
        assert_eq!(w.cleanse, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Washout::default().enabled);
    }

    #[test]
    fn rinse_increases_cleanse() {
        let mut w = washout();
        w.rinse(30.0);
        assert_eq!(w.cleanse, 30.0);
    }

    #[test]
    fn rinse_clamps_at_max() {
        let mut w = washout();
        w.rinse(200.0);
        assert_eq!(w.cleanse, 100.0);
    }

    #[test]
    fn rinse_no_op_when_disabled() {
        let mut w = washout();
        w.enabled = false;
        w.rinse(50.0);
        assert_eq!(w.cleanse, 0.0);
    }

    #[test]
    fn rinse_sets_just_purged_at_max() {
        let mut w = washout();
        w.rinse(100.0);
        assert!(w.just_purged);
    }

    #[test]
    fn rinse_no_just_purged_if_already_max() {
        let mut w = washout();
        w.cleanse = 100.0;
        w.rinse(1.0);
        assert!(!w.just_purged);
    }

    #[test]
    fn soil_decreases_cleanse() {
        let mut w = washout();
        w.cleanse = 60.0;
        w.soil(20.0);
        assert_eq!(w.cleanse, 40.0);
    }

    #[test]
    fn soil_clamps_at_zero() {
        let mut w = washout();
        w.cleanse = 30.0;
        w.soil(200.0);
        assert_eq!(w.cleanse, 0.0);
    }

    #[test]
    fn soil_no_op_when_disabled() {
        let mut w = washout();
        w.cleanse = 50.0;
        w.enabled = false;
        w.soil(10.0);
        assert_eq!(w.cleanse, 50.0);
    }

    #[test]
    fn soil_no_op_when_already_soiled() {
        let mut w = washout();
        w.soil(10.0);
        assert_eq!(w.cleanse, 0.0);
    }

    #[test]
    fn soil_sets_just_soiled_at_zero() {
        let mut w = washout();
        w.cleanse = 10.0;
        w.soil(10.0);
        assert!(w.just_soiled);
    }

    #[test]
    fn soil_no_just_soiled_if_already_zero() {
        let mut w = washout();
        w.soil(1.0);
        assert!(!w.just_soiled);
    }

    #[test]
    fn tick_increases_cleanse() {
        let mut w = washout();
        w.tick(1.0);
        assert_eq!(w.cleanse, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = washout();
        w.tick(2.0);
        assert_eq!(w.cleanse, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = washout();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.cleanse, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_purged() {
        let mut w = washout();
        w.cleanse = 100.0;
        w.tick(1.0);
        assert_eq!(w.cleanse, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = washout();
        w.rinse_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.cleanse, 0.0);
    }

    #[test]
    fn is_purged_true_at_max() {
        let mut w = washout();
        w.cleanse = 100.0;
        assert!(w.is_purged());
    }

    #[test]
    fn is_purged_false_below_max() {
        let mut w = washout();
        w.cleanse = 50.0;
        assert!(!w.is_purged());
    }

    #[test]
    fn is_purged_false_when_disabled() {
        let mut w = washout();
        w.cleanse = 100.0;
        w.enabled = false;
        assert!(!w.is_purged());
    }

    #[test]
    fn is_soiled_true_at_zero() {
        let w = washout();
        assert!(w.is_soiled());
    }

    #[test]
    fn is_soiled_false_above_zero() {
        let mut w = washout();
        w.cleanse = 1.0;
        assert!(!w.is_soiled());
    }

    #[test]
    fn cleanse_fraction_zero_when_soiled() {
        let w = washout();
        assert_eq!(w.cleanse_fraction(), 0.0);
    }

    #[test]
    fn cleanse_fraction_one_at_max() {
        let mut w = washout();
        w.cleanse = 100.0;
        assert_eq!(w.cleanse_fraction(), 1.0);
    }

    #[test]
    fn cleanse_fraction_half_at_midpoint() {
        let mut w = washout();
        w.cleanse = 50.0;
        assert_eq!(w.cleanse_fraction(), 0.5);
    }

    #[test]
    fn cleanse_fraction_zero_when_max_zero() {
        let mut w = washout();
        w.max_cleanse = 0.0;
        assert_eq!(w.cleanse_fraction(), 0.0);
    }

    #[test]
    fn effective_purity_scales() {
        let mut w = washout();
        w.cleanse = 50.0;
        assert_eq!(w.effective_purity(2.0), 1.0);
    }

    #[test]
    fn effective_purity_zero_when_soiled() {
        let w = washout();
        assert_eq!(w.effective_purity(10.0), 0.0);
    }

    #[test]
    fn just_purged_cleared_on_next_rinse() {
        let mut w = washout();
        w.rinse(100.0);
        assert!(w.just_purged);
        w.rinse(1.0);
        assert!(!w.just_purged);
    }

    #[test]
    fn just_soiled_cleared_on_next_soil() {
        let mut w = washout();
        w.cleanse = 10.0;
        w.soil(10.0);
        assert!(w.just_soiled);
        w.cleanse = 10.0;
        w.soil(1.0);
        assert!(!w.just_soiled);
    }
}
