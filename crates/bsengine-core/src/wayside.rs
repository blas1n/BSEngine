use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wayside {
    pub detour: f32,
    pub max_detour: f32,
    pub drift_rate: f32,
    pub just_lost: bool,
    pub just_found: bool,
    pub enabled: bool,
}

impl Default for Wayside {
    fn default() -> Self {
        Self {
            detour: 0.0,
            max_detour: 100.0,
            drift_rate: 1.0,
            just_lost: false,
            just_found: false,
            enabled: true,
        }
    }
}

impl Wayside {
    pub fn drift(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_lost = false;
        self.just_found = false;
        let prev = self.detour;
        self.detour = (self.detour + amount).clamp(0.0, self.max_detour);
        if self.detour >= self.max_detour && prev < self.max_detour {
            self.just_lost = true;
        }
    }

    pub fn recover(&mut self, amount: f32) {
        if !self.enabled || self.detour <= 0.0 {
            return;
        }
        self.just_lost = false;
        self.just_found = false;
        let prev = self.detour;
        self.detour = (self.detour - amount).max(0.0);
        if self.detour <= 0.0 && prev > 0.0 {
            self.just_found = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.detour >= self.max_detour {
            return;
        }
        self.drift(self.drift_rate * dt);
    }

    pub fn is_lost(&self) -> bool {
        self.enabled && self.detour >= self.max_detour
    }
    pub fn is_found(&self) -> bool {
        self.detour <= 0.0
    }

    pub fn detour_fraction(&self) -> f32 {
        if self.max_detour <= 0.0 {
            return 0.0;
        }
        self.detour / self.max_detour
    }

    pub fn effective_stray(&self, scale: f32) -> f32 {
        self.detour_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wayside() -> Wayside {
        Wayside {
            detour: 0.0,
            max_detour: 100.0,
            drift_rate: 10.0,
            just_lost: false,
            just_found: false,
            enabled: true,
        }
    }

    #[test]
    fn default_detour_zero() {
        assert_eq!(Wayside::default().detour, 0.0);
    }
    #[test]
    fn default_enabled() {
        assert!(Wayside::default().enabled);
    }
    #[test]
    fn drift_increases_detour() {
        let mut w = wayside();
        w.drift(30.0);
        assert_eq!(w.detour, 30.0);
    }
    #[test]
    fn drift_clamps_at_max() {
        let mut w = wayside();
        w.drift(200.0);
        assert_eq!(w.detour, 100.0);
    }
    #[test]
    fn drift_no_op_when_disabled() {
        let mut w = wayside();
        w.enabled = false;
        w.drift(50.0);
        assert_eq!(w.detour, 0.0);
    }
    #[test]
    fn drift_sets_just_lost_at_max() {
        let mut w = wayside();
        w.drift(100.0);
        assert!(w.just_lost);
    }
    #[test]
    fn drift_no_just_lost_if_already_max() {
        let mut w = wayside();
        w.detour = 100.0;
        w.drift(1.0);
        assert!(!w.just_lost);
    }
    #[test]
    fn recover_decreases_detour() {
        let mut w = wayside();
        w.detour = 60.0;
        w.recover(20.0);
        assert_eq!(w.detour, 40.0);
    }
    #[test]
    fn recover_clamps_at_zero() {
        let mut w = wayside();
        w.detour = 30.0;
        w.recover(200.0);
        assert_eq!(w.detour, 0.0);
    }
    #[test]
    fn recover_no_op_when_disabled() {
        let mut w = wayside();
        w.detour = 50.0;
        w.enabled = false;
        w.recover(10.0);
        assert_eq!(w.detour, 50.0);
    }
    #[test]
    fn recover_no_op_when_already_found() {
        let mut w = wayside();
        w.recover(10.0);
        assert_eq!(w.detour, 0.0);
    }
    #[test]
    fn recover_sets_just_found_at_zero() {
        let mut w = wayside();
        w.detour = 10.0;
        w.recover(10.0);
        assert!(w.just_found);
    }
    #[test]
    fn recover_no_just_found_if_already_zero() {
        let mut w = wayside();
        w.recover(1.0);
        assert!(!w.just_found);
    }
    #[test]
    fn tick_increases_detour() {
        let mut w = wayside();
        w.tick(1.0);
        assert_eq!(w.detour, 10.0);
    }
    #[test]
    fn tick_scales_with_dt() {
        let mut w = wayside();
        w.tick(2.0);
        assert_eq!(w.detour, 20.0);
    }
    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wayside();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.detour, 0.0);
    }
    #[test]
    fn tick_no_op_when_already_lost() {
        let mut w = wayside();
        w.detour = 100.0;
        w.tick(1.0);
        assert_eq!(w.detour, 100.0);
    }
    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wayside();
        w.drift_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.detour, 0.0);
    }
    #[test]
    fn is_lost_true_at_max() {
        let mut w = wayside();
        w.detour = 100.0;
        assert!(w.is_lost());
    }
    #[test]
    fn is_lost_false_below_max() {
        let mut w = wayside();
        w.detour = 50.0;
        assert!(!w.is_lost());
    }
    #[test]
    fn is_lost_false_when_disabled() {
        let mut w = wayside();
        w.detour = 100.0;
        w.enabled = false;
        assert!(!w.is_lost());
    }
    #[test]
    fn is_found_true_at_zero() {
        let w = wayside();
        assert!(w.is_found());
    }
    #[test]
    fn is_found_false_above_zero() {
        let mut w = wayside();
        w.detour = 1.0;
        assert!(!w.is_found());
    }
    #[test]
    fn detour_fraction_zero_when_found() {
        let w = wayside();
        assert_eq!(w.detour_fraction(), 0.0);
    }
    #[test]
    fn detour_fraction_one_at_max() {
        let mut w = wayside();
        w.detour = 100.0;
        assert_eq!(w.detour_fraction(), 1.0);
    }
    #[test]
    fn detour_fraction_half_at_midpoint() {
        let mut w = wayside();
        w.detour = 50.0;
        assert_eq!(w.detour_fraction(), 0.5);
    }
    #[test]
    fn detour_fraction_zero_when_max_zero() {
        let mut w = wayside();
        w.max_detour = 0.0;
        assert_eq!(w.detour_fraction(), 0.0);
    }
    #[test]
    fn effective_stray_scales() {
        let mut w = wayside();
        w.detour = 50.0;
        assert_eq!(w.effective_stray(2.0), 1.0);
    }
    #[test]
    fn effective_stray_zero_when_found() {
        let w = wayside();
        assert_eq!(w.effective_stray(10.0), 0.0);
    }
    #[test]
    fn just_lost_cleared_on_next_drift() {
        let mut w = wayside();
        w.drift(100.0);
        assert!(w.just_lost);
        w.drift(1.0);
        assert!(!w.just_lost);
    }
    #[test]
    fn just_found_cleared_on_next_recover() {
        let mut w = wayside();
        w.detour = 10.0;
        w.recover(10.0);
        assert!(w.just_found);
        w.detour = 10.0;
        w.recover(1.0);
        assert!(!w.just_found);
    }
}
