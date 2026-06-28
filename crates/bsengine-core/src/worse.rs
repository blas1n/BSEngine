use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Worse {
    pub deterioration: f32,
    pub max_deterioration: f32,
    pub decline_rate: f32,
    pub just_ruined: bool,
    pub just_restored: bool,
    pub enabled: bool,
}

impl Default for Worse {
    fn default() -> Self {
        Self {
            deterioration: 0.0,
            max_deterioration: 100.0,
            decline_rate: 1.0,
            just_ruined: false,
            just_restored: false,
            enabled: true,
        }
    }
}

impl Worse {
    pub fn decline(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_ruined = false;
        self.just_restored = false;
        let prev = self.deterioration;
        self.deterioration = (self.deterioration + amount).clamp(0.0, self.max_deterioration);
        if self.deterioration >= self.max_deterioration && prev < self.max_deterioration {
            self.just_ruined = true;
        }
    }

    pub fn restore(&mut self, amount: f32) {
        if !self.enabled || self.deterioration <= 0.0 {
            return;
        }
        self.just_ruined = false;
        self.just_restored = false;
        let prev = self.deterioration;
        self.deterioration = (self.deterioration - amount).max(0.0);
        if self.deterioration <= 0.0 && prev > 0.0 {
            self.just_restored = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.deterioration >= self.max_deterioration {
            return;
        }
        self.decline(self.decline_rate * dt);
    }

    pub fn is_ruined(&self) -> bool {
        self.enabled && self.deterioration >= self.max_deterioration
    }
    pub fn is_restored(&self) -> bool {
        self.deterioration <= 0.0
    }

    pub fn deterioration_fraction(&self) -> f32 {
        if self.max_deterioration <= 0.0 {
            return 0.0;
        }
        self.deterioration / self.max_deterioration
    }

    pub fn effective_decay(&self, scale: f32) -> f32 {
        self.deterioration_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inst() -> Worse {
        Worse {
            deterioration: 0.0,
            max_deterioration: 100.0,
            decline_rate: 10.0,
            just_ruined: false,
            just_restored: false,
            enabled: true,
        }
    }
    #[test]
    fn default_deterioration_zero() {
        assert_eq!(Worse::default().deterioration, 0.0);
    }
    #[test]
    fn default_enabled() {
        assert!(Worse::default().enabled);
    }
    #[test]
    fn ADD_increases() {
        let mut w = inst();
        w.decline(30.0);
        assert_eq!(w.deterioration, 30.0);
    }
    #[test]
    fn ADD_clamps_at_max() {
        let mut w = inst();
        w.decline(200.0);
        assert_eq!(w.deterioration, 100.0);
    }
    #[test]
    fn ADD_no_op_when_disabled() {
        let mut w = inst();
        w.enabled = false;
        w.decline(50.0);
        assert_eq!(w.deterioration, 0.0);
    }
    #[test]
    fn ADD_sets_just_ruined_at_max() {
        let mut w = inst();
        w.decline(100.0);
        assert!(w.just_ruined);
    }
    #[test]
    fn ADD_no_just_ruined_if_already_max() {
        let mut w = inst();
        w.deterioration = 100.0;
        w.decline(1.0);
        assert!(!w.just_ruined);
    }
    #[test]
    fn REMOVE_decreases() {
        let mut w = inst();
        w.deterioration = 60.0;
        w.restore(20.0);
        assert_eq!(w.deterioration, 40.0);
    }
    #[test]
    fn REMOVE_clamps_at_zero() {
        let mut w = inst();
        w.deterioration = 30.0;
        w.restore(200.0);
        assert_eq!(w.deterioration, 0.0);
    }
    #[test]
    fn REMOVE_no_op_when_disabled() {
        let mut w = inst();
        w.deterioration = 50.0;
        w.enabled = false;
        w.restore(10.0);
        assert_eq!(w.deterioration, 50.0);
    }
    #[test]
    fn REMOVE_no_op_when_already_zero() {
        let mut w = inst();
        w.restore(10.0);
        assert_eq!(w.deterioration, 0.0);
    }
    #[test]
    fn REMOVE_sets_just_restored_at_zero() {
        let mut w = inst();
        w.deterioration = 10.0;
        w.restore(10.0);
        assert!(w.just_restored);
    }
    #[test]
    fn REMOVE_no_just_restored_if_already_zero() {
        let mut w = inst();
        w.restore(1.0);
        assert!(!w.just_restored);
    }
    #[test]
    fn tick_increases() {
        let mut w = inst();
        w.tick(1.0);
        assert_eq!(w.deterioration, 10.0);
    }
    #[test]
    fn tick_scales_with_dt() {
        let mut w = inst();
        w.tick(2.0);
        assert_eq!(w.deterioration, 20.0);
    }
    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = inst();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.deterioration, 0.0);
    }
    #[test]
    fn tick_no_op_at_max() {
        let mut w = inst();
        w.deterioration = 100.0;
        w.tick(1.0);
        assert_eq!(w.deterioration, 100.0);
    }
    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = inst();
        w.decline_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.deterioration, 0.0);
    }
    #[test]
    fn is_ruined_true_at_max() {
        let mut w = inst();
        w.deterioration = 100.0;
        assert!(w.is_ruined());
    }
    #[test]
    fn is_ruined_false_below_max() {
        let mut w = inst();
        w.deterioration = 50.0;
        assert!(!w.is_ruined());
    }
    #[test]
    fn is_ruined_false_when_disabled() {
        let mut w = inst();
        w.deterioration = 100.0;
        w.enabled = false;
        assert!(!w.is_ruined());
    }
    #[test]
    fn is_restored_true_at_zero() {
        let w = inst();
        assert!(w.is_restored());
    }
    #[test]
    fn is_restored_false_above_zero() {
        let mut w = inst();
        w.deterioration = 1.0;
        assert!(!w.is_restored());
    }
    #[test]
    fn deterioration_fraction_zero_when_zero() {
        let w = inst();
        assert_eq!(w.deterioration_fraction(), 0.0);
    }
    #[test]
    fn deterioration_fraction_one_at_max() {
        let mut w = inst();
        w.deterioration = 100.0;
        assert_eq!(w.deterioration_fraction(), 1.0);
    }
    #[test]
    fn deterioration_fraction_half_at_midpoint() {
        let mut w = inst();
        w.deterioration = 50.0;
        assert_eq!(w.deterioration_fraction(), 0.5);
    }
    #[test]
    fn deterioration_fraction_zero_when_max_zero() {
        let mut w = inst();
        w.max_deterioration = 0.0;
        assert_eq!(w.deterioration_fraction(), 0.0);
    }
    #[test]
    fn effective_decay_scales() {
        let mut w = inst();
        w.deterioration = 50.0;
        assert_eq!(w.effective_decay(2.0), 1.0);
    }
    #[test]
    fn effective_decay_zero_when_zero() {
        let w = inst();
        assert_eq!(w.effective_decay(10.0), 0.0);
    }
    #[test]
    fn just_ruined_cleared_on_next_ADD() {
        let mut w = inst();
        w.decline(100.0);
        assert!(w.just_ruined);
        w.decline(1.0);
        assert!(!w.just_ruined);
    }
    #[test]
    fn just_restored_cleared_on_next_REMOVE() {
        let mut w = inst();
        w.deterioration = 10.0;
        w.restore(10.0);
        assert!(w.just_restored);
        w.deterioration = 10.0;
        w.restore(1.0);
        assert!(!w.just_restored);
    }
}
