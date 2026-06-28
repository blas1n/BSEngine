use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wombat {
    pub burrow: f32,
    pub max_burrow: f32,
    pub dig_rate: f32,
    pub just_tunneled: bool,
    pub just_surfaced: bool,
    pub enabled: bool,
}

impl Default for Wombat {
    fn default() -> Self {
        Self {
            burrow: 0.0,
            max_burrow: 100.0,
            dig_rate: 1.0,
            just_tunneled: false,
            just_surfaced: false,
            enabled: true,
        }
    }
}

impl Wombat {
    pub fn dig(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_tunneled = false;
        self.just_surfaced = false;
        let prev = self.burrow;
        self.burrow = (self.burrow + amount).clamp(0.0, self.max_burrow);
        if self.burrow >= self.max_burrow && prev < self.max_burrow {
            self.just_tunneled = true;
        }
    }

    pub fn surface(&mut self, amount: f32) {
        if !self.enabled || self.burrow <= 0.0 {
            return;
        }
        self.just_tunneled = false;
        self.just_surfaced = false;
        let prev = self.burrow;
        self.burrow = (self.burrow - amount).max(0.0);
        if self.burrow <= 0.0 && prev > 0.0 {
            self.just_surfaced = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.burrow >= self.max_burrow {
            return;
        }
        self.dig(self.dig_rate * dt);
    }

    pub fn is_tunneled(&self) -> bool {
        self.enabled && self.burrow >= self.max_burrow
    }

    pub fn is_surfaced(&self) -> bool {
        self.burrow <= 0.0
    }

    pub fn burrow_fraction(&self) -> f32 {
        if self.max_burrow <= 0.0 {
            return 0.0;
        }
        self.burrow / self.max_burrow
    }

    pub fn effective_dig_depth(&self, scale: f32) -> f32 {
        self.burrow_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wombat() -> Wombat {
        Wombat {
            burrow: 0.0,
            max_burrow: 100.0,
            dig_rate: 10.0,
            just_tunneled: false,
            just_surfaced: false,
            enabled: true,
        }
    }

    #[test]
    fn default_burrow_zero() {
        let w = Wombat::default();
        assert_eq!(w.burrow, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wombat::default().enabled);
    }

    #[test]
    fn dig_increases_burrow() {
        let mut w = wombat();
        w.dig(30.0);
        assert_eq!(w.burrow, 30.0);
    }

    #[test]
    fn dig_clamps_at_max() {
        let mut w = wombat();
        w.dig(200.0);
        assert_eq!(w.burrow, 100.0);
    }

    #[test]
    fn dig_no_op_when_disabled() {
        let mut w = wombat();
        w.enabled = false;
        w.dig(50.0);
        assert_eq!(w.burrow, 0.0);
    }

    #[test]
    fn dig_sets_just_tunneled_at_max() {
        let mut w = wombat();
        w.dig(100.0);
        assert!(w.just_tunneled);
    }

    #[test]
    fn dig_no_just_tunneled_if_already_max() {
        let mut w = wombat();
        w.burrow = 100.0;
        w.dig(1.0);
        assert!(!w.just_tunneled);
    }

    #[test]
    fn surface_decreases_burrow() {
        let mut w = wombat();
        w.burrow = 60.0;
        w.surface(20.0);
        assert_eq!(w.burrow, 40.0);
    }

    #[test]
    fn surface_clamps_at_zero() {
        let mut w = wombat();
        w.burrow = 30.0;
        w.surface(200.0);
        assert_eq!(w.burrow, 0.0);
    }

    #[test]
    fn surface_no_op_when_disabled() {
        let mut w = wombat();
        w.burrow = 50.0;
        w.enabled = false;
        w.surface(10.0);
        assert_eq!(w.burrow, 50.0);
    }

    #[test]
    fn surface_no_op_when_already_surfaced() {
        let mut w = wombat();
        w.surface(10.0);
        assert_eq!(w.burrow, 0.0);
    }

    #[test]
    fn surface_sets_just_surfaced_at_zero() {
        let mut w = wombat();
        w.burrow = 10.0;
        w.surface(10.0);
        assert!(w.just_surfaced);
    }

    #[test]
    fn surface_no_just_surfaced_if_already_zero() {
        let mut w = wombat();
        w.surface(1.0);
        assert!(!w.just_surfaced);
    }

    #[test]
    fn tick_increases_burrow() {
        let mut w = wombat();
        w.tick(1.0);
        assert_eq!(w.burrow, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wombat();
        w.tick(2.0);
        assert_eq!(w.burrow, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wombat();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.burrow, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_tunneled() {
        let mut w = wombat();
        w.burrow = 100.0;
        w.tick(1.0);
        assert_eq!(w.burrow, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wombat();
        w.dig_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.burrow, 0.0);
    }

    #[test]
    fn is_tunneled_true_at_max() {
        let mut w = wombat();
        w.burrow = 100.0;
        assert!(w.is_tunneled());
    }

    #[test]
    fn is_tunneled_false_below_max() {
        let mut w = wombat();
        w.burrow = 50.0;
        assert!(!w.is_tunneled());
    }

    #[test]
    fn is_tunneled_false_when_disabled() {
        let mut w = wombat();
        w.burrow = 100.0;
        w.enabled = false;
        assert!(!w.is_tunneled());
    }

    #[test]
    fn is_surfaced_true_at_zero() {
        let w = wombat();
        assert!(w.is_surfaced());
    }

    #[test]
    fn is_surfaced_false_above_zero() {
        let mut w = wombat();
        w.burrow = 1.0;
        assert!(!w.is_surfaced());
    }

    #[test]
    fn burrow_fraction_zero_when_surfaced() {
        let w = wombat();
        assert_eq!(w.burrow_fraction(), 0.0);
    }

    #[test]
    fn burrow_fraction_one_at_max() {
        let mut w = wombat();
        w.burrow = 100.0;
        assert_eq!(w.burrow_fraction(), 1.0);
    }

    #[test]
    fn burrow_fraction_half_at_midpoint() {
        let mut w = wombat();
        w.burrow = 50.0;
        assert_eq!(w.burrow_fraction(), 0.5);
    }

    #[test]
    fn burrow_fraction_zero_when_max_zero() {
        let mut w = wombat();
        w.max_burrow = 0.0;
        assert_eq!(w.burrow_fraction(), 0.0);
    }

    #[test]
    fn effective_dig_depth_scales() {
        let mut w = wombat();
        w.burrow = 50.0;
        assert_eq!(w.effective_dig_depth(2.0), 1.0);
    }

    #[test]
    fn effective_dig_depth_zero_when_surfaced() {
        let w = wombat();
        assert_eq!(w.effective_dig_depth(10.0), 0.0);
    }

    #[test]
    fn just_tunneled_cleared_on_next_dig() {
        let mut w = wombat();
        w.dig(100.0);
        assert!(w.just_tunneled);
        w.dig(1.0);
        assert!(!w.just_tunneled);
    }

    #[test]
    fn just_surfaced_cleared_on_next_surface() {
        let mut w = wombat();
        w.burrow = 10.0;
        w.surface(10.0);
        assert!(w.just_surfaced);
        w.burrow = 10.0;
        w.surface(1.0);
        assert!(!w.just_surfaced);
    }
}
