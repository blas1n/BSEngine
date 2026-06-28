use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wealth {
    pub riches: f32,
    pub max_riches: f32,
    pub accrue_rate: f32,
    pub just_affluent: bool,
    pub just_destitute: bool,
    pub enabled: bool,
}

impl Default for Wealth {
    fn default() -> Self {
        Self {
            riches: 0.0,
            max_riches: 100.0,
            accrue_rate: 1.0,
            just_affluent: false,
            just_destitute: false,
            enabled: true,
        }
    }
}

impl Wealth {
    pub fn accrue(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_affluent = false;
        self.just_destitute = false;
        let prev = self.riches;
        self.riches = (self.riches + amount).clamp(0.0, self.max_riches);
        if self.riches >= self.max_riches && prev < self.max_riches {
            self.just_affluent = true;
        }
    }

    pub fn spend(&mut self, amount: f32) {
        if !self.enabled || self.riches <= 0.0 {
            return;
        }
        self.just_affluent = false;
        self.just_destitute = false;
        let prev = self.riches;
        self.riches = (self.riches - amount).max(0.0);
        if self.riches <= 0.0 && prev > 0.0 {
            self.just_destitute = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.riches >= self.max_riches {
            return;
        }
        self.accrue(self.accrue_rate * dt);
    }

    pub fn is_affluent(&self) -> bool {
        self.enabled && self.riches >= self.max_riches
    }

    pub fn is_destitute(&self) -> bool {
        self.riches <= 0.0
    }

    pub fn riches_fraction(&self) -> f32 {
        if self.max_riches <= 0.0 {
            return 0.0;
        }
        self.riches / self.max_riches
    }

    pub fn effective_prosperity(&self, scale: f32) -> f32 {
        self.riches_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wealth() -> Wealth {
        Wealth {
            riches: 0.0,
            max_riches: 100.0,
            accrue_rate: 10.0,
            just_affluent: false,
            just_destitute: false,
            enabled: true,
        }
    }

    #[test]
    fn default_riches_zero() {
        let w = Wealth::default();
        assert_eq!(w.riches, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wealth::default().enabled);
    }

    #[test]
    fn accrue_increases_riches() {
        let mut w = wealth();
        w.accrue(30.0);
        assert_eq!(w.riches, 30.0);
    }

    #[test]
    fn accrue_clamps_at_max() {
        let mut w = wealth();
        w.accrue(200.0);
        assert_eq!(w.riches, 100.0);
    }

    #[test]
    fn accrue_no_op_when_disabled() {
        let mut w = wealth();
        w.enabled = false;
        w.accrue(50.0);
        assert_eq!(w.riches, 0.0);
    }

    #[test]
    fn accrue_sets_just_affluent_at_max() {
        let mut w = wealth();
        w.accrue(100.0);
        assert!(w.just_affluent);
    }

    #[test]
    fn accrue_no_just_affluent_if_already_max() {
        let mut w = wealth();
        w.riches = 100.0;
        w.accrue(1.0);
        assert!(!w.just_affluent);
    }

    #[test]
    fn spend_decreases_riches() {
        let mut w = wealth();
        w.riches = 60.0;
        w.spend(20.0);
        assert_eq!(w.riches, 40.0);
    }

    #[test]
    fn spend_clamps_at_zero() {
        let mut w = wealth();
        w.riches = 30.0;
        w.spend(200.0);
        assert_eq!(w.riches, 0.0);
    }

    #[test]
    fn spend_no_op_when_disabled() {
        let mut w = wealth();
        w.riches = 50.0;
        w.enabled = false;
        w.spend(10.0);
        assert_eq!(w.riches, 50.0);
    }

    #[test]
    fn spend_no_op_when_already_destitute() {
        let mut w = wealth();
        w.spend(10.0);
        assert_eq!(w.riches, 0.0);
    }

    #[test]
    fn spend_sets_just_destitute_at_zero() {
        let mut w = wealth();
        w.riches = 10.0;
        w.spend(10.0);
        assert!(w.just_destitute);
    }

    #[test]
    fn spend_no_just_destitute_if_already_zero() {
        let mut w = wealth();
        w.spend(1.0);
        assert!(!w.just_destitute);
    }

    #[test]
    fn tick_increases_riches() {
        let mut w = wealth();
        w.tick(1.0);
        assert_eq!(w.riches, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wealth();
        w.tick(2.0);
        assert_eq!(w.riches, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wealth();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.riches, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_affluent() {
        let mut w = wealth();
        w.riches = 100.0;
        w.tick(1.0);
        assert_eq!(w.riches, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wealth();
        w.accrue_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.riches, 0.0);
    }

    #[test]
    fn is_affluent_true_at_max() {
        let mut w = wealth();
        w.riches = 100.0;
        assert!(w.is_affluent());
    }

    #[test]
    fn is_affluent_false_below_max() {
        let mut w = wealth();
        w.riches = 50.0;
        assert!(!w.is_affluent());
    }

    #[test]
    fn is_affluent_false_when_disabled() {
        let mut w = wealth();
        w.riches = 100.0;
        w.enabled = false;
        assert!(!w.is_affluent());
    }

    #[test]
    fn is_destitute_true_at_zero() {
        let w = wealth();
        assert!(w.is_destitute());
    }

    #[test]
    fn is_destitute_false_above_zero() {
        let mut w = wealth();
        w.riches = 1.0;
        assert!(!w.is_destitute());
    }

    #[test]
    fn riches_fraction_zero_when_destitute() {
        let w = wealth();
        assert_eq!(w.riches_fraction(), 0.0);
    }

    #[test]
    fn riches_fraction_one_at_max() {
        let mut w = wealth();
        w.riches = 100.0;
        assert_eq!(w.riches_fraction(), 1.0);
    }

    #[test]
    fn riches_fraction_half_at_midpoint() {
        let mut w = wealth();
        w.riches = 50.0;
        assert_eq!(w.riches_fraction(), 0.5);
    }

    #[test]
    fn riches_fraction_zero_when_max_zero() {
        let mut w = wealth();
        w.max_riches = 0.0;
        assert_eq!(w.riches_fraction(), 0.0);
    }

    #[test]
    fn effective_prosperity_scales() {
        let mut w = wealth();
        w.riches = 50.0;
        assert_eq!(w.effective_prosperity(2.0), 1.0);
    }

    #[test]
    fn effective_prosperity_zero_when_destitute() {
        let w = wealth();
        assert_eq!(w.effective_prosperity(10.0), 0.0);
    }

    #[test]
    fn just_affluent_cleared_on_next_accrue() {
        let mut w = wealth();
        w.accrue(100.0);
        assert!(w.just_affluent);
        w.accrue(1.0);
        assert!(!w.just_affluent);
    }

    #[test]
    fn just_destitute_cleared_on_next_spend() {
        let mut w = wealth();
        w.riches = 10.0;
        w.spend(10.0);
        assert!(w.just_destitute);
        w.riches = 10.0;
        w.spend(1.0);
        assert!(!w.just_destitute);
    }
}
