use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Waddle {
    pub sway: f32,
    pub max_sway: f32,
    pub toddle_rate: f32,
    pub just_listing: bool,
    pub just_upright: bool,
    pub enabled: bool,
}

impl Default for Waddle {
    fn default() -> Self {
        Self {
            sway: 0.0,
            max_sway: 100.0,
            toddle_rate: 1.0,
            just_listing: false,
            just_upright: false,
            enabled: true,
        }
    }
}

impl Waddle {
    pub fn toddle(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_listing = false;
        self.just_upright = false;
        let prev = self.sway;
        self.sway = (self.sway + amount).clamp(0.0, self.max_sway);
        if self.sway >= self.max_sway && prev < self.max_sway {
            self.just_listing = true;
        }
    }

    pub fn steady(&mut self, amount: f32) {
        if !self.enabled || self.sway <= 0.0 {
            return;
        }
        self.just_listing = false;
        self.just_upright = false;
        let prev = self.sway;
        self.sway = (self.sway - amount).max(0.0);
        if self.sway <= 0.0 && prev > 0.0 {
            self.just_upright = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.sway >= self.max_sway {
            return;
        }
        self.toddle(self.toddle_rate * dt);
    }

    pub fn is_listing(&self) -> bool {
        self.enabled && self.sway >= self.max_sway
    }

    pub fn is_upright(&self) -> bool {
        self.sway <= 0.0
    }

    pub fn sway_fraction(&self) -> f32 {
        if self.max_sway <= 0.0 {
            return 0.0;
        }
        self.sway / self.max_sway
    }

    pub fn effective_lurch(&self, scale: f32) -> f32 {
        self.sway_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waddle() -> Waddle {
        Waddle {
            sway: 0.0,
            max_sway: 100.0,
            toddle_rate: 10.0,
            just_listing: false,
            just_upright: false,
            enabled: true,
        }
    }

    #[test]
    fn default_sway_zero() {
        let w = Waddle::default();
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Waddle::default().enabled);
    }

    #[test]
    fn toddle_increases_sway() {
        let mut w = waddle();
        w.toddle(30.0);
        assert_eq!(w.sway, 30.0);
    }

    #[test]
    fn toddle_clamps_at_max() {
        let mut w = waddle();
        w.toddle(200.0);
        assert_eq!(w.sway, 100.0);
    }

    #[test]
    fn toddle_no_op_when_disabled() {
        let mut w = waddle();
        w.enabled = false;
        w.toddle(50.0);
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn toddle_sets_just_listing_at_max() {
        let mut w = waddle();
        w.toddle(100.0);
        assert!(w.just_listing);
    }

    #[test]
    fn toddle_no_just_listing_if_already_max() {
        let mut w = waddle();
        w.sway = 100.0;
        w.toddle(1.0);
        assert!(!w.just_listing);
    }

    #[test]
    fn steady_decreases_sway() {
        let mut w = waddle();
        w.sway = 60.0;
        w.steady(20.0);
        assert_eq!(w.sway, 40.0);
    }

    #[test]
    fn steady_clamps_at_zero() {
        let mut w = waddle();
        w.sway = 30.0;
        w.steady(200.0);
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn steady_no_op_when_disabled() {
        let mut w = waddle();
        w.sway = 50.0;
        w.enabled = false;
        w.steady(10.0);
        assert_eq!(w.sway, 50.0);
    }

    #[test]
    fn steady_no_op_when_already_upright() {
        let mut w = waddle();
        w.steady(10.0);
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn steady_sets_just_upright_at_zero() {
        let mut w = waddle();
        w.sway = 10.0;
        w.steady(10.0);
        assert!(w.just_upright);
    }

    #[test]
    fn steady_no_just_upright_if_already_zero() {
        let mut w = waddle();
        w.steady(1.0);
        assert!(!w.just_upright);
    }

    #[test]
    fn tick_increases_sway() {
        let mut w = waddle();
        w.tick(1.0);
        assert_eq!(w.sway, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = waddle();
        w.tick(2.0);
        assert_eq!(w.sway, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = waddle();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_listing() {
        let mut w = waddle();
        w.sway = 100.0;
        w.tick(1.0);
        assert_eq!(w.sway, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = waddle();
        w.toddle_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn is_listing_true_at_max() {
        let mut w = waddle();
        w.sway = 100.0;
        assert!(w.is_listing());
    }

    #[test]
    fn is_listing_false_below_max() {
        let mut w = waddle();
        w.sway = 50.0;
        assert!(!w.is_listing());
    }

    #[test]
    fn is_listing_false_when_disabled() {
        let mut w = waddle();
        w.sway = 100.0;
        w.enabled = false;
        assert!(!w.is_listing());
    }

    #[test]
    fn is_upright_true_at_zero() {
        let w = waddle();
        assert!(w.is_upright());
    }

    #[test]
    fn is_upright_false_above_zero() {
        let mut w = waddle();
        w.sway = 1.0;
        assert!(!w.is_upright());
    }

    #[test]
    fn sway_fraction_zero_when_upright() {
        let w = waddle();
        assert_eq!(w.sway_fraction(), 0.0);
    }

    #[test]
    fn sway_fraction_one_at_max() {
        let mut w = waddle();
        w.sway = 100.0;
        assert_eq!(w.sway_fraction(), 1.0);
    }

    #[test]
    fn sway_fraction_half_at_midpoint() {
        let mut w = waddle();
        w.sway = 50.0;
        assert_eq!(w.sway_fraction(), 0.5);
    }

    #[test]
    fn sway_fraction_zero_when_max_zero() {
        let mut w = waddle();
        w.max_sway = 0.0;
        assert_eq!(w.sway_fraction(), 0.0);
    }

    #[test]
    fn effective_lurch_scales() {
        let mut w = waddle();
        w.sway = 50.0;
        assert_eq!(w.effective_lurch(2.0), 1.0);
    }

    #[test]
    fn effective_lurch_zero_when_upright() {
        let w = waddle();
        assert_eq!(w.effective_lurch(10.0), 0.0);
    }

    #[test]
    fn just_listing_cleared_on_next_toddle() {
        let mut w = waddle();
        w.toddle(100.0);
        assert!(w.just_listing);
        w.toddle(1.0);
        assert!(!w.just_listing);
    }

    #[test]
    fn just_upright_cleared_on_next_steady() {
        let mut w = waddle();
        w.sway = 10.0;
        w.steady(10.0);
        assert!(w.just_upright);
        w.sway = 10.0;
        w.steady(1.0);
        assert!(!w.just_upright);
    }
}
