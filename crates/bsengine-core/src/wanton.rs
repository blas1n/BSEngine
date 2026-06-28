use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wanton {
    pub excess: f32,
    pub max_excess: f32,
    pub indulge_rate: f32,
    pub just_reckless: bool,
    pub just_restrained: bool,
    pub enabled: bool,
}

impl Default for Wanton {
    fn default() -> Self {
        Self {
            excess: 0.0,
            max_excess: 100.0,
            indulge_rate: 1.0,
            just_reckless: false,
            just_restrained: false,
            enabled: true,
        }
    }
}

impl Wanton {
    pub fn indulge(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_reckless = false;
        self.just_restrained = false;
        let prev = self.excess;
        self.excess = (self.excess + amount).clamp(0.0, self.max_excess);
        if self.excess >= self.max_excess && prev < self.max_excess {
            self.just_reckless = true;
        }
    }

    pub fn restrain(&mut self, amount: f32) {
        if !self.enabled || self.excess <= 0.0 {
            return;
        }
        self.just_reckless = false;
        self.just_restrained = false;
        let prev = self.excess;
        self.excess = (self.excess - amount).max(0.0);
        if self.excess <= 0.0 && prev > 0.0 {
            self.just_restrained = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.excess >= self.max_excess {
            return;
        }
        self.indulge(self.indulge_rate * dt);
    }

    pub fn is_reckless(&self) -> bool {
        self.enabled && self.excess >= self.max_excess
    }

    pub fn is_restrained(&self) -> bool {
        self.excess <= 0.0
    }

    pub fn excess_fraction(&self) -> f32 {
        if self.max_excess <= 0.0 {
            return 0.0;
        }
        self.excess / self.max_excess
    }

    pub fn effective_abandon(&self, scale: f32) -> f32 {
        self.excess_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wanton() -> Wanton {
        Wanton {
            excess: 0.0,
            max_excess: 100.0,
            indulge_rate: 10.0,
            just_reckless: false,
            just_restrained: false,
            enabled: true,
        }
    }

    #[test]
    fn default_excess_zero() {
        let w = Wanton::default();
        assert_eq!(w.excess, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wanton::default().enabled);
    }

    #[test]
    fn indulge_increases_excess() {
        let mut w = wanton();
        w.indulge(30.0);
        assert_eq!(w.excess, 30.0);
    }

    #[test]
    fn indulge_clamps_at_max() {
        let mut w = wanton();
        w.indulge(200.0);
        assert_eq!(w.excess, 100.0);
    }

    #[test]
    fn indulge_no_op_when_disabled() {
        let mut w = wanton();
        w.enabled = false;
        w.indulge(50.0);
        assert_eq!(w.excess, 0.0);
    }

    #[test]
    fn indulge_sets_just_reckless_at_max() {
        let mut w = wanton();
        w.indulge(100.0);
        assert!(w.just_reckless);
    }

    #[test]
    fn indulge_no_just_reckless_if_already_max() {
        let mut w = wanton();
        w.excess = 100.0;
        w.indulge(1.0);
        assert!(!w.just_reckless);
    }

    #[test]
    fn restrain_decreases_excess() {
        let mut w = wanton();
        w.excess = 60.0;
        w.restrain(20.0);
        assert_eq!(w.excess, 40.0);
    }

    #[test]
    fn restrain_clamps_at_zero() {
        let mut w = wanton();
        w.excess = 30.0;
        w.restrain(200.0);
        assert_eq!(w.excess, 0.0);
    }

    #[test]
    fn restrain_no_op_when_disabled() {
        let mut w = wanton();
        w.excess = 50.0;
        w.enabled = false;
        w.restrain(10.0);
        assert_eq!(w.excess, 50.0);
    }

    #[test]
    fn restrain_no_op_when_already_restrained() {
        let mut w = wanton();
        w.restrain(10.0);
        assert_eq!(w.excess, 0.0);
    }

    #[test]
    fn restrain_sets_just_restrained_at_zero() {
        let mut w = wanton();
        w.excess = 10.0;
        w.restrain(10.0);
        assert!(w.just_restrained);
    }

    #[test]
    fn restrain_no_just_restrained_if_already_zero() {
        let mut w = wanton();
        w.restrain(1.0);
        assert!(!w.just_restrained);
    }

    #[test]
    fn tick_increases_excess() {
        let mut w = wanton();
        w.tick(1.0);
        assert_eq!(w.excess, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wanton();
        w.tick(2.0);
        assert_eq!(w.excess, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wanton();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.excess, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_reckless() {
        let mut w = wanton();
        w.excess = 100.0;
        w.tick(1.0);
        assert_eq!(w.excess, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wanton();
        w.indulge_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.excess, 0.0);
    }

    #[test]
    fn is_reckless_true_at_max() {
        let mut w = wanton();
        w.excess = 100.0;
        assert!(w.is_reckless());
    }

    #[test]
    fn is_reckless_false_below_max() {
        let mut w = wanton();
        w.excess = 50.0;
        assert!(!w.is_reckless());
    }

    #[test]
    fn is_reckless_false_when_disabled() {
        let mut w = wanton();
        w.excess = 100.0;
        w.enabled = false;
        assert!(!w.is_reckless());
    }

    #[test]
    fn is_restrained_true_at_zero() {
        let w = wanton();
        assert!(w.is_restrained());
    }

    #[test]
    fn is_restrained_false_above_zero() {
        let mut w = wanton();
        w.excess = 1.0;
        assert!(!w.is_restrained());
    }

    #[test]
    fn excess_fraction_zero_when_restrained() {
        let w = wanton();
        assert_eq!(w.excess_fraction(), 0.0);
    }

    #[test]
    fn excess_fraction_one_at_max() {
        let mut w = wanton();
        w.excess = 100.0;
        assert_eq!(w.excess_fraction(), 1.0);
    }

    #[test]
    fn excess_fraction_half_at_midpoint() {
        let mut w = wanton();
        w.excess = 50.0;
        assert_eq!(w.excess_fraction(), 0.5);
    }

    #[test]
    fn excess_fraction_zero_when_max_zero() {
        let mut w = wanton();
        w.max_excess = 0.0;
        assert_eq!(w.excess_fraction(), 0.0);
    }

    #[test]
    fn effective_abandon_scales() {
        let mut w = wanton();
        w.excess = 50.0;
        assert_eq!(w.effective_abandon(2.0), 1.0);
    }

    #[test]
    fn effective_abandon_zero_when_restrained() {
        let w = wanton();
        assert_eq!(w.effective_abandon(10.0), 0.0);
    }

    #[test]
    fn just_reckless_cleared_on_next_indulge() {
        let mut w = wanton();
        w.indulge(100.0);
        assert!(w.just_reckless);
        w.indulge(1.0);
        assert!(!w.just_reckless);
    }

    #[test]
    fn just_restrained_cleared_on_next_restrain() {
        let mut w = wanton();
        w.excess = 10.0;
        w.restrain(10.0);
        assert!(w.just_restrained);
        w.excess = 10.0;
        w.restrain(1.0);
        assert!(!w.just_restrained);
    }
}
