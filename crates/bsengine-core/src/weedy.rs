use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weedy {
    pub overgrowth: f32,
    pub max_overgrowth: f32,
    pub spread_rate: f32,
    pub just_overrun: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Default for Weedy {
    fn default() -> Self {
        Self {
            overgrowth: 0.0,
            max_overgrowth: 100.0,
            spread_rate: 1.0,
            just_overrun: false,
            just_cleared: false,
            enabled: true,
        }
    }
}

impl Weedy {
    pub fn spread(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_overrun = false;
        self.just_cleared = false;
        let prev = self.overgrowth;
        self.overgrowth = (self.overgrowth + amount).clamp(0.0, self.max_overgrowth);
        if self.overgrowth >= self.max_overgrowth && prev < self.max_overgrowth {
            self.just_overrun = true;
        }
    }

    pub fn clear(&mut self, amount: f32) {
        if !self.enabled || self.overgrowth <= 0.0 {
            return;
        }
        self.just_overrun = false;
        self.just_cleared = false;
        let prev = self.overgrowth;
        self.overgrowth = (self.overgrowth - amount).max(0.0);
        if self.overgrowth <= 0.0 && prev > 0.0 {
            self.just_cleared = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.overgrowth >= self.max_overgrowth {
            return;
        }
        self.spread(self.spread_rate * dt);
    }

    pub fn is_overrun(&self) -> bool {
        self.enabled && self.overgrowth >= self.max_overgrowth
    }

    pub fn is_cleared(&self) -> bool {
        self.overgrowth <= 0.0
    }

    pub fn overgrowth_fraction(&self) -> f32 {
        if self.max_overgrowth <= 0.0 {
            return 0.0;
        }
        self.overgrowth / self.max_overgrowth
    }

    pub fn effective_infestation(&self, scale: f32) -> f32 {
        self.overgrowth_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weedy() -> Weedy {
        Weedy {
            overgrowth: 0.0,
            max_overgrowth: 100.0,
            spread_rate: 10.0,
            just_overrun: false,
            just_cleared: false,
            enabled: true,
        }
    }

    #[test]
    fn default_overgrowth_zero() {
        let w = Weedy::default();
        assert_eq!(w.overgrowth, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Weedy::default().enabled);
    }

    #[test]
    fn spread_increases_overgrowth() {
        let mut w = weedy();
        w.spread(30.0);
        assert_eq!(w.overgrowth, 30.0);
    }

    #[test]
    fn spread_clamps_at_max() {
        let mut w = weedy();
        w.spread(200.0);
        assert_eq!(w.overgrowth, 100.0);
    }

    #[test]
    fn spread_no_op_when_disabled() {
        let mut w = weedy();
        w.enabled = false;
        w.spread(50.0);
        assert_eq!(w.overgrowth, 0.0);
    }

    #[test]
    fn spread_sets_just_overrun_at_max() {
        let mut w = weedy();
        w.spread(100.0);
        assert!(w.just_overrun);
    }

    #[test]
    fn spread_no_just_overrun_if_already_max() {
        let mut w = weedy();
        w.overgrowth = 100.0;
        w.spread(1.0);
        assert!(!w.just_overrun);
    }

    #[test]
    fn clear_decreases_overgrowth() {
        let mut w = weedy();
        w.overgrowth = 60.0;
        w.clear(20.0);
        assert_eq!(w.overgrowth, 40.0);
    }

    #[test]
    fn clear_clamps_at_zero() {
        let mut w = weedy();
        w.overgrowth = 30.0;
        w.clear(200.0);
        assert_eq!(w.overgrowth, 0.0);
    }

    #[test]
    fn clear_no_op_when_disabled() {
        let mut w = weedy();
        w.overgrowth = 50.0;
        w.enabled = false;
        w.clear(10.0);
        assert_eq!(w.overgrowth, 50.0);
    }

    #[test]
    fn clear_no_op_when_already_cleared() {
        let mut w = weedy();
        w.clear(10.0);
        assert_eq!(w.overgrowth, 0.0);
    }

    #[test]
    fn clear_sets_just_cleared_at_zero() {
        let mut w = weedy();
        w.overgrowth = 10.0;
        w.clear(10.0);
        assert!(w.just_cleared);
    }

    #[test]
    fn clear_no_just_cleared_if_already_zero() {
        let mut w = weedy();
        w.clear(1.0);
        assert!(!w.just_cleared);
    }

    #[test]
    fn tick_increases_overgrowth() {
        let mut w = weedy();
        w.tick(1.0);
        assert_eq!(w.overgrowth, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = weedy();
        w.tick(2.0);
        assert_eq!(w.overgrowth, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = weedy();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.overgrowth, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_overrun() {
        let mut w = weedy();
        w.overgrowth = 100.0;
        w.tick(1.0);
        assert_eq!(w.overgrowth, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = weedy();
        w.spread_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.overgrowth, 0.0);
    }

    #[test]
    fn is_overrun_true_at_max() {
        let mut w = weedy();
        w.overgrowth = 100.0;
        assert!(w.is_overrun());
    }

    #[test]
    fn is_overrun_false_below_max() {
        let mut w = weedy();
        w.overgrowth = 50.0;
        assert!(!w.is_overrun());
    }

    #[test]
    fn is_overrun_false_when_disabled() {
        let mut w = weedy();
        w.overgrowth = 100.0;
        w.enabled = false;
        assert!(!w.is_overrun());
    }

    #[test]
    fn is_cleared_true_at_zero() {
        let w = weedy();
        assert!(w.is_cleared());
    }

    #[test]
    fn is_cleared_false_above_zero() {
        let mut w = weedy();
        w.overgrowth = 1.0;
        assert!(!w.is_cleared());
    }

    #[test]
    fn overgrowth_fraction_zero_when_cleared() {
        let w = weedy();
        assert_eq!(w.overgrowth_fraction(), 0.0);
    }

    #[test]
    fn overgrowth_fraction_one_at_max() {
        let mut w = weedy();
        w.overgrowth = 100.0;
        assert_eq!(w.overgrowth_fraction(), 1.0);
    }

    #[test]
    fn overgrowth_fraction_half_at_midpoint() {
        let mut w = weedy();
        w.overgrowth = 50.0;
        assert_eq!(w.overgrowth_fraction(), 0.5);
    }

    #[test]
    fn overgrowth_fraction_zero_when_max_zero() {
        let mut w = weedy();
        w.max_overgrowth = 0.0;
        assert_eq!(w.overgrowth_fraction(), 0.0);
    }

    #[test]
    fn effective_infestation_scales() {
        let mut w = weedy();
        w.overgrowth = 50.0;
        assert_eq!(w.effective_infestation(2.0), 1.0);
    }

    #[test]
    fn effective_infestation_zero_when_cleared() {
        let w = weedy();
        assert_eq!(w.effective_infestation(10.0), 0.0);
    }

    #[test]
    fn just_overrun_cleared_on_next_spread() {
        let mut w = weedy();
        w.spread(100.0);
        assert!(w.just_overrun);
        w.spread(1.0);
        assert!(!w.just_overrun);
    }

    #[test]
    fn just_cleared_cleared_on_next_clear() {
        let mut w = weedy();
        w.overgrowth = 10.0;
        w.clear(10.0);
        assert!(w.just_cleared);
        w.overgrowth = 10.0;
        w.clear(1.0);
        assert!(!w.just_cleared);
    }
}
