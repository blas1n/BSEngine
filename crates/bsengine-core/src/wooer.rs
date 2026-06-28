use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wooer {
    pub appeal: f32,
    pub max_appeal: f32,
    pub charm_rate: f32,
    pub just_smitten: bool,
    pub just_spurned: bool,
    pub enabled: bool,
}

impl Default for Wooer {
    fn default() -> Self {
        Self {
            appeal: 0.0,
            max_appeal: 100.0,
            charm_rate: 1.0,
            just_smitten: false,
            just_spurned: false,
            enabled: true,
        }
    }
}

impl Wooer {
    pub fn woo(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_smitten = false;
        self.just_spurned = false;
        let prev = self.appeal;
        self.appeal = (self.appeal + amount).clamp(0.0, self.max_appeal);
        if self.appeal >= self.max_appeal && prev < self.max_appeal {
            self.just_smitten = true;
        }
    }

    pub fn spurn(&mut self, amount: f32) {
        if !self.enabled || self.appeal <= 0.0 {
            return;
        }
        self.just_smitten = false;
        self.just_spurned = false;
        let prev = self.appeal;
        self.appeal = (self.appeal - amount).max(0.0);
        if self.appeal <= 0.0 && prev > 0.0 {
            self.just_spurned = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.appeal >= self.max_appeal {
            return;
        }
        self.woo(self.charm_rate * dt);
    }

    pub fn is_smitten(&self) -> bool {
        self.enabled && self.appeal >= self.max_appeal
    }

    pub fn is_spurned(&self) -> bool {
        self.appeal <= 0.0
    }

    pub fn appeal_fraction(&self) -> f32 {
        if self.max_appeal <= 0.0 {
            return 0.0;
        }
        self.appeal / self.max_appeal
    }

    pub fn effective_allure(&self, scale: f32) -> f32 {
        self.appeal_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wooer() -> Wooer {
        Wooer {
            appeal: 0.0,
            max_appeal: 100.0,
            charm_rate: 10.0,
            just_smitten: false,
            just_spurned: false,
            enabled: true,
        }
    }

    #[test]
    fn default_appeal_zero() {
        let w = Wooer::default();
        assert_eq!(w.appeal, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wooer::default().enabled);
    }

    #[test]
    fn woo_increases_appeal() {
        let mut w = wooer();
        w.woo(30.0);
        assert_eq!(w.appeal, 30.0);
    }

    #[test]
    fn woo_clamps_at_max() {
        let mut w = wooer();
        w.woo(200.0);
        assert_eq!(w.appeal, 100.0);
    }

    #[test]
    fn woo_no_op_when_disabled() {
        let mut w = wooer();
        w.enabled = false;
        w.woo(50.0);
        assert_eq!(w.appeal, 0.0);
    }

    #[test]
    fn woo_sets_just_smitten_at_max() {
        let mut w = wooer();
        w.woo(100.0);
        assert!(w.just_smitten);
    }

    #[test]
    fn woo_no_just_smitten_if_already_max() {
        let mut w = wooer();
        w.appeal = 100.0;
        w.woo(1.0);
        assert!(!w.just_smitten);
    }

    #[test]
    fn spurn_decreases_appeal() {
        let mut w = wooer();
        w.appeal = 60.0;
        w.spurn(20.0);
        assert_eq!(w.appeal, 40.0);
    }

    #[test]
    fn spurn_clamps_at_zero() {
        let mut w = wooer();
        w.appeal = 30.0;
        w.spurn(200.0);
        assert_eq!(w.appeal, 0.0);
    }

    #[test]
    fn spurn_no_op_when_disabled() {
        let mut w = wooer();
        w.appeal = 50.0;
        w.enabled = false;
        w.spurn(10.0);
        assert_eq!(w.appeal, 50.0);
    }

    #[test]
    fn spurn_no_op_when_already_spurned() {
        let mut w = wooer();
        w.spurn(10.0);
        assert_eq!(w.appeal, 0.0);
    }

    #[test]
    fn spurn_sets_just_spurned_at_zero() {
        let mut w = wooer();
        w.appeal = 10.0;
        w.spurn(10.0);
        assert!(w.just_spurned);
    }

    #[test]
    fn spurn_no_just_spurned_if_already_zero() {
        let mut w = wooer();
        w.spurn(1.0);
        assert!(!w.just_spurned);
    }

    #[test]
    fn tick_increases_appeal() {
        let mut w = wooer();
        w.tick(1.0);
        assert_eq!(w.appeal, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wooer();
        w.tick(2.0);
        assert_eq!(w.appeal, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wooer();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.appeal, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_smitten() {
        let mut w = wooer();
        w.appeal = 100.0;
        w.tick(1.0);
        assert_eq!(w.appeal, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wooer();
        w.charm_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.appeal, 0.0);
    }

    #[test]
    fn is_smitten_true_at_max() {
        let mut w = wooer();
        w.appeal = 100.0;
        assert!(w.is_smitten());
    }

    #[test]
    fn is_smitten_false_below_max() {
        let mut w = wooer();
        w.appeal = 50.0;
        assert!(!w.is_smitten());
    }

    #[test]
    fn is_smitten_false_when_disabled() {
        let mut w = wooer();
        w.appeal = 100.0;
        w.enabled = false;
        assert!(!w.is_smitten());
    }

    #[test]
    fn is_spurned_true_at_zero() {
        let w = wooer();
        assert!(w.is_spurned());
    }

    #[test]
    fn is_spurned_false_above_zero() {
        let mut w = wooer();
        w.appeal = 1.0;
        assert!(!w.is_spurned());
    }

    #[test]
    fn appeal_fraction_zero_when_spurned() {
        let w = wooer();
        assert_eq!(w.appeal_fraction(), 0.0);
    }

    #[test]
    fn appeal_fraction_one_at_max() {
        let mut w = wooer();
        w.appeal = 100.0;
        assert_eq!(w.appeal_fraction(), 1.0);
    }

    #[test]
    fn appeal_fraction_half_at_midpoint() {
        let mut w = wooer();
        w.appeal = 50.0;
        assert_eq!(w.appeal_fraction(), 0.5);
    }

    #[test]
    fn appeal_fraction_zero_when_max_zero() {
        let mut w = wooer();
        w.max_appeal = 0.0;
        assert_eq!(w.appeal_fraction(), 0.0);
    }

    #[test]
    fn effective_allure_scales() {
        let mut w = wooer();
        w.appeal = 50.0;
        assert_eq!(w.effective_allure(2.0), 1.0);
    }

    #[test]
    fn effective_allure_zero_when_spurned() {
        let w = wooer();
        assert_eq!(w.effective_allure(10.0), 0.0);
    }

    #[test]
    fn just_smitten_cleared_on_next_woo() {
        let mut w = wooer();
        w.woo(100.0);
        assert!(w.just_smitten);
        w.woo(1.0);
        assert!(!w.just_smitten);
    }

    #[test]
    fn just_spurned_cleared_on_next_spurn() {
        let mut w = wooer();
        w.appeal = 10.0;
        w.spurn(10.0);
        assert!(w.just_spurned);
        w.appeal = 10.0;
        w.spurn(1.0);
        assert!(!w.just_spurned);
    }
}
