use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrist {
    pub flex: f32,
    pub max_flex: f32,
    pub rotate_rate: f32,
    pub just_flexible: bool,
    pub just_locked: bool,
    pub enabled: bool,
}

impl Default for Wrist {
    fn default() -> Self {
        Self {
            flex: 0.0,
            max_flex: 100.0,
            rotate_rate: 1.0,
            just_flexible: false,
            just_locked: false,
            enabled: true,
        }
    }
}

impl Wrist {
    pub fn rotate(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_flexible = false;
        self.just_locked = false;
        let prev = self.flex;
        self.flex = (self.flex + amount).clamp(0.0, self.max_flex);
        if self.flex >= self.max_flex && prev < self.max_flex {
            self.just_flexible = true;
        }
    }

    pub fn stiffen(&mut self, amount: f32) {
        if !self.enabled || self.flex <= 0.0 {
            return;
        }
        self.just_flexible = false;
        self.just_locked = false;
        let prev = self.flex;
        self.flex = (self.flex - amount).max(0.0);
        if self.flex <= 0.0 && prev > 0.0 {
            self.just_locked = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.flex >= self.max_flex {
            return;
        }
        self.rotate(self.rotate_rate * dt);
    }

    pub fn is_flexible(&self) -> bool {
        self.enabled && self.flex >= self.max_flex
    }

    pub fn is_locked(&self) -> bool {
        self.flex <= 0.0
    }

    pub fn flex_fraction(&self) -> f32 {
        if self.max_flex <= 0.0 {
            return 0.0;
        }
        self.flex / self.max_flex
    }

    pub fn effective_mobility(&self, scale: f32) -> f32 {
        self.flex_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrist() -> Wrist {
        Wrist {
            flex: 0.0,
            max_flex: 100.0,
            rotate_rate: 10.0,
            just_flexible: false,
            just_locked: false,
            enabled: true,
        }
    }

    #[test]
    fn default_flex_zero() {
        let w = Wrist::default();
        assert_eq!(w.flex, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wrist::default().enabled);
    }

    #[test]
    fn rotate_increases_flex() {
        let mut w = wrist();
        w.rotate(30.0);
        assert_eq!(w.flex, 30.0);
    }

    #[test]
    fn rotate_clamps_at_max() {
        let mut w = wrist();
        w.rotate(200.0);
        assert_eq!(w.flex, 100.0);
    }

    #[test]
    fn rotate_no_op_when_disabled() {
        let mut w = wrist();
        w.enabled = false;
        w.rotate(50.0);
        assert_eq!(w.flex, 0.0);
    }

    #[test]
    fn rotate_sets_just_flexible_at_max() {
        let mut w = wrist();
        w.rotate(100.0);
        assert!(w.just_flexible);
    }

    #[test]
    fn rotate_no_just_flexible_if_already_max() {
        let mut w = wrist();
        w.flex = 100.0;
        w.rotate(1.0);
        assert!(!w.just_flexible);
    }

    #[test]
    fn stiffen_decreases_flex() {
        let mut w = wrist();
        w.flex = 60.0;
        w.stiffen(20.0);
        assert_eq!(w.flex, 40.0);
    }

    #[test]
    fn stiffen_clamps_at_zero() {
        let mut w = wrist();
        w.flex = 30.0;
        w.stiffen(200.0);
        assert_eq!(w.flex, 0.0);
    }

    #[test]
    fn stiffen_no_op_when_disabled() {
        let mut w = wrist();
        w.flex = 50.0;
        w.enabled = false;
        w.stiffen(10.0);
        assert_eq!(w.flex, 50.0);
    }

    #[test]
    fn stiffen_no_op_when_already_locked() {
        let mut w = wrist();
        w.stiffen(10.0);
        assert_eq!(w.flex, 0.0);
    }

    #[test]
    fn stiffen_sets_just_locked_at_zero() {
        let mut w = wrist();
        w.flex = 10.0;
        w.stiffen(10.0);
        assert!(w.just_locked);
    }

    #[test]
    fn stiffen_no_just_locked_if_already_zero() {
        let mut w = wrist();
        w.stiffen(1.0);
        assert!(!w.just_locked);
    }

    #[test]
    fn tick_increases_flex() {
        let mut w = wrist();
        w.tick(1.0);
        assert_eq!(w.flex, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wrist();
        w.tick(2.0);
        assert_eq!(w.flex, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wrist();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.flex, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_flexible() {
        let mut w = wrist();
        w.flex = 100.0;
        w.tick(1.0);
        assert_eq!(w.flex, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wrist();
        w.rotate_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.flex, 0.0);
    }

    #[test]
    fn is_flexible_true_at_max() {
        let mut w = wrist();
        w.flex = 100.0;
        assert!(w.is_flexible());
    }

    #[test]
    fn is_flexible_false_below_max() {
        let mut w = wrist();
        w.flex = 50.0;
        assert!(!w.is_flexible());
    }

    #[test]
    fn is_flexible_false_when_disabled() {
        let mut w = wrist();
        w.flex = 100.0;
        w.enabled = false;
        assert!(!w.is_flexible());
    }

    #[test]
    fn is_locked_true_at_zero() {
        let w = wrist();
        assert!(w.is_locked());
    }

    #[test]
    fn is_locked_false_above_zero() {
        let mut w = wrist();
        w.flex = 1.0;
        assert!(!w.is_locked());
    }

    #[test]
    fn flex_fraction_zero_when_locked() {
        let w = wrist();
        assert_eq!(w.flex_fraction(), 0.0);
    }

    #[test]
    fn flex_fraction_one_at_max() {
        let mut w = wrist();
        w.flex = 100.0;
        assert_eq!(w.flex_fraction(), 1.0);
    }

    #[test]
    fn flex_fraction_half_at_midpoint() {
        let mut w = wrist();
        w.flex = 50.0;
        assert_eq!(w.flex_fraction(), 0.5);
    }

    #[test]
    fn flex_fraction_zero_when_max_zero() {
        let mut w = wrist();
        w.max_flex = 0.0;
        assert_eq!(w.flex_fraction(), 0.0);
    }

    #[test]
    fn effective_mobility_scales() {
        let mut w = wrist();
        w.flex = 50.0;
        assert_eq!(w.effective_mobility(2.0), 1.0);
    }

    #[test]
    fn effective_mobility_zero_when_locked() {
        let w = wrist();
        assert_eq!(w.effective_mobility(10.0), 0.0);
    }

    #[test]
    fn just_flexible_cleared_on_next_rotate() {
        let mut w = wrist();
        w.rotate(100.0);
        assert!(w.just_flexible);
        w.rotate(1.0);
        assert!(!w.just_flexible);
    }

    #[test]
    fn just_locked_cleared_on_next_stiffen() {
        let mut w = wrist();
        w.flex = 10.0;
        w.stiffen(10.0);
        assert!(w.just_locked);
        w.flex = 10.0;
        w.stiffen(1.0);
        assert!(!w.just_locked);
    }
}
