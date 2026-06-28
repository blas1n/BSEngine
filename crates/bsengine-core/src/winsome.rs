use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Winsome {
    pub charm: f32,
    pub max_charm: f32,
    pub delight_rate: f32,
    pub just_delightful: bool,
    pub just_dull: bool,
    pub enabled: bool,
}

impl Default for Winsome {
    fn default() -> Self {
        Self {
            charm: 0.0,
            max_charm: 100.0,
            delight_rate: 1.0,
            just_delightful: false,
            just_dull: false,
            enabled: true,
        }
    }
}

impl Winsome {
    pub fn delight(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_delightful = false;
        self.just_dull = false;
        let prev = self.charm;
        self.charm = (self.charm + amount).clamp(0.0, self.max_charm);
        if self.charm >= self.max_charm && prev < self.max_charm {
            self.just_delightful = true;
        }
    }

    pub fn dull(&mut self, amount: f32) {
        if !self.enabled || self.charm <= 0.0 {
            return;
        }
        self.just_delightful = false;
        self.just_dull = false;
        let prev = self.charm;
        self.charm = (self.charm - amount).max(0.0);
        if self.charm <= 0.0 && prev > 0.0 {
            self.just_dull = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.charm >= self.max_charm {
            return;
        }
        self.delight(self.delight_rate * dt);
    }

    pub fn is_delightful(&self) -> bool {
        self.enabled && self.charm >= self.max_charm
    }

    pub fn is_dull(&self) -> bool {
        self.charm <= 0.0
    }

    pub fn charm_fraction(&self) -> f32 {
        if self.max_charm <= 0.0 {
            return 0.0;
        }
        self.charm / self.max_charm
    }

    pub fn effective_appeal(&self, scale: f32) -> f32 {
        self.charm_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn winsome() -> Winsome {
        Winsome {
            charm: 0.0,
            max_charm: 100.0,
            delight_rate: 10.0,
            just_delightful: false,
            just_dull: false,
            enabled: true,
        }
    }

    #[test]
    fn default_charm_zero() {
        let w = Winsome::default();
        assert_eq!(w.charm, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Winsome::default().enabled);
    }

    #[test]
    fn delight_increases_charm() {
        let mut w = winsome();
        w.delight(30.0);
        assert_eq!(w.charm, 30.0);
    }

    #[test]
    fn delight_clamps_at_max() {
        let mut w = winsome();
        w.delight(200.0);
        assert_eq!(w.charm, 100.0);
    }

    #[test]
    fn delight_no_op_when_disabled() {
        let mut w = winsome();
        w.enabled = false;
        w.delight(50.0);
        assert_eq!(w.charm, 0.0);
    }

    #[test]
    fn delight_sets_just_delightful_at_max() {
        let mut w = winsome();
        w.delight(100.0);
        assert!(w.just_delightful);
    }

    #[test]
    fn delight_no_just_delightful_if_already_max() {
        let mut w = winsome();
        w.charm = 100.0;
        w.delight(1.0);
        assert!(!w.just_delightful);
    }

    #[test]
    fn dull_decreases_charm() {
        let mut w = winsome();
        w.charm = 60.0;
        w.dull(20.0);
        assert_eq!(w.charm, 40.0);
    }

    #[test]
    fn dull_clamps_at_zero() {
        let mut w = winsome();
        w.charm = 30.0;
        w.dull(200.0);
        assert_eq!(w.charm, 0.0);
    }

    #[test]
    fn dull_no_op_when_disabled() {
        let mut w = winsome();
        w.charm = 50.0;
        w.enabled = false;
        w.dull(10.0);
        assert_eq!(w.charm, 50.0);
    }

    #[test]
    fn dull_no_op_when_already_dull() {
        let mut w = winsome();
        w.dull(10.0);
        assert_eq!(w.charm, 0.0);
    }

    #[test]
    fn dull_sets_just_dull_at_zero() {
        let mut w = winsome();
        w.charm = 10.0;
        w.dull(10.0);
        assert!(w.just_dull);
    }

    #[test]
    fn dull_no_just_dull_if_already_zero() {
        let mut w = winsome();
        w.dull(1.0);
        assert!(!w.just_dull);
    }

    #[test]
    fn tick_increases_charm() {
        let mut w = winsome();
        w.tick(1.0);
        assert_eq!(w.charm, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = winsome();
        w.tick(2.0);
        assert_eq!(w.charm, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = winsome();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.charm, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_delightful() {
        let mut w = winsome();
        w.charm = 100.0;
        w.tick(1.0);
        assert_eq!(w.charm, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = winsome();
        w.delight_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.charm, 0.0);
    }

    #[test]
    fn is_delightful_true_at_max() {
        let mut w = winsome();
        w.charm = 100.0;
        assert!(w.is_delightful());
    }

    #[test]
    fn is_delightful_false_below_max() {
        let mut w = winsome();
        w.charm = 50.0;
        assert!(!w.is_delightful());
    }

    #[test]
    fn is_delightful_false_when_disabled() {
        let mut w = winsome();
        w.charm = 100.0;
        w.enabled = false;
        assert!(!w.is_delightful());
    }

    #[test]
    fn is_dull_true_at_zero() {
        let w = winsome();
        assert!(w.is_dull());
    }

    #[test]
    fn is_dull_false_above_zero() {
        let mut w = winsome();
        w.charm = 1.0;
        assert!(!w.is_dull());
    }

    #[test]
    fn charm_fraction_zero_when_dull() {
        let w = winsome();
        assert_eq!(w.charm_fraction(), 0.0);
    }

    #[test]
    fn charm_fraction_one_at_max() {
        let mut w = winsome();
        w.charm = 100.0;
        assert_eq!(w.charm_fraction(), 1.0);
    }

    #[test]
    fn charm_fraction_half_at_midpoint() {
        let mut w = winsome();
        w.charm = 50.0;
        assert_eq!(w.charm_fraction(), 0.5);
    }

    #[test]
    fn charm_fraction_zero_when_max_zero() {
        let mut w = winsome();
        w.max_charm = 0.0;
        assert_eq!(w.charm_fraction(), 0.0);
    }

    #[test]
    fn effective_appeal_scales() {
        let mut w = winsome();
        w.charm = 50.0;
        assert_eq!(w.effective_appeal(2.0), 1.0);
    }

    #[test]
    fn effective_appeal_zero_when_dull() {
        let w = winsome();
        assert_eq!(w.effective_appeal(10.0), 0.0);
    }

    #[test]
    fn just_delightful_cleared_on_next_delight() {
        let mut w = winsome();
        w.delight(100.0);
        assert!(w.just_delightful);
        w.delight(1.0);
        assert!(!w.just_delightful);
    }

    #[test]
    fn just_dull_cleared_on_next_dull() {
        let mut w = winsome();
        w.charm = 10.0;
        w.dull(10.0);
        assert!(w.just_dull);
        w.charm = 10.0;
        w.dull(1.0);
        assert!(!w.just_dull);
    }
}
