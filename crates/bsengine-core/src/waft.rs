use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Waft {
    pub drift: f32,
    pub max_drift: f32,
    pub carry_rate: f32,
    pub just_carried: bool,
    pub just_settled: bool,
    pub enabled: bool,
}

impl Default for Waft {
    fn default() -> Self {
        Self {
            drift: 0.0,
            max_drift: 100.0,
            carry_rate: 1.0,
            just_carried: false,
            just_settled: false,
            enabled: true,
        }
    }
}

impl Waft {
    pub fn carry(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_carried = false;
        self.just_settled = false;
        let prev = self.drift;
        self.drift = (self.drift + amount).clamp(0.0, self.max_drift);
        if self.drift >= self.max_drift && prev < self.max_drift {
            self.just_carried = true;
        }
    }

    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || self.drift <= 0.0 {
            return;
        }
        self.just_carried = false;
        self.just_settled = false;
        let prev = self.drift;
        self.drift = (self.drift - amount).max(0.0);
        if self.drift <= 0.0 && prev > 0.0 {
            self.just_settled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.drift >= self.max_drift {
            return;
        }
        self.carry(self.carry_rate * dt);
    }

    pub fn is_carried(&self) -> bool {
        self.enabled && self.drift >= self.max_drift
    }

    pub fn is_settled(&self) -> bool {
        self.drift <= 0.0
    }

    pub fn drift_fraction(&self) -> f32 {
        if self.max_drift <= 0.0 {
            return 0.0;
        }
        self.drift / self.max_drift
    }

    pub fn effective_carry(&self, scale: f32) -> f32 {
        self.drift_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waft() -> Waft {
        Waft {
            drift: 0.0,
            max_drift: 100.0,
            carry_rate: 10.0,
            just_carried: false,
            just_settled: false,
            enabled: true,
        }
    }

    #[test]
    fn default_drift_zero() {
        let w = Waft::default();
        assert_eq!(w.drift, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Waft::default().enabled);
    }

    #[test]
    fn carry_increases_drift() {
        let mut w = waft();
        w.carry(30.0);
        assert_eq!(w.drift, 30.0);
    }

    #[test]
    fn carry_clamps_at_max() {
        let mut w = waft();
        w.carry(200.0);
        assert_eq!(w.drift, 100.0);
    }

    #[test]
    fn carry_no_op_when_disabled() {
        let mut w = waft();
        w.enabled = false;
        w.carry(50.0);
        assert_eq!(w.drift, 0.0);
    }

    #[test]
    fn carry_sets_just_carried_at_max() {
        let mut w = waft();
        w.carry(100.0);
        assert!(w.just_carried);
    }

    #[test]
    fn carry_no_just_carried_if_already_max() {
        let mut w = waft();
        w.drift = 100.0;
        w.carry(1.0);
        assert!(!w.just_carried);
    }

    #[test]
    fn settle_decreases_drift() {
        let mut w = waft();
        w.drift = 60.0;
        w.settle(20.0);
        assert_eq!(w.drift, 40.0);
    }

    #[test]
    fn settle_clamps_at_zero() {
        let mut w = waft();
        w.drift = 30.0;
        w.settle(200.0);
        assert_eq!(w.drift, 0.0);
    }

    #[test]
    fn settle_no_op_when_disabled() {
        let mut w = waft();
        w.drift = 50.0;
        w.enabled = false;
        w.settle(10.0);
        assert_eq!(w.drift, 50.0);
    }

    #[test]
    fn settle_no_op_when_already_settled() {
        let mut w = waft();
        w.settle(10.0);
        assert_eq!(w.drift, 0.0);
    }

    #[test]
    fn settle_sets_just_settled_at_zero() {
        let mut w = waft();
        w.drift = 10.0;
        w.settle(10.0);
        assert!(w.just_settled);
    }

    #[test]
    fn settle_no_just_settled_if_already_zero() {
        let mut w = waft();
        w.settle(1.0);
        assert!(!w.just_settled);
    }

    #[test]
    fn tick_increases_drift() {
        let mut w = waft();
        w.tick(1.0);
        assert_eq!(w.drift, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = waft();
        w.tick(2.0);
        assert_eq!(w.drift, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = waft();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.drift, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_carried() {
        let mut w = waft();
        w.drift = 100.0;
        w.tick(1.0);
        assert_eq!(w.drift, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = waft();
        w.carry_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.drift, 0.0);
    }

    #[test]
    fn is_carried_true_at_max() {
        let mut w = waft();
        w.drift = 100.0;
        assert!(w.is_carried());
    }

    #[test]
    fn is_carried_false_below_max() {
        let mut w = waft();
        w.drift = 50.0;
        assert!(!w.is_carried());
    }

    #[test]
    fn is_carried_false_when_disabled() {
        let mut w = waft();
        w.drift = 100.0;
        w.enabled = false;
        assert!(!w.is_carried());
    }

    #[test]
    fn is_settled_true_at_zero() {
        let w = waft();
        assert!(w.is_settled());
    }

    #[test]
    fn is_settled_false_above_zero() {
        let mut w = waft();
        w.drift = 1.0;
        assert!(!w.is_settled());
    }

    #[test]
    fn drift_fraction_zero_when_settled() {
        let w = waft();
        assert_eq!(w.drift_fraction(), 0.0);
    }

    #[test]
    fn drift_fraction_one_at_max() {
        let mut w = waft();
        w.drift = 100.0;
        assert_eq!(w.drift_fraction(), 1.0);
    }

    #[test]
    fn drift_fraction_half_at_midpoint() {
        let mut w = waft();
        w.drift = 50.0;
        assert_eq!(w.drift_fraction(), 0.5);
    }

    #[test]
    fn drift_fraction_zero_when_max_zero() {
        let mut w = waft();
        w.max_drift = 0.0;
        assert_eq!(w.drift_fraction(), 0.0);
    }

    #[test]
    fn effective_carry_scales() {
        let mut w = waft();
        w.drift = 50.0;
        assert_eq!(w.effective_carry(2.0), 1.0);
    }

    #[test]
    fn effective_carry_zero_when_settled() {
        let w = waft();
        assert_eq!(w.effective_carry(10.0), 0.0);
    }

    #[test]
    fn just_carried_cleared_on_next_carry() {
        let mut w = waft();
        w.carry(100.0);
        assert!(w.just_carried);
        w.carry(1.0);
        assert!(!w.just_carried);
    }

    #[test]
    fn just_settled_cleared_on_next_settle() {
        let mut w = waft();
        w.drift = 10.0;
        w.settle(10.0);
        assert!(w.just_settled);
        w.drift = 10.0;
        w.settle(1.0);
        assert!(!w.just_settled);
    }
}
