use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Want {
    pub desire: f32,
    pub max_desire: f32,
    pub craving_rate: f32,
    pub just_wanted: bool,
    pub just_sated: bool,
    pub enabled: bool,
}

impl Default for Want {
    fn default() -> Self {
        Self {
            desire: 0.0,
            max_desire: 100.0,
            craving_rate: 1.0,
            just_wanted: false,
            just_sated: false,
            enabled: true,
        }
    }
}

impl Want {
    pub fn crave(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_wanted = false;
        self.just_sated = false;
        let prev = self.desire;
        self.desire = (self.desire + amount).clamp(0.0, self.max_desire);
        if self.desire >= self.max_desire && prev < self.max_desire {
            self.just_wanted = true;
        }
    }

    pub fn sate(&mut self, amount: f32) {
        if !self.enabled || self.desire <= 0.0 {
            return;
        }
        self.just_wanted = false;
        self.just_sated = false;
        let prev = self.desire;
        self.desire = (self.desire - amount).max(0.0);
        if self.desire <= 0.0 && prev > 0.0 {
            self.just_sated = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.desire >= self.max_desire {
            return;
        }
        self.crave(self.craving_rate * dt);
    }

    pub fn is_wanted(&self) -> bool {
        self.enabled && self.desire >= self.max_desire
    }

    pub fn is_sated(&self) -> bool {
        self.desire <= 0.0
    }

    pub fn desire_fraction(&self) -> f32 {
        if self.max_desire <= 0.0 {
            return 0.0;
        }
        self.desire / self.max_desire
    }

    pub fn effective_longing(&self, scale: f32) -> f32 {
        self.desire_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn want() -> Want {
        Want {
            desire: 0.0,
            max_desire: 100.0,
            craving_rate: 10.0,
            just_wanted: false,
            just_sated: false,
            enabled: true,
        }
    }

    #[test]
    fn default_desire_zero() {
        let w = Want::default();
        assert_eq!(w.desire, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Want::default().enabled);
    }

    #[test]
    fn crave_increases_desire() {
        let mut w = want();
        w.crave(30.0);
        assert_eq!(w.desire, 30.0);
    }

    #[test]
    fn crave_clamps_at_max() {
        let mut w = want();
        w.crave(200.0);
        assert_eq!(w.desire, 100.0);
    }

    #[test]
    fn crave_no_op_when_disabled() {
        let mut w = want();
        w.enabled = false;
        w.crave(50.0);
        assert_eq!(w.desire, 0.0);
    }

    #[test]
    fn crave_sets_just_wanted_at_max() {
        let mut w = want();
        w.crave(100.0);
        assert!(w.just_wanted);
    }

    #[test]
    fn crave_no_just_wanted_if_already_max() {
        let mut w = want();
        w.desire = 100.0;
        w.crave(1.0);
        assert!(!w.just_wanted);
    }

    #[test]
    fn sate_decreases_desire() {
        let mut w = want();
        w.desire = 60.0;
        w.sate(20.0);
        assert_eq!(w.desire, 40.0);
    }

    #[test]
    fn sate_clamps_at_zero() {
        let mut w = want();
        w.desire = 30.0;
        w.sate(200.0);
        assert_eq!(w.desire, 0.0);
    }

    #[test]
    fn sate_no_op_when_disabled() {
        let mut w = want();
        w.desire = 50.0;
        w.enabled = false;
        w.sate(10.0);
        assert_eq!(w.desire, 50.0);
    }

    #[test]
    fn sate_no_op_when_already_zero() {
        let mut w = want();
        w.sate(10.0);
        assert_eq!(w.desire, 0.0);
    }

    #[test]
    fn sate_sets_just_sated_at_zero() {
        let mut w = want();
        w.desire = 10.0;
        w.sate(10.0);
        assert!(w.just_sated);
    }

    #[test]
    fn sate_no_just_sated_if_already_zero() {
        let mut w = want();
        w.sate(1.0);
        assert!(!w.just_sated);
    }

    #[test]
    fn tick_increases_desire() {
        let mut w = want();
        w.tick(1.0);
        assert_eq!(w.desire, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = want();
        w.tick(2.0);
        assert_eq!(w.desire, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = want();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.desire, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_wanted() {
        let mut w = want();
        w.desire = 100.0;
        w.tick(1.0);
        assert_eq!(w.desire, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = want();
        w.craving_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.desire, 0.0);
    }

    #[test]
    fn is_wanted_true_at_max() {
        let mut w = want();
        w.desire = 100.0;
        assert!(w.is_wanted());
    }

    #[test]
    fn is_wanted_false_below_max() {
        let mut w = want();
        w.desire = 50.0;
        assert!(!w.is_wanted());
    }

    #[test]
    fn is_wanted_false_when_disabled() {
        let mut w = want();
        w.desire = 100.0;
        w.enabled = false;
        assert!(!w.is_wanted());
    }

    #[test]
    fn is_sated_true_at_zero() {
        let w = want();
        assert!(w.is_sated());
    }

    #[test]
    fn is_sated_false_above_zero() {
        let mut w = want();
        w.desire = 1.0;
        assert!(!w.is_sated());
    }

    #[test]
    fn desire_fraction_zero_when_sated() {
        let w = want();
        assert_eq!(w.desire_fraction(), 0.0);
    }

    #[test]
    fn desire_fraction_one_at_max() {
        let mut w = want();
        w.desire = 100.0;
        assert_eq!(w.desire_fraction(), 1.0);
    }

    #[test]
    fn desire_fraction_half_at_midpoint() {
        let mut w = want();
        w.desire = 50.0;
        assert_eq!(w.desire_fraction(), 0.5);
    }

    #[test]
    fn desire_fraction_zero_when_max_zero() {
        let mut w = want();
        w.max_desire = 0.0;
        assert_eq!(w.desire_fraction(), 0.0);
    }

    #[test]
    fn effective_longing_scales() {
        let mut w = want();
        w.desire = 50.0;
        assert_eq!(w.effective_longing(2.0), 1.0);
    }

    #[test]
    fn effective_longing_zero_when_sated() {
        let w = want();
        assert_eq!(w.effective_longing(10.0), 0.0);
    }

    #[test]
    fn just_wanted_cleared_on_next_crave() {
        let mut w = want();
        w.crave(100.0);
        assert!(w.just_wanted);
        w.crave(1.0);
        assert!(!w.just_wanted);
    }

    #[test]
    fn just_sated_cleared_on_next_sate() {
        let mut w = want();
        w.desire = 10.0;
        w.sate(10.0);
        assert!(w.just_sated);
        w.desire = 10.0;
        w.sate(1.0);
        assert!(!w.just_sated);
    }
}
