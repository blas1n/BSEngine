use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Windfall {
    pub fortune: f32,
    pub max_fortune: f32,
    pub accrue_rate: f32,
    pub just_fortunate: bool,
    pub just_penniless: bool,
    pub enabled: bool,
}

impl Default for Windfall {
    fn default() -> Self {
        Self {
            fortune: 0.0,
            max_fortune: 100.0,
            accrue_rate: 1.0,
            just_fortunate: false,
            just_penniless: false,
            enabled: true,
        }
    }
}

impl Windfall {
    pub fn accrue(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_fortunate = false;
        self.just_penniless = false;
        let prev = self.fortune;
        self.fortune = (self.fortune + amount).clamp(0.0, self.max_fortune);
        if self.fortune >= self.max_fortune && prev < self.max_fortune {
            self.just_fortunate = true;
        }
    }

    pub fn spend(&mut self, amount: f32) {
        if !self.enabled || self.fortune <= 0.0 {
            return;
        }
        self.just_fortunate = false;
        self.just_penniless = false;
        let prev = self.fortune;
        self.fortune = (self.fortune - amount).max(0.0);
        if self.fortune <= 0.0 && prev > 0.0 {
            self.just_penniless = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.fortune >= self.max_fortune {
            return;
        }
        self.accrue(self.accrue_rate * dt);
    }

    pub fn is_fortunate(&self) -> bool {
        self.enabled && self.fortune >= self.max_fortune
    }

    pub fn is_penniless(&self) -> bool {
        self.fortune <= 0.0
    }

    pub fn fortune_fraction(&self) -> f32 {
        if self.max_fortune <= 0.0 {
            return 0.0;
        }
        self.fortune / self.max_fortune
    }

    pub fn effective_bounty(&self, scale: f32) -> f32 {
        self.fortune_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn windfall() -> Windfall {
        Windfall {
            fortune: 0.0,
            max_fortune: 100.0,
            accrue_rate: 10.0,
            just_fortunate: false,
            just_penniless: false,
            enabled: true,
        }
    }

    #[test]
    fn default_fortune_zero() {
        let w = Windfall::default();
        assert_eq!(w.fortune, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Windfall::default().enabled);
    }

    #[test]
    fn accrue_increases_fortune() {
        let mut w = windfall();
        w.accrue(30.0);
        assert_eq!(w.fortune, 30.0);
    }

    #[test]
    fn accrue_clamps_at_max() {
        let mut w = windfall();
        w.accrue(200.0);
        assert_eq!(w.fortune, 100.0);
    }

    #[test]
    fn accrue_no_op_when_disabled() {
        let mut w = windfall();
        w.enabled = false;
        w.accrue(50.0);
        assert_eq!(w.fortune, 0.0);
    }

    #[test]
    fn accrue_sets_just_fortunate_at_max() {
        let mut w = windfall();
        w.accrue(100.0);
        assert!(w.just_fortunate);
    }

    #[test]
    fn accrue_no_just_fortunate_if_already_max() {
        let mut w = windfall();
        w.fortune = 100.0;
        w.accrue(1.0);
        assert!(!w.just_fortunate);
    }

    #[test]
    fn spend_decreases_fortune() {
        let mut w = windfall();
        w.fortune = 60.0;
        w.spend(20.0);
        assert_eq!(w.fortune, 40.0);
    }

    #[test]
    fn spend_clamps_at_zero() {
        let mut w = windfall();
        w.fortune = 30.0;
        w.spend(200.0);
        assert_eq!(w.fortune, 0.0);
    }

    #[test]
    fn spend_no_op_when_disabled() {
        let mut w = windfall();
        w.fortune = 50.0;
        w.enabled = false;
        w.spend(10.0);
        assert_eq!(w.fortune, 50.0);
    }

    #[test]
    fn spend_no_op_when_already_penniless() {
        let mut w = windfall();
        w.spend(10.0);
        assert_eq!(w.fortune, 0.0);
    }

    #[test]
    fn spend_sets_just_penniless_at_zero() {
        let mut w = windfall();
        w.fortune = 10.0;
        w.spend(10.0);
        assert!(w.just_penniless);
    }

    #[test]
    fn spend_no_just_penniless_if_already_zero() {
        let mut w = windfall();
        w.spend(1.0);
        assert!(!w.just_penniless);
    }

    #[test]
    fn tick_increases_fortune() {
        let mut w = windfall();
        w.tick(1.0);
        assert_eq!(w.fortune, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = windfall();
        w.tick(2.0);
        assert_eq!(w.fortune, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = windfall();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.fortune, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_fortunate() {
        let mut w = windfall();
        w.fortune = 100.0;
        w.tick(1.0);
        assert_eq!(w.fortune, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = windfall();
        w.accrue_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.fortune, 0.0);
    }

    #[test]
    fn is_fortunate_true_at_max() {
        let mut w = windfall();
        w.fortune = 100.0;
        assert!(w.is_fortunate());
    }

    #[test]
    fn is_fortunate_false_below_max() {
        let mut w = windfall();
        w.fortune = 50.0;
        assert!(!w.is_fortunate());
    }

    #[test]
    fn is_fortunate_false_when_disabled() {
        let mut w = windfall();
        w.fortune = 100.0;
        w.enabled = false;
        assert!(!w.is_fortunate());
    }

    #[test]
    fn is_penniless_true_at_zero() {
        let w = windfall();
        assert!(w.is_penniless());
    }

    #[test]
    fn is_penniless_false_above_zero() {
        let mut w = windfall();
        w.fortune = 1.0;
        assert!(!w.is_penniless());
    }

    #[test]
    fn fortune_fraction_zero_when_penniless() {
        let w = windfall();
        assert_eq!(w.fortune_fraction(), 0.0);
    }

    #[test]
    fn fortune_fraction_one_at_max() {
        let mut w = windfall();
        w.fortune = 100.0;
        assert_eq!(w.fortune_fraction(), 1.0);
    }

    #[test]
    fn fortune_fraction_half_at_midpoint() {
        let mut w = windfall();
        w.fortune = 50.0;
        assert_eq!(w.fortune_fraction(), 0.5);
    }

    #[test]
    fn fortune_fraction_zero_when_max_zero() {
        let mut w = windfall();
        w.max_fortune = 0.0;
        assert_eq!(w.fortune_fraction(), 0.0);
    }

    #[test]
    fn effective_bounty_scales() {
        let mut w = windfall();
        w.fortune = 50.0;
        assert_eq!(w.effective_bounty(2.0), 1.0);
    }

    #[test]
    fn effective_bounty_zero_when_penniless() {
        let w = windfall();
        assert_eq!(w.effective_bounty(10.0), 0.0);
    }

    #[test]
    fn just_fortunate_cleared_on_next_accrue() {
        let mut w = windfall();
        w.accrue(100.0);
        assert!(w.just_fortunate);
        w.accrue(1.0);
        assert!(!w.just_fortunate);
    }

    #[test]
    fn just_penniless_cleared_on_next_spend() {
        let mut w = windfall();
        w.fortune = 10.0;
        w.spend(10.0);
        assert!(w.just_penniless);
        w.fortune = 10.0;
        w.spend(1.0);
        assert!(!w.just_penniless);
    }
}
