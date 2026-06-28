use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weird {
    pub strangeness: f32,
    pub max_strangeness: f32,
    pub warp_rate: f32,
    pub just_bizarre: bool,
    pub just_mundane: bool,
    pub enabled: bool,
}

impl Default for Weird {
    fn default() -> Self {
        Self {
            strangeness: 0.0,
            max_strangeness: 100.0,
            warp_rate: 1.0,
            just_bizarre: false,
            just_mundane: false,
            enabled: true,
        }
    }
}

impl Weird {
    pub fn warp(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_bizarre = false;
        self.just_mundane = false;
        let prev = self.strangeness;
        self.strangeness = (self.strangeness + amount).clamp(0.0, self.max_strangeness);
        if self.strangeness >= self.max_strangeness && prev < self.max_strangeness {
            self.just_bizarre = true;
        }
    }

    pub fn normalize(&mut self, amount: f32) {
        if !self.enabled || self.strangeness <= 0.0 {
            return;
        }
        self.just_bizarre = false;
        self.just_mundane = false;
        let prev = self.strangeness;
        self.strangeness = (self.strangeness - amount).max(0.0);
        if self.strangeness <= 0.0 && prev > 0.0 {
            self.just_mundane = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.strangeness >= self.max_strangeness {
            return;
        }
        self.warp(self.warp_rate * dt);
    }

    pub fn is_bizarre(&self) -> bool {
        self.enabled && self.strangeness >= self.max_strangeness
    }

    pub fn is_mundane(&self) -> bool {
        self.strangeness <= 0.0
    }

    pub fn strangeness_fraction(&self) -> f32 {
        if self.max_strangeness <= 0.0 {
            return 0.0;
        }
        self.strangeness / self.max_strangeness
    }

    pub fn effective_oddity(&self, scale: f32) -> f32 {
        self.strangeness_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weird() -> Weird {
        Weird {
            strangeness: 0.0,
            max_strangeness: 100.0,
            warp_rate: 10.0,
            just_bizarre: false,
            just_mundane: false,
            enabled: true,
        }
    }

    #[test]
    fn default_strangeness_zero() {
        let w = Weird::default();
        assert_eq!(w.strangeness, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Weird::default().enabled);
    }

    #[test]
    fn warp_increases_strangeness() {
        let mut w = weird();
        w.warp(30.0);
        assert_eq!(w.strangeness, 30.0);
    }

    #[test]
    fn warp_clamps_at_max() {
        let mut w = weird();
        w.warp(200.0);
        assert_eq!(w.strangeness, 100.0);
    }

    #[test]
    fn warp_no_op_when_disabled() {
        let mut w = weird();
        w.enabled = false;
        w.warp(50.0);
        assert_eq!(w.strangeness, 0.0);
    }

    #[test]
    fn warp_sets_just_bizarre_at_max() {
        let mut w = weird();
        w.warp(100.0);
        assert!(w.just_bizarre);
    }

    #[test]
    fn warp_no_just_bizarre_if_already_max() {
        let mut w = weird();
        w.strangeness = 100.0;
        w.warp(1.0);
        assert!(!w.just_bizarre);
    }

    #[test]
    fn normalize_decreases_strangeness() {
        let mut w = weird();
        w.strangeness = 60.0;
        w.normalize(20.0);
        assert_eq!(w.strangeness, 40.0);
    }

    #[test]
    fn normalize_clamps_at_zero() {
        let mut w = weird();
        w.strangeness = 30.0;
        w.normalize(200.0);
        assert_eq!(w.strangeness, 0.0);
    }

    #[test]
    fn normalize_no_op_when_disabled() {
        let mut w = weird();
        w.strangeness = 50.0;
        w.enabled = false;
        w.normalize(10.0);
        assert_eq!(w.strangeness, 50.0);
    }

    #[test]
    fn normalize_no_op_when_already_mundane() {
        let mut w = weird();
        w.normalize(10.0);
        assert_eq!(w.strangeness, 0.0);
    }

    #[test]
    fn normalize_sets_just_mundane_at_zero() {
        let mut w = weird();
        w.strangeness = 10.0;
        w.normalize(10.0);
        assert!(w.just_mundane);
    }

    #[test]
    fn normalize_no_just_mundane_if_already_zero() {
        let mut w = weird();
        w.normalize(1.0);
        assert!(!w.just_mundane);
    }

    #[test]
    fn tick_increases_strangeness() {
        let mut w = weird();
        w.tick(1.0);
        assert_eq!(w.strangeness, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = weird();
        w.tick(2.0);
        assert_eq!(w.strangeness, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = weird();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.strangeness, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_bizarre() {
        let mut w = weird();
        w.strangeness = 100.0;
        w.tick(1.0);
        assert_eq!(w.strangeness, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = weird();
        w.warp_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.strangeness, 0.0);
    }

    #[test]
    fn is_bizarre_true_at_max() {
        let mut w = weird();
        w.strangeness = 100.0;
        assert!(w.is_bizarre());
    }

    #[test]
    fn is_bizarre_false_below_max() {
        let mut w = weird();
        w.strangeness = 50.0;
        assert!(!w.is_bizarre());
    }

    #[test]
    fn is_bizarre_false_when_disabled() {
        let mut w = weird();
        w.strangeness = 100.0;
        w.enabled = false;
        assert!(!w.is_bizarre());
    }

    #[test]
    fn is_mundane_true_at_zero() {
        let w = weird();
        assert!(w.is_mundane());
    }

    #[test]
    fn is_mundane_false_above_zero() {
        let mut w = weird();
        w.strangeness = 1.0;
        assert!(!w.is_mundane());
    }

    #[test]
    fn strangeness_fraction_zero_when_mundane() {
        let w = weird();
        assert_eq!(w.strangeness_fraction(), 0.0);
    }

    #[test]
    fn strangeness_fraction_one_at_max() {
        let mut w = weird();
        w.strangeness = 100.0;
        assert_eq!(w.strangeness_fraction(), 1.0);
    }

    #[test]
    fn strangeness_fraction_half_at_midpoint() {
        let mut w = weird();
        w.strangeness = 50.0;
        assert_eq!(w.strangeness_fraction(), 0.5);
    }

    #[test]
    fn strangeness_fraction_zero_when_max_zero() {
        let mut w = weird();
        w.max_strangeness = 0.0;
        assert_eq!(w.strangeness_fraction(), 0.0);
    }

    #[test]
    fn effective_oddity_scales() {
        let mut w = weird();
        w.strangeness = 50.0;
        assert_eq!(w.effective_oddity(2.0), 1.0);
    }

    #[test]
    fn effective_oddity_zero_when_mundane() {
        let w = weird();
        assert_eq!(w.effective_oddity(10.0), 0.0);
    }

    #[test]
    fn just_bizarre_cleared_on_next_warp() {
        let mut w = weird();
        w.warp(100.0);
        assert!(w.just_bizarre);
        w.warp(1.0);
        assert!(!w.just_bizarre);
    }

    #[test]
    fn just_mundane_cleared_on_next_normalize() {
        let mut w = weird();
        w.strangeness = 10.0;
        w.normalize(10.0);
        assert!(w.just_mundane);
        w.strangeness = 10.0;
        w.normalize(1.0);
        assert!(!w.just_mundane);
    }
}
