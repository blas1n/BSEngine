use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Whack {
    pub impact: f32,
    pub max_impact: f32,
    pub strike_rate: f32,
    pub just_struck: bool,
    pub just_glanced: bool,
    pub enabled: bool,
}

impl Default for Whack {
    fn default() -> Self {
        Self {
            impact: 0.0,
            max_impact: 100.0,
            strike_rate: 1.0,
            just_struck: false,
            just_glanced: false,
            enabled: true,
        }
    }
}

impl Whack {
    pub fn strike(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_struck = false;
        self.just_glanced = false;
        let prev = self.impact;
        self.impact = (self.impact + amount).clamp(0.0, self.max_impact);
        if self.impact >= self.max_impact && prev < self.max_impact {
            self.just_struck = true;
        }
    }

    pub fn glance(&mut self, amount: f32) {
        if !self.enabled || self.impact <= 0.0 {
            return;
        }
        self.just_struck = false;
        self.just_glanced = false;
        let prev = self.impact;
        self.impact = (self.impact - amount).max(0.0);
        if self.impact <= 0.0 && prev > 0.0 {
            self.just_glanced = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.impact >= self.max_impact {
            return;
        }
        self.strike(self.strike_rate * dt);
    }

    pub fn is_struck(&self) -> bool {
        self.enabled && self.impact >= self.max_impact
    }

    pub fn is_glanced(&self) -> bool {
        self.impact <= 0.0
    }

    pub fn impact_fraction(&self) -> f32 {
        if self.max_impact <= 0.0 {
            return 0.0;
        }
        self.impact / self.max_impact
    }

    pub fn effective_blow(&self, scale: f32) -> f32 {
        self.impact_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whack() -> Whack {
        Whack {
            impact: 0.0,
            max_impact: 100.0,
            strike_rate: 10.0,
            just_struck: false,
            just_glanced: false,
            enabled: true,
        }
    }

    #[test]
    fn default_impact_zero() {
        let w = Whack::default();
        assert_eq!(w.impact, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Whack::default().enabled);
    }

    #[test]
    fn strike_increases_impact() {
        let mut w = whack();
        w.strike(30.0);
        assert_eq!(w.impact, 30.0);
    }

    #[test]
    fn strike_clamps_at_max() {
        let mut w = whack();
        w.strike(200.0);
        assert_eq!(w.impact, 100.0);
    }

    #[test]
    fn strike_no_op_when_disabled() {
        let mut w = whack();
        w.enabled = false;
        w.strike(50.0);
        assert_eq!(w.impact, 0.0);
    }

    #[test]
    fn strike_sets_just_struck_at_max() {
        let mut w = whack();
        w.strike(100.0);
        assert!(w.just_struck);
    }

    #[test]
    fn strike_no_just_struck_if_already_max() {
        let mut w = whack();
        w.impact = 100.0;
        w.strike(1.0);
        assert!(!w.just_struck);
    }

    #[test]
    fn glance_decreases_impact() {
        let mut w = whack();
        w.impact = 60.0;
        w.glance(20.0);
        assert_eq!(w.impact, 40.0);
    }

    #[test]
    fn glance_clamps_at_zero() {
        let mut w = whack();
        w.impact = 30.0;
        w.glance(200.0);
        assert_eq!(w.impact, 0.0);
    }

    #[test]
    fn glance_no_op_when_disabled() {
        let mut w = whack();
        w.impact = 50.0;
        w.enabled = false;
        w.glance(10.0);
        assert_eq!(w.impact, 50.0);
    }

    #[test]
    fn glance_no_op_when_already_glanced() {
        let mut w = whack();
        w.glance(10.0);
        assert_eq!(w.impact, 0.0);
    }

    #[test]
    fn glance_sets_just_glanced_at_zero() {
        let mut w = whack();
        w.impact = 10.0;
        w.glance(10.0);
        assert!(w.just_glanced);
    }

    #[test]
    fn glance_no_just_glanced_if_already_zero() {
        let mut w = whack();
        w.glance(1.0);
        assert!(!w.just_glanced);
    }

    #[test]
    fn tick_increases_impact() {
        let mut w = whack();
        w.tick(1.0);
        assert_eq!(w.impact, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = whack();
        w.tick(2.0);
        assert_eq!(w.impact, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = whack();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.impact, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_struck() {
        let mut w = whack();
        w.impact = 100.0;
        w.tick(1.0);
        assert_eq!(w.impact, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = whack();
        w.strike_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.impact, 0.0);
    }

    #[test]
    fn is_struck_true_at_max() {
        let mut w = whack();
        w.impact = 100.0;
        assert!(w.is_struck());
    }

    #[test]
    fn is_struck_false_below_max() {
        let mut w = whack();
        w.impact = 50.0;
        assert!(!w.is_struck());
    }

    #[test]
    fn is_struck_false_when_disabled() {
        let mut w = whack();
        w.impact = 100.0;
        w.enabled = false;
        assert!(!w.is_struck());
    }

    #[test]
    fn is_glanced_true_at_zero() {
        let w = whack();
        assert!(w.is_glanced());
    }

    #[test]
    fn is_glanced_false_above_zero() {
        let mut w = whack();
        w.impact = 1.0;
        assert!(!w.is_glanced());
    }

    #[test]
    fn impact_fraction_zero_when_glanced() {
        let w = whack();
        assert_eq!(w.impact_fraction(), 0.0);
    }

    #[test]
    fn impact_fraction_one_at_max() {
        let mut w = whack();
        w.impact = 100.0;
        assert_eq!(w.impact_fraction(), 1.0);
    }

    #[test]
    fn impact_fraction_half_at_midpoint() {
        let mut w = whack();
        w.impact = 50.0;
        assert_eq!(w.impact_fraction(), 0.5);
    }

    #[test]
    fn impact_fraction_zero_when_max_zero() {
        let mut w = whack();
        w.max_impact = 0.0;
        assert_eq!(w.impact_fraction(), 0.0);
    }

    #[test]
    fn effective_blow_scales() {
        let mut w = whack();
        w.impact = 50.0;
        assert_eq!(w.effective_blow(2.0), 1.0);
    }

    #[test]
    fn effective_blow_zero_when_glanced() {
        let w = whack();
        assert_eq!(w.effective_blow(10.0), 0.0);
    }

    #[test]
    fn just_struck_cleared_on_next_strike() {
        let mut w = whack();
        w.strike(100.0);
        assert!(w.just_struck);
        w.strike(1.0);
        assert!(!w.just_struck);
    }

    #[test]
    fn just_glanced_cleared_on_next_glance() {
        let mut w = whack();
        w.impact = 10.0;
        w.glance(10.0);
        assert!(w.just_glanced);
        w.impact = 10.0;
        w.glance(1.0);
        assert!(!w.just_glanced);
    }
}
