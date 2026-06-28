use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wicker {
    pub weave: f32,
    pub max_weave: f32,
    pub plait_rate: f32,
    pub just_plaited: bool,
    pub just_unraveled: bool,
    pub enabled: bool,
}

impl Default for Wicker {
    fn default() -> Self {
        Self {
            weave: 0.0,
            max_weave: 100.0,
            plait_rate: 1.0,
            just_plaited: false,
            just_unraveled: false,
            enabled: true,
        }
    }
}

impl Wicker {
    pub fn plait(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_plaited = false;
        self.just_unraveled = false;
        let prev = self.weave;
        self.weave = (self.weave + amount).clamp(0.0, self.max_weave);
        if self.weave >= self.max_weave && prev < self.max_weave {
            self.just_plaited = true;
        }
    }

    pub fn unravel(&mut self, amount: f32) {
        if !self.enabled || self.weave <= 0.0 {
            return;
        }
        self.just_plaited = false;
        self.just_unraveled = false;
        let prev = self.weave;
        self.weave = (self.weave - amount).max(0.0);
        if self.weave <= 0.0 && prev > 0.0 {
            self.just_unraveled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.weave >= self.max_weave {
            return;
        }
        self.plait(self.plait_rate * dt);
    }

    pub fn is_plaited(&self) -> bool {
        self.enabled && self.weave >= self.max_weave
    }

    pub fn is_unraveled(&self) -> bool {
        self.weave <= 0.0
    }

    pub fn weave_fraction(&self) -> f32 {
        if self.max_weave <= 0.0 {
            return 0.0;
        }
        self.weave / self.max_weave
    }

    pub fn effective_basket(&self, scale: f32) -> f32 {
        self.weave_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wicker() -> Wicker {
        Wicker {
            weave: 0.0,
            max_weave: 100.0,
            plait_rate: 10.0,
            just_plaited: false,
            just_unraveled: false,
            enabled: true,
        }
    }

    #[test]
    fn default_weave_zero() {
        let w = Wicker::default();
        assert_eq!(w.weave, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wicker::default().enabled);
    }

    #[test]
    fn plait_increases_weave() {
        let mut w = wicker();
        w.plait(30.0);
        assert_eq!(w.weave, 30.0);
    }

    #[test]
    fn plait_clamps_at_max() {
        let mut w = wicker();
        w.plait(200.0);
        assert_eq!(w.weave, 100.0);
    }

    #[test]
    fn plait_no_op_when_disabled() {
        let mut w = wicker();
        w.enabled = false;
        w.plait(50.0);
        assert_eq!(w.weave, 0.0);
    }

    #[test]
    fn plait_sets_just_plaited_at_max() {
        let mut w = wicker();
        w.plait(100.0);
        assert!(w.just_plaited);
    }

    #[test]
    fn plait_no_just_plaited_if_already_max() {
        let mut w = wicker();
        w.weave = 100.0;
        w.plait(1.0);
        assert!(!w.just_plaited);
    }

    #[test]
    fn unravel_decreases_weave() {
        let mut w = wicker();
        w.weave = 60.0;
        w.unravel(20.0);
        assert_eq!(w.weave, 40.0);
    }

    #[test]
    fn unravel_clamps_at_zero() {
        let mut w = wicker();
        w.weave = 30.0;
        w.unravel(200.0);
        assert_eq!(w.weave, 0.0);
    }

    #[test]
    fn unravel_no_op_when_disabled() {
        let mut w = wicker();
        w.weave = 50.0;
        w.enabled = false;
        w.unravel(10.0);
        assert_eq!(w.weave, 50.0);
    }

    #[test]
    fn unravel_no_op_when_already_unraveled() {
        let mut w = wicker();
        w.unravel(10.0);
        assert_eq!(w.weave, 0.0);
    }

    #[test]
    fn unravel_sets_just_unraveled_at_zero() {
        let mut w = wicker();
        w.weave = 10.0;
        w.unravel(10.0);
        assert!(w.just_unraveled);
    }

    #[test]
    fn unravel_no_just_unraveled_if_already_zero() {
        let mut w = wicker();
        w.unravel(1.0);
        assert!(!w.just_unraveled);
    }

    #[test]
    fn tick_increases_weave() {
        let mut w = wicker();
        w.tick(1.0);
        assert_eq!(w.weave, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wicker();
        w.tick(2.0);
        assert_eq!(w.weave, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wicker();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.weave, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_plaited() {
        let mut w = wicker();
        w.weave = 100.0;
        w.tick(1.0);
        assert_eq!(w.weave, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wicker();
        w.plait_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.weave, 0.0);
    }

    #[test]
    fn is_plaited_true_at_max() {
        let mut w = wicker();
        w.weave = 100.0;
        assert!(w.is_plaited());
    }

    #[test]
    fn is_plaited_false_below_max() {
        let mut w = wicker();
        w.weave = 50.0;
        assert!(!w.is_plaited());
    }

    #[test]
    fn is_plaited_false_when_disabled() {
        let mut w = wicker();
        w.weave = 100.0;
        w.enabled = false;
        assert!(!w.is_plaited());
    }

    #[test]
    fn is_unraveled_true_at_zero() {
        let w = wicker();
        assert!(w.is_unraveled());
    }

    #[test]
    fn is_unraveled_false_above_zero() {
        let mut w = wicker();
        w.weave = 1.0;
        assert!(!w.is_unraveled());
    }

    #[test]
    fn weave_fraction_zero_when_unraveled() {
        let w = wicker();
        assert_eq!(w.weave_fraction(), 0.0);
    }

    #[test]
    fn weave_fraction_one_at_max() {
        let mut w = wicker();
        w.weave = 100.0;
        assert_eq!(w.weave_fraction(), 1.0);
    }

    #[test]
    fn weave_fraction_half_at_midpoint() {
        let mut w = wicker();
        w.weave = 50.0;
        assert_eq!(w.weave_fraction(), 0.5);
    }

    #[test]
    fn weave_fraction_zero_when_max_zero() {
        let mut w = wicker();
        w.max_weave = 0.0;
        assert_eq!(w.weave_fraction(), 0.0);
    }

    #[test]
    fn effective_basket_scales() {
        let mut w = wicker();
        w.weave = 50.0;
        assert_eq!(w.effective_basket(2.0), 1.0);
    }

    #[test]
    fn effective_basket_zero_when_unraveled() {
        let w = wicker();
        assert_eq!(w.effective_basket(10.0), 0.0);
    }

    #[test]
    fn just_plaited_cleared_on_next_plait() {
        let mut w = wicker();
        w.plait(100.0);
        assert!(w.just_plaited);
        w.plait(1.0);
        assert!(!w.just_plaited);
    }

    #[test]
    fn just_unraveled_cleared_on_next_unravel() {
        let mut w = wicker();
        w.weave = 10.0;
        w.unravel(10.0);
        assert!(w.just_unraveled);
        w.weave = 10.0;
        w.unravel(1.0);
        assert!(!w.just_unraveled);
    }
}
