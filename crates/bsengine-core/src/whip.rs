use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Whip {
    pub lash: f32,
    pub max_lash: f32,
    pub crack_rate: f32,
    pub just_cracking: bool,
    pub just_slack: bool,
    pub enabled: bool,
}

impl Default for Whip {
    fn default() -> Self {
        Self {
            lash: 0.0,
            max_lash: 100.0,
            crack_rate: 1.0,
            just_cracking: false,
            just_slack: false,
            enabled: true,
        }
    }
}

impl Whip {
    pub fn crack(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_cracking = false;
        self.just_slack = false;
        let prev = self.lash;
        self.lash = (self.lash + amount).clamp(0.0, self.max_lash);
        if self.lash >= self.max_lash && prev < self.max_lash {
            self.just_cracking = true;
        }
    }

    pub fn slacken(&mut self, amount: f32) {
        if !self.enabled || self.lash <= 0.0 {
            return;
        }
        self.just_cracking = false;
        self.just_slack = false;
        let prev = self.lash;
        self.lash = (self.lash - amount).max(0.0);
        if self.lash <= 0.0 && prev > 0.0 {
            self.just_slack = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.lash >= self.max_lash {
            return;
        }
        self.crack(self.crack_rate * dt);
    }

    pub fn is_cracking(&self) -> bool {
        self.enabled && self.lash >= self.max_lash
    }

    pub fn is_slack(&self) -> bool {
        self.lash <= 0.0
    }

    pub fn lash_fraction(&self) -> f32 {
        if self.max_lash <= 0.0 {
            return 0.0;
        }
        self.lash / self.max_lash
    }

    pub fn effective_sting(&self, scale: f32) -> f32 {
        self.lash_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whip() -> Whip {
        Whip {
            lash: 0.0,
            max_lash: 100.0,
            crack_rate: 10.0,
            just_cracking: false,
            just_slack: false,
            enabled: true,
        }
    }

    #[test]
    fn default_lash_zero() {
        let w = Whip::default();
        assert_eq!(w.lash, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Whip::default().enabled);
    }

    #[test]
    fn crack_increases_lash() {
        let mut w = whip();
        w.crack(30.0);
        assert_eq!(w.lash, 30.0);
    }

    #[test]
    fn crack_clamps_at_max() {
        let mut w = whip();
        w.crack(200.0);
        assert_eq!(w.lash, 100.0);
    }

    #[test]
    fn crack_no_op_when_disabled() {
        let mut w = whip();
        w.enabled = false;
        w.crack(50.0);
        assert_eq!(w.lash, 0.0);
    }

    #[test]
    fn crack_sets_just_cracking_at_max() {
        let mut w = whip();
        w.crack(100.0);
        assert!(w.just_cracking);
    }

    #[test]
    fn crack_no_just_cracking_if_already_max() {
        let mut w = whip();
        w.lash = 100.0;
        w.crack(1.0);
        assert!(!w.just_cracking);
    }

    #[test]
    fn slacken_decreases_lash() {
        let mut w = whip();
        w.lash = 60.0;
        w.slacken(20.0);
        assert_eq!(w.lash, 40.0);
    }

    #[test]
    fn slacken_clamps_at_zero() {
        let mut w = whip();
        w.lash = 30.0;
        w.slacken(200.0);
        assert_eq!(w.lash, 0.0);
    }

    #[test]
    fn slacken_no_op_when_disabled() {
        let mut w = whip();
        w.lash = 50.0;
        w.enabled = false;
        w.slacken(10.0);
        assert_eq!(w.lash, 50.0);
    }

    #[test]
    fn slacken_no_op_when_already_slack() {
        let mut w = whip();
        w.slacken(10.0);
        assert_eq!(w.lash, 0.0);
    }

    #[test]
    fn slacken_sets_just_slack_at_zero() {
        let mut w = whip();
        w.lash = 10.0;
        w.slacken(10.0);
        assert!(w.just_slack);
    }

    #[test]
    fn slacken_no_just_slack_if_already_zero() {
        let mut w = whip();
        w.slacken(1.0);
        assert!(!w.just_slack);
    }

    #[test]
    fn tick_increases_lash() {
        let mut w = whip();
        w.tick(1.0);
        assert_eq!(w.lash, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = whip();
        w.tick(2.0);
        assert_eq!(w.lash, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = whip();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.lash, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_cracking() {
        let mut w = whip();
        w.lash = 100.0;
        w.tick(1.0);
        assert_eq!(w.lash, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = whip();
        w.crack_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.lash, 0.0);
    }

    #[test]
    fn is_cracking_true_at_max() {
        let mut w = whip();
        w.lash = 100.0;
        assert!(w.is_cracking());
    }

    #[test]
    fn is_cracking_false_below_max() {
        let mut w = whip();
        w.lash = 50.0;
        assert!(!w.is_cracking());
    }

    #[test]
    fn is_cracking_false_when_disabled() {
        let mut w = whip();
        w.lash = 100.0;
        w.enabled = false;
        assert!(!w.is_cracking());
    }

    #[test]
    fn is_slack_true_at_zero() {
        let w = whip();
        assert!(w.is_slack());
    }

    #[test]
    fn is_slack_false_above_zero() {
        let mut w = whip();
        w.lash = 1.0;
        assert!(!w.is_slack());
    }

    #[test]
    fn lash_fraction_zero_when_slack() {
        let w = whip();
        assert_eq!(w.lash_fraction(), 0.0);
    }

    #[test]
    fn lash_fraction_one_at_max() {
        let mut w = whip();
        w.lash = 100.0;
        assert_eq!(w.lash_fraction(), 1.0);
    }

    #[test]
    fn lash_fraction_half_at_midpoint() {
        let mut w = whip();
        w.lash = 50.0;
        assert_eq!(w.lash_fraction(), 0.5);
    }

    #[test]
    fn lash_fraction_zero_when_max_zero() {
        let mut w = whip();
        w.max_lash = 0.0;
        assert_eq!(w.lash_fraction(), 0.0);
    }

    #[test]
    fn effective_sting_scales() {
        let mut w = whip();
        w.lash = 50.0;
        assert_eq!(w.effective_sting(2.0), 1.0);
    }

    #[test]
    fn effective_sting_zero_when_slack() {
        let w = whip();
        assert_eq!(w.effective_sting(10.0), 0.0);
    }

    #[test]
    fn just_cracking_cleared_on_next_crack() {
        let mut w = whip();
        w.crack(100.0);
        assert!(w.just_cracking);
        w.crack(1.0);
        assert!(!w.just_cracking);
    }

    #[test]
    fn just_slack_cleared_on_next_slacken() {
        let mut w = whip();
        w.lash = 10.0;
        w.slacken(10.0);
        assert!(w.just_slack);
        w.lash = 10.0;
        w.slacken(1.0);
        assert!(!w.just_slack);
    }
}
