use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Winch {
    pub tension: f32,
    pub max_tension: f32,
    pub crank_rate: f32,
    pub just_taut: bool,
    pub just_slack: bool,
    pub enabled: bool,
}

impl Default for Winch {
    fn default() -> Self {
        Self {
            tension: 0.0,
            max_tension: 100.0,
            crank_rate: 1.0,
            just_taut: false,
            just_slack: false,
            enabled: true,
        }
    }
}

impl Winch {
    pub fn crank(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_taut = false;
        self.just_slack = false;
        let prev = self.tension;
        self.tension = (self.tension + amount).clamp(0.0, self.max_tension);
        if self.tension >= self.max_tension && prev < self.max_tension {
            self.just_taut = true;
        }
    }

    pub fn release(&mut self, amount: f32) {
        if !self.enabled || self.tension <= 0.0 {
            return;
        }
        self.just_taut = false;
        self.just_slack = false;
        let prev = self.tension;
        self.tension = (self.tension - amount).max(0.0);
        if self.tension <= 0.0 && prev > 0.0 {
            self.just_slack = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.tension >= self.max_tension {
            return;
        }
        self.crank(self.crank_rate * dt);
    }

    pub fn is_taut(&self) -> bool {
        self.enabled && self.tension >= self.max_tension
    }

    pub fn is_slack(&self) -> bool {
        self.tension <= 0.0
    }

    pub fn tension_fraction(&self) -> f32 {
        if self.max_tension <= 0.0 {
            return 0.0;
        }
        self.tension / self.max_tension
    }

    pub fn effective_pull(&self, scale: f32) -> f32 {
        self.tension_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn winch() -> Winch {
        Winch {
            tension: 0.0,
            max_tension: 100.0,
            crank_rate: 10.0,
            just_taut: false,
            just_slack: false,
            enabled: true,
        }
    }

    #[test]
    fn default_tension_zero() {
        let w = Winch::default();
        assert_eq!(w.tension, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Winch::default().enabled);
    }

    #[test]
    fn crank_increases_tension() {
        let mut w = winch();
        w.crank(30.0);
        assert_eq!(w.tension, 30.0);
    }

    #[test]
    fn crank_clamps_at_max() {
        let mut w = winch();
        w.crank(200.0);
        assert_eq!(w.tension, 100.0);
    }

    #[test]
    fn crank_no_op_when_disabled() {
        let mut w = winch();
        w.enabled = false;
        w.crank(50.0);
        assert_eq!(w.tension, 0.0);
    }

    #[test]
    fn crank_sets_just_taut_at_max() {
        let mut w = winch();
        w.crank(100.0);
        assert!(w.just_taut);
    }

    #[test]
    fn crank_no_just_taut_if_already_max() {
        let mut w = winch();
        w.tension = 100.0;
        w.crank(1.0);
        assert!(!w.just_taut);
    }

    #[test]
    fn release_decreases_tension() {
        let mut w = winch();
        w.tension = 60.0;
        w.release(20.0);
        assert_eq!(w.tension, 40.0);
    }

    #[test]
    fn release_clamps_at_zero() {
        let mut w = winch();
        w.tension = 30.0;
        w.release(200.0);
        assert_eq!(w.tension, 0.0);
    }

    #[test]
    fn release_no_op_when_disabled() {
        let mut w = winch();
        w.tension = 50.0;
        w.enabled = false;
        w.release(10.0);
        assert_eq!(w.tension, 50.0);
    }

    #[test]
    fn release_no_op_when_already_slack() {
        let mut w = winch();
        w.release(10.0);
        assert_eq!(w.tension, 0.0);
    }

    #[test]
    fn release_sets_just_slack_at_zero() {
        let mut w = winch();
        w.tension = 10.0;
        w.release(10.0);
        assert!(w.just_slack);
    }

    #[test]
    fn release_no_just_slack_if_already_zero() {
        let mut w = winch();
        w.release(1.0);
        assert!(!w.just_slack);
    }

    #[test]
    fn tick_increases_tension() {
        let mut w = winch();
        w.tick(1.0);
        assert_eq!(w.tension, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = winch();
        w.tick(2.0);
        assert_eq!(w.tension, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = winch();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.tension, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_taut() {
        let mut w = winch();
        w.tension = 100.0;
        w.tick(1.0);
        assert_eq!(w.tension, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = winch();
        w.crank_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.tension, 0.0);
    }

    #[test]
    fn is_taut_true_at_max() {
        let mut w = winch();
        w.tension = 100.0;
        assert!(w.is_taut());
    }

    #[test]
    fn is_taut_false_below_max() {
        let mut w = winch();
        w.tension = 50.0;
        assert!(!w.is_taut());
    }

    #[test]
    fn is_taut_false_when_disabled() {
        let mut w = winch();
        w.tension = 100.0;
        w.enabled = false;
        assert!(!w.is_taut());
    }

    #[test]
    fn is_slack_true_at_zero() {
        let w = winch();
        assert!(w.is_slack());
    }

    #[test]
    fn is_slack_false_above_zero() {
        let mut w = winch();
        w.tension = 1.0;
        assert!(!w.is_slack());
    }

    #[test]
    fn tension_fraction_zero_when_slack() {
        let w = winch();
        assert_eq!(w.tension_fraction(), 0.0);
    }

    #[test]
    fn tension_fraction_one_at_max() {
        let mut w = winch();
        w.tension = 100.0;
        assert_eq!(w.tension_fraction(), 1.0);
    }

    #[test]
    fn tension_fraction_half_at_midpoint() {
        let mut w = winch();
        w.tension = 50.0;
        assert_eq!(w.tension_fraction(), 0.5);
    }

    #[test]
    fn tension_fraction_zero_when_max_zero() {
        let mut w = winch();
        w.max_tension = 0.0;
        assert_eq!(w.tension_fraction(), 0.0);
    }

    #[test]
    fn effective_pull_scales() {
        let mut w = winch();
        w.tension = 50.0;
        assert_eq!(w.effective_pull(2.0), 1.0);
    }

    #[test]
    fn effective_pull_zero_when_slack() {
        let w = winch();
        assert_eq!(w.effective_pull(10.0), 0.0);
    }

    #[test]
    fn just_taut_cleared_on_next_crank() {
        let mut w = winch();
        w.crank(100.0);
        assert!(w.just_taut);
        w.crank(1.0);
        assert!(!w.just_taut);
    }

    #[test]
    fn just_slack_cleared_on_next_release() {
        let mut w = winch();
        w.tension = 10.0;
        w.release(10.0);
        assert!(w.just_slack);
        w.tension = 10.0;
        w.release(1.0);
        assert!(!w.just_slack);
    }
}
