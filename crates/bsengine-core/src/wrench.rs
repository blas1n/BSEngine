use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrench {
    pub torque: f32,
    pub max_torque: f32,
    pub twist_rate: f32,
    pub just_tightened: bool,
    pub just_loose: bool,
    pub enabled: bool,
}

impl Default for Wrench {
    fn default() -> Self {
        Self {
            torque: 0.0,
            max_torque: 100.0,
            twist_rate: 1.0,
            just_tightened: false,
            just_loose: false,
            enabled: true,
        }
    }
}

impl Wrench {
    pub fn twist(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_tightened = false;
        self.just_loose = false;
        let prev = self.torque;
        self.torque = (self.torque + amount).clamp(0.0, self.max_torque);
        if self.torque >= self.max_torque && prev < self.max_torque {
            self.just_tightened = true;
        }
    }

    pub fn loosen(&mut self, amount: f32) {
        if !self.enabled || self.torque <= 0.0 {
            return;
        }
        self.just_tightened = false;
        self.just_loose = false;
        let prev = self.torque;
        self.torque = (self.torque - amount).max(0.0);
        if self.torque <= 0.0 && prev > 0.0 {
            self.just_loose = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.torque >= self.max_torque {
            return;
        }
        self.twist(self.twist_rate * dt);
    }

    pub fn is_tightened(&self) -> bool {
        self.enabled && self.torque >= self.max_torque
    }

    pub fn is_loose(&self) -> bool {
        self.torque <= 0.0
    }

    pub fn torque_fraction(&self) -> f32 {
        if self.max_torque <= 0.0 {
            return 0.0;
        }
        self.torque / self.max_torque
    }

    pub fn effective_force(&self, scale: f32) -> f32 {
        self.torque_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrench() -> Wrench {
        Wrench {
            torque: 0.0,
            max_torque: 100.0,
            twist_rate: 10.0,
            just_tightened: false,
            just_loose: false,
            enabled: true,
        }
    }

    #[test]
    fn default_torque_zero() {
        let w = Wrench::default();
        assert_eq!(w.torque, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wrench::default().enabled);
    }

    #[test]
    fn twist_increases_torque() {
        let mut w = wrench();
        w.twist(30.0);
        assert_eq!(w.torque, 30.0);
    }

    #[test]
    fn twist_clamps_at_max() {
        let mut w = wrench();
        w.twist(200.0);
        assert_eq!(w.torque, 100.0);
    }

    #[test]
    fn twist_no_op_when_disabled() {
        let mut w = wrench();
        w.enabled = false;
        w.twist(50.0);
        assert_eq!(w.torque, 0.0);
    }

    #[test]
    fn twist_sets_just_tightened_at_max() {
        let mut w = wrench();
        w.twist(100.0);
        assert!(w.just_tightened);
    }

    #[test]
    fn twist_no_just_tightened_if_already_max() {
        let mut w = wrench();
        w.torque = 100.0;
        w.twist(1.0);
        assert!(!w.just_tightened);
    }

    #[test]
    fn loosen_decreases_torque() {
        let mut w = wrench();
        w.torque = 60.0;
        w.loosen(20.0);
        assert_eq!(w.torque, 40.0);
    }

    #[test]
    fn loosen_clamps_at_zero() {
        let mut w = wrench();
        w.torque = 30.0;
        w.loosen(200.0);
        assert_eq!(w.torque, 0.0);
    }

    #[test]
    fn loosen_no_op_when_disabled() {
        let mut w = wrench();
        w.torque = 50.0;
        w.enabled = false;
        w.loosen(10.0);
        assert_eq!(w.torque, 50.0);
    }

    #[test]
    fn loosen_no_op_when_already_loose() {
        let mut w = wrench();
        w.loosen(10.0);
        assert_eq!(w.torque, 0.0);
    }

    #[test]
    fn loosen_sets_just_loose_at_zero() {
        let mut w = wrench();
        w.torque = 10.0;
        w.loosen(10.0);
        assert!(w.just_loose);
    }

    #[test]
    fn loosen_no_just_loose_if_already_zero() {
        let mut w = wrench();
        w.loosen(1.0);
        assert!(!w.just_loose);
    }

    #[test]
    fn tick_increases_torque() {
        let mut w = wrench();
        w.tick(1.0);
        assert_eq!(w.torque, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wrench();
        w.tick(2.0);
        assert_eq!(w.torque, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wrench();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.torque, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_tightened() {
        let mut w = wrench();
        w.torque = 100.0;
        w.tick(1.0);
        assert_eq!(w.torque, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wrench();
        w.twist_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.torque, 0.0);
    }

    #[test]
    fn is_tightened_true_at_max() {
        let mut w = wrench();
        w.torque = 100.0;
        assert!(w.is_tightened());
    }

    #[test]
    fn is_tightened_false_below_max() {
        let mut w = wrench();
        w.torque = 50.0;
        assert!(!w.is_tightened());
    }

    #[test]
    fn is_tightened_false_when_disabled() {
        let mut w = wrench();
        w.torque = 100.0;
        w.enabled = false;
        assert!(!w.is_tightened());
    }

    #[test]
    fn is_loose_true_at_zero() {
        let w = wrench();
        assert!(w.is_loose());
    }

    #[test]
    fn is_loose_false_above_zero() {
        let mut w = wrench();
        w.torque = 1.0;
        assert!(!w.is_loose());
    }

    #[test]
    fn torque_fraction_zero_when_loose() {
        let w = wrench();
        assert_eq!(w.torque_fraction(), 0.0);
    }

    #[test]
    fn torque_fraction_one_at_max() {
        let mut w = wrench();
        w.torque = 100.0;
        assert_eq!(w.torque_fraction(), 1.0);
    }

    #[test]
    fn torque_fraction_half_at_midpoint() {
        let mut w = wrench();
        w.torque = 50.0;
        assert_eq!(w.torque_fraction(), 0.5);
    }

    #[test]
    fn torque_fraction_zero_when_max_zero() {
        let mut w = wrench();
        w.max_torque = 0.0;
        assert_eq!(w.torque_fraction(), 0.0);
    }

    #[test]
    fn effective_force_scales() {
        let mut w = wrench();
        w.torque = 50.0;
        assert_eq!(w.effective_force(2.0), 1.0);
    }

    #[test]
    fn effective_force_zero_when_loose() {
        let w = wrench();
        assert_eq!(w.effective_force(10.0), 0.0);
    }

    #[test]
    fn just_tightened_cleared_on_next_twist() {
        let mut w = wrench();
        w.twist(100.0);
        assert!(w.just_tightened);
        w.twist(1.0);
        assert!(!w.just_tightened);
    }

    #[test]
    fn just_loose_cleared_on_next_loosen() {
        let mut w = wrench();
        w.torque = 10.0;
        w.loosen(10.0);
        assert!(w.just_loose);
        w.torque = 10.0;
        w.loosen(1.0);
        assert!(!w.just_loose);
    }
}
