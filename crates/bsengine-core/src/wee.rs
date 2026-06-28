use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wee {
    pub glee: f32,
    pub max_glee: f32,
    pub cheer_rate: f32,
    pub just_gleeful: bool,
    pub just_subdued: bool,
    pub enabled: bool,
}

impl Default for Wee {
    fn default() -> Self {
        Self {
            glee: 0.0,
            max_glee: 100.0,
            cheer_rate: 1.0,
            just_gleeful: false,
            just_subdued: false,
            enabled: true,
        }
    }
}

impl Wee {
    pub fn cheer(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_gleeful = false;
        self.just_subdued = false;
        let prev = self.glee;
        self.glee = (self.glee + amount).clamp(0.0, self.max_glee);
        if self.glee >= self.max_glee && prev < self.max_glee {
            self.just_gleeful = true;
        }
    }

    pub fn subdue(&mut self, amount: f32) {
        if !self.enabled || self.glee <= 0.0 {
            return;
        }
        self.just_gleeful = false;
        self.just_subdued = false;
        let prev = self.glee;
        self.glee = (self.glee - amount).max(0.0);
        if self.glee <= 0.0 && prev > 0.0 {
            self.just_subdued = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.glee >= self.max_glee {
            return;
        }
        self.cheer(self.cheer_rate * dt);
    }

    pub fn is_gleeful(&self) -> bool {
        self.enabled && self.glee >= self.max_glee
    }

    pub fn is_subdued(&self) -> bool {
        self.glee <= 0.0
    }

    pub fn glee_fraction(&self) -> f32 {
        if self.max_glee <= 0.0 {
            return 0.0;
        }
        self.glee / self.max_glee
    }

    pub fn effective_joy(&self, scale: f32) -> f32 {
        self.glee_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wee() -> Wee {
        Wee {
            glee: 0.0,
            max_glee: 100.0,
            cheer_rate: 10.0,
            just_gleeful: false,
            just_subdued: false,
            enabled: true,
        }
    }

    #[test]
    fn default_glee_zero() {
        let w = Wee::default();
        assert_eq!(w.glee, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wee::default().enabled);
    }

    #[test]
    fn cheer_increases_glee() {
        let mut w = wee();
        w.cheer(30.0);
        assert_eq!(w.glee, 30.0);
    }

    #[test]
    fn cheer_clamps_at_max() {
        let mut w = wee();
        w.cheer(200.0);
        assert_eq!(w.glee, 100.0);
    }

    #[test]
    fn cheer_no_op_when_disabled() {
        let mut w = wee();
        w.enabled = false;
        w.cheer(50.0);
        assert_eq!(w.glee, 0.0);
    }

    #[test]
    fn cheer_sets_just_gleeful_at_max() {
        let mut w = wee();
        w.cheer(100.0);
        assert!(w.just_gleeful);
    }

    #[test]
    fn cheer_no_just_gleeful_if_already_max() {
        let mut w = wee();
        w.glee = 100.0;
        w.cheer(1.0);
        assert!(!w.just_gleeful);
    }

    #[test]
    fn subdue_decreases_glee() {
        let mut w = wee();
        w.glee = 60.0;
        w.subdue(20.0);
        assert_eq!(w.glee, 40.0);
    }

    #[test]
    fn subdue_clamps_at_zero() {
        let mut w = wee();
        w.glee = 30.0;
        w.subdue(200.0);
        assert_eq!(w.glee, 0.0);
    }

    #[test]
    fn subdue_no_op_when_disabled() {
        let mut w = wee();
        w.glee = 50.0;
        w.enabled = false;
        w.subdue(10.0);
        assert_eq!(w.glee, 50.0);
    }

    #[test]
    fn subdue_no_op_when_already_subdued() {
        let mut w = wee();
        w.subdue(10.0);
        assert_eq!(w.glee, 0.0);
    }

    #[test]
    fn subdue_sets_just_subdued_at_zero() {
        let mut w = wee();
        w.glee = 10.0;
        w.subdue(10.0);
        assert!(w.just_subdued);
    }

    #[test]
    fn subdue_no_just_subdued_if_already_zero() {
        let mut w = wee();
        w.subdue(1.0);
        assert!(!w.just_subdued);
    }

    #[test]
    fn tick_increases_glee() {
        let mut w = wee();
        w.tick(1.0);
        assert_eq!(w.glee, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wee();
        w.tick(2.0);
        assert_eq!(w.glee, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wee();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.glee, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_gleeful() {
        let mut w = wee();
        w.glee = 100.0;
        w.tick(1.0);
        assert_eq!(w.glee, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wee();
        w.cheer_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.glee, 0.0);
    }

    #[test]
    fn is_gleeful_true_at_max() {
        let mut w = wee();
        w.glee = 100.0;
        assert!(w.is_gleeful());
    }

    #[test]
    fn is_gleeful_false_below_max() {
        let mut w = wee();
        w.glee = 50.0;
        assert!(!w.is_gleeful());
    }

    #[test]
    fn is_gleeful_false_when_disabled() {
        let mut w = wee();
        w.glee = 100.0;
        w.enabled = false;
        assert!(!w.is_gleeful());
    }

    #[test]
    fn is_subdued_true_at_zero() {
        let w = wee();
        assert!(w.is_subdued());
    }

    #[test]
    fn is_subdued_false_above_zero() {
        let mut w = wee();
        w.glee = 1.0;
        assert!(!w.is_subdued());
    }

    #[test]
    fn glee_fraction_zero_when_subdued() {
        let w = wee();
        assert_eq!(w.glee_fraction(), 0.0);
    }

    #[test]
    fn glee_fraction_one_at_max() {
        let mut w = wee();
        w.glee = 100.0;
        assert_eq!(w.glee_fraction(), 1.0);
    }

    #[test]
    fn glee_fraction_half_at_midpoint() {
        let mut w = wee();
        w.glee = 50.0;
        assert_eq!(w.glee_fraction(), 0.5);
    }

    #[test]
    fn glee_fraction_zero_when_max_zero() {
        let mut w = wee();
        w.max_glee = 0.0;
        assert_eq!(w.glee_fraction(), 0.0);
    }

    #[test]
    fn effective_joy_scales() {
        let mut w = wee();
        w.glee = 50.0;
        assert_eq!(w.effective_joy(2.0), 1.0);
    }

    #[test]
    fn effective_joy_zero_when_subdued() {
        let w = wee();
        assert_eq!(w.effective_joy(10.0), 0.0);
    }

    #[test]
    fn just_gleeful_cleared_on_next_cheer() {
        let mut w = wee();
        w.cheer(100.0);
        assert!(w.just_gleeful);
        w.cheer(1.0);
        assert!(!w.just_gleeful);
    }

    #[test]
    fn just_subdued_cleared_on_next_subdue() {
        let mut w = wee();
        w.glee = 10.0;
        w.subdue(10.0);
        assert!(w.just_subdued);
        w.glee = 10.0;
        w.subdue(1.0);
        assert!(!w.just_subdued);
    }
}
