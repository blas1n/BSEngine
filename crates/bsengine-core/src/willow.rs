use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Willow {
    pub sway: f32,
    pub max_sway: f32,
    pub bend_rate: f32,
    pub just_swaying: bool,
    pub just_still: bool,
    pub enabled: bool,
}

impl Default for Willow {
    fn default() -> Self {
        Self {
            sway: 0.0,
            max_sway: 100.0,
            bend_rate: 1.0,
            just_swaying: false,
            just_still: false,
            enabled: true,
        }
    }
}

impl Willow {
    pub fn bend(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_swaying = false;
        self.just_still = false;
        let prev = self.sway;
        self.sway = (self.sway + amount).clamp(0.0, self.max_sway);
        if self.sway >= self.max_sway && prev < self.max_sway {
            self.just_swaying = true;
        }
    }

    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || self.sway <= 0.0 {
            return;
        }
        self.just_swaying = false;
        self.just_still = false;
        let prev = self.sway;
        self.sway = (self.sway - amount).max(0.0);
        if self.sway <= 0.0 && prev > 0.0 {
            self.just_still = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.sway >= self.max_sway {
            return;
        }
        self.bend(self.bend_rate * dt);
    }

    pub fn is_swaying(&self) -> bool {
        self.enabled && self.sway >= self.max_sway
    }

    pub fn is_still(&self) -> bool {
        self.sway <= 0.0
    }

    pub fn sway_fraction(&self) -> f32 {
        if self.max_sway <= 0.0 {
            return 0.0;
        }
        self.sway / self.max_sway
    }

    pub fn effective_flex(&self, scale: f32) -> f32 {
        self.sway_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn willow() -> Willow {
        Willow {
            sway: 0.0,
            max_sway: 100.0,
            bend_rate: 10.0,
            just_swaying: false,
            just_still: false,
            enabled: true,
        }
    }

    #[test]
    fn default_sway_zero() {
        let w = Willow::default();
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Willow::default().enabled);
    }

    #[test]
    fn bend_increases_sway() {
        let mut w = willow();
        w.bend(30.0);
        assert_eq!(w.sway, 30.0);
    }

    #[test]
    fn bend_clamps_at_max() {
        let mut w = willow();
        w.bend(200.0);
        assert_eq!(w.sway, 100.0);
    }

    #[test]
    fn bend_no_op_when_disabled() {
        let mut w = willow();
        w.enabled = false;
        w.bend(50.0);
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn bend_sets_just_swaying_at_max() {
        let mut w = willow();
        w.bend(100.0);
        assert!(w.just_swaying);
    }

    #[test]
    fn bend_no_just_swaying_if_already_max() {
        let mut w = willow();
        w.sway = 100.0;
        w.bend(1.0);
        assert!(!w.just_swaying);
    }

    #[test]
    fn settle_decreases_sway() {
        let mut w = willow();
        w.sway = 60.0;
        w.settle(20.0);
        assert_eq!(w.sway, 40.0);
    }

    #[test]
    fn settle_clamps_at_zero() {
        let mut w = willow();
        w.sway = 30.0;
        w.settle(200.0);
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn settle_no_op_when_disabled() {
        let mut w = willow();
        w.sway = 50.0;
        w.enabled = false;
        w.settle(10.0);
        assert_eq!(w.sway, 50.0);
    }

    #[test]
    fn settle_no_op_when_already_still() {
        let mut w = willow();
        w.settle(10.0);
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn settle_sets_just_still_at_zero() {
        let mut w = willow();
        w.sway = 10.0;
        w.settle(10.0);
        assert!(w.just_still);
    }

    #[test]
    fn settle_no_just_still_if_already_zero() {
        let mut w = willow();
        w.settle(1.0);
        assert!(!w.just_still);
    }

    #[test]
    fn tick_increases_sway() {
        let mut w = willow();
        w.tick(1.0);
        assert_eq!(w.sway, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = willow();
        w.tick(2.0);
        assert_eq!(w.sway, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = willow();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_swaying() {
        let mut w = willow();
        w.sway = 100.0;
        w.tick(1.0);
        assert_eq!(w.sway, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = willow();
        w.bend_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.sway, 0.0);
    }

    #[test]
    fn is_swaying_true_at_max() {
        let mut w = willow();
        w.sway = 100.0;
        assert!(w.is_swaying());
    }

    #[test]
    fn is_swaying_false_below_max() {
        let mut w = willow();
        w.sway = 50.0;
        assert!(!w.is_swaying());
    }

    #[test]
    fn is_swaying_false_when_disabled() {
        let mut w = willow();
        w.sway = 100.0;
        w.enabled = false;
        assert!(!w.is_swaying());
    }

    #[test]
    fn is_still_true_at_zero() {
        let w = willow();
        assert!(w.is_still());
    }

    #[test]
    fn is_still_false_above_zero() {
        let mut w = willow();
        w.sway = 1.0;
        assert!(!w.is_still());
    }

    #[test]
    fn sway_fraction_zero_when_still() {
        let w = willow();
        assert_eq!(w.sway_fraction(), 0.0);
    }

    #[test]
    fn sway_fraction_one_at_max() {
        let mut w = willow();
        w.sway = 100.0;
        assert_eq!(w.sway_fraction(), 1.0);
    }

    #[test]
    fn sway_fraction_half_at_midpoint() {
        let mut w = willow();
        w.sway = 50.0;
        assert_eq!(w.sway_fraction(), 0.5);
    }

    #[test]
    fn sway_fraction_zero_when_max_zero() {
        let mut w = willow();
        w.max_sway = 0.0;
        assert_eq!(w.sway_fraction(), 0.0);
    }

    #[test]
    fn effective_flex_scales() {
        let mut w = willow();
        w.sway = 50.0;
        assert_eq!(w.effective_flex(2.0), 1.0);
    }

    #[test]
    fn effective_flex_zero_when_still() {
        let w = willow();
        assert_eq!(w.effective_flex(10.0), 0.0);
    }

    #[test]
    fn just_swaying_cleared_on_next_bend() {
        let mut w = willow();
        w.bend(100.0);
        assert!(w.just_swaying);
        w.bend(1.0);
        assert!(!w.just_swaying);
    }

    #[test]
    fn just_still_cleared_on_next_settle() {
        let mut w = willow();
        w.sway = 10.0;
        w.settle(10.0);
        assert!(w.just_still);
        w.sway = 10.0;
        w.settle(1.0);
        assert!(!w.just_still);
    }
}
