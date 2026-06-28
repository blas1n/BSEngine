use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Whim {
    pub impulse: f32,
    pub max_impulse: f32,
    pub whim_rate: f32,
    pub just_whimsy: bool,
    pub just_grounded: bool,
    pub enabled: bool,
}

impl Default for Whim {
    fn default() -> Self {
        Self {
            impulse: 0.0,
            max_impulse: 100.0,
            whim_rate: 1.0,
            just_whimsy: false,
            just_grounded: false,
            enabled: true,
        }
    }
}

impl Whim {
    pub fn fancy(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_whimsy = false;
        self.just_grounded = false;
        let prev = self.impulse;
        self.impulse = (self.impulse + amount).clamp(0.0, self.max_impulse);
        if self.impulse >= self.max_impulse && prev < self.max_impulse {
            self.just_whimsy = true;
        }
    }

    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || self.impulse <= 0.0 {
            return;
        }
        self.just_whimsy = false;
        self.just_grounded = false;
        let prev = self.impulse;
        self.impulse = (self.impulse - amount).max(0.0);
        if self.impulse <= 0.0 && prev > 0.0 {
            self.just_grounded = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.impulse >= self.max_impulse {
            return;
        }
        self.fancy(self.whim_rate * dt);
    }

    pub fn is_whimsy(&self) -> bool {
        self.enabled && self.impulse >= self.max_impulse
    }

    pub fn is_grounded(&self) -> bool {
        self.impulse <= 0.0
    }

    pub fn impulse_fraction(&self) -> f32 {
        if self.max_impulse <= 0.0 {
            return 0.0;
        }
        self.impulse / self.max_impulse
    }

    pub fn effective_caprice(&self, scale: f32) -> f32 {
        self.impulse_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whim() -> Whim {
        Whim {
            impulse: 0.0,
            max_impulse: 100.0,
            whim_rate: 10.0,
            just_whimsy: false,
            just_grounded: false,
            enabled: true,
        }
    }

    #[test]
    fn default_impulse_zero() {
        let w = Whim::default();
        assert_eq!(w.impulse, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Whim::default().enabled);
    }

    #[test]
    fn fancy_increases_impulse() {
        let mut w = whim();
        w.fancy(30.0);
        assert_eq!(w.impulse, 30.0);
    }

    #[test]
    fn fancy_clamps_at_max() {
        let mut w = whim();
        w.fancy(200.0);
        assert_eq!(w.impulse, 100.0);
    }

    #[test]
    fn fancy_no_op_when_disabled() {
        let mut w = whim();
        w.enabled = false;
        w.fancy(50.0);
        assert_eq!(w.impulse, 0.0);
    }

    #[test]
    fn fancy_sets_just_whimsy_at_max() {
        let mut w = whim();
        w.fancy(100.0);
        assert!(w.just_whimsy);
    }

    #[test]
    fn fancy_no_just_whimsy_if_already_max() {
        let mut w = whim();
        w.impulse = 100.0;
        w.fancy(1.0);
        assert!(!w.just_whimsy);
    }

    #[test]
    fn settle_decreases_impulse() {
        let mut w = whim();
        w.impulse = 60.0;
        w.settle(20.0);
        assert_eq!(w.impulse, 40.0);
    }

    #[test]
    fn settle_clamps_at_zero() {
        let mut w = whim();
        w.impulse = 30.0;
        w.settle(200.0);
        assert_eq!(w.impulse, 0.0);
    }

    #[test]
    fn settle_no_op_when_disabled() {
        let mut w = whim();
        w.impulse = 50.0;
        w.enabled = false;
        w.settle(10.0);
        assert_eq!(w.impulse, 50.0);
    }

    #[test]
    fn settle_no_op_when_already_zero() {
        let mut w = whim();
        w.settle(10.0);
        assert_eq!(w.impulse, 0.0);
    }

    #[test]
    fn settle_sets_just_grounded_at_zero() {
        let mut w = whim();
        w.impulse = 10.0;
        w.settle(10.0);
        assert!(w.just_grounded);
    }

    #[test]
    fn settle_no_just_grounded_if_already_zero() {
        let mut w = whim();
        w.settle(1.0);
        assert!(!w.just_grounded);
    }

    #[test]
    fn tick_increases_impulse() {
        let mut w = whim();
        w.tick(1.0);
        assert_eq!(w.impulse, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = whim();
        w.tick(2.0);
        assert_eq!(w.impulse, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = whim();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.impulse, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_whimsy() {
        let mut w = whim();
        w.impulse = 100.0;
        w.tick(1.0);
        assert_eq!(w.impulse, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = whim();
        w.whim_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.impulse, 0.0);
    }

    #[test]
    fn is_whimsy_true_at_max() {
        let mut w = whim();
        w.impulse = 100.0;
        assert!(w.is_whimsy());
    }

    #[test]
    fn is_whimsy_false_below_max() {
        let mut w = whim();
        w.impulse = 50.0;
        assert!(!w.is_whimsy());
    }

    #[test]
    fn is_whimsy_false_when_disabled() {
        let mut w = whim();
        w.impulse = 100.0;
        w.enabled = false;
        assert!(!w.is_whimsy());
    }

    #[test]
    fn is_grounded_true_at_zero() {
        let w = whim();
        assert!(w.is_grounded());
    }

    #[test]
    fn is_grounded_false_above_zero() {
        let mut w = whim();
        w.impulse = 1.0;
        assert!(!w.is_grounded());
    }

    #[test]
    fn impulse_fraction_zero_when_grounded() {
        let w = whim();
        assert_eq!(w.impulse_fraction(), 0.0);
    }

    #[test]
    fn impulse_fraction_one_at_max() {
        let mut w = whim();
        w.impulse = 100.0;
        assert_eq!(w.impulse_fraction(), 1.0);
    }

    #[test]
    fn impulse_fraction_half_at_midpoint() {
        let mut w = whim();
        w.impulse = 50.0;
        assert_eq!(w.impulse_fraction(), 0.5);
    }

    #[test]
    fn impulse_fraction_zero_when_max_zero() {
        let mut w = whim();
        w.max_impulse = 0.0;
        assert_eq!(w.impulse_fraction(), 0.0);
    }

    #[test]
    fn effective_caprice_scales() {
        let mut w = whim();
        w.impulse = 50.0;
        assert_eq!(w.effective_caprice(2.0), 1.0);
    }

    #[test]
    fn effective_caprice_zero_when_grounded() {
        let w = whim();
        assert_eq!(w.effective_caprice(10.0), 0.0);
    }

    #[test]
    fn just_whimsy_cleared_on_next_fancy() {
        let mut w = whim();
        w.fancy(100.0);
        assert!(w.just_whimsy);
        w.fancy(1.0);
        assert!(!w.just_whimsy);
    }

    #[test]
    fn just_grounded_cleared_on_next_settle() {
        let mut w = whim();
        w.impulse = 10.0;
        w.settle(10.0);
        assert!(w.just_grounded);
        w.impulse = 10.0;
        w.settle(1.0);
        assert!(!w.just_grounded);
    }
}
