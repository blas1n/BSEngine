use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wielder {
    pub control: f32,
    pub max_control: f32,
    pub mastery_rate: f32,
    pub just_mastered: bool,
    pub just_fumbled: bool,
    pub enabled: bool,
}

impl Default for Wielder {
    fn default() -> Self {
        Self {
            control: 0.0,
            max_control: 100.0,
            mastery_rate: 1.0,
            just_mastered: false,
            just_fumbled: false,
            enabled: true,
        }
    }
}

impl Wielder {
    pub fn master(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_mastered = false;
        self.just_fumbled = false;
        let prev = self.control;
        self.control = (self.control + amount).clamp(0.0, self.max_control);
        if self.control >= self.max_control && prev < self.max_control {
            self.just_mastered = true;
        }
    }

    pub fn fumble(&mut self, amount: f32) {
        if !self.enabled || self.control <= 0.0 {
            return;
        }
        self.just_mastered = false;
        self.just_fumbled = false;
        let prev = self.control;
        self.control = (self.control - amount).max(0.0);
        if self.control <= 0.0 && prev > 0.0 {
            self.just_fumbled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.control >= self.max_control {
            return;
        }
        self.master(self.mastery_rate * dt);
    }

    pub fn is_mastered(&self) -> bool {
        self.enabled && self.control >= self.max_control
    }

    pub fn is_fumbled(&self) -> bool {
        self.control <= 0.0
    }

    pub fn control_fraction(&self) -> f32 {
        if self.max_control <= 0.0 {
            return 0.0;
        }
        self.control / self.max_control
    }

    pub fn effective_grip(&self, scale: f32) -> f32 {
        self.control_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wielder() -> Wielder {
        Wielder {
            control: 0.0,
            max_control: 100.0,
            mastery_rate: 10.0,
            just_mastered: false,
            just_fumbled: false,
            enabled: true,
        }
    }

    #[test]
    fn default_control_zero() {
        let w = Wielder::default();
        assert_eq!(w.control, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wielder::default().enabled);
    }

    #[test]
    fn master_increases_control() {
        let mut w = wielder();
        w.master(30.0);
        assert_eq!(w.control, 30.0);
    }

    #[test]
    fn master_clamps_at_max() {
        let mut w = wielder();
        w.master(200.0);
        assert_eq!(w.control, 100.0);
    }

    #[test]
    fn master_no_op_when_disabled() {
        let mut w = wielder();
        w.enabled = false;
        w.master(50.0);
        assert_eq!(w.control, 0.0);
    }

    #[test]
    fn master_sets_just_mastered_at_max() {
        let mut w = wielder();
        w.master(100.0);
        assert!(w.just_mastered);
    }

    #[test]
    fn master_no_just_mastered_if_already_max() {
        let mut w = wielder();
        w.control = 100.0;
        w.master(1.0);
        assert!(!w.just_mastered);
    }

    #[test]
    fn fumble_decreases_control() {
        let mut w = wielder();
        w.control = 60.0;
        w.fumble(20.0);
        assert_eq!(w.control, 40.0);
    }

    #[test]
    fn fumble_clamps_at_zero() {
        let mut w = wielder();
        w.control = 30.0;
        w.fumble(200.0);
        assert_eq!(w.control, 0.0);
    }

    #[test]
    fn fumble_no_op_when_disabled() {
        let mut w = wielder();
        w.control = 50.0;
        w.enabled = false;
        w.fumble(10.0);
        assert_eq!(w.control, 50.0);
    }

    #[test]
    fn fumble_no_op_when_already_fumbled() {
        let mut w = wielder();
        w.fumble(10.0);
        assert_eq!(w.control, 0.0);
    }

    #[test]
    fn fumble_sets_just_fumbled_at_zero() {
        let mut w = wielder();
        w.control = 10.0;
        w.fumble(10.0);
        assert!(w.just_fumbled);
    }

    #[test]
    fn fumble_no_just_fumbled_if_already_zero() {
        let mut w = wielder();
        w.fumble(1.0);
        assert!(!w.just_fumbled);
    }

    #[test]
    fn tick_increases_control() {
        let mut w = wielder();
        w.tick(1.0);
        assert_eq!(w.control, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wielder();
        w.tick(2.0);
        assert_eq!(w.control, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wielder();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.control, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_mastered() {
        let mut w = wielder();
        w.control = 100.0;
        w.tick(1.0);
        assert_eq!(w.control, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wielder();
        w.mastery_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.control, 0.0);
    }

    #[test]
    fn is_mastered_true_at_max() {
        let mut w = wielder();
        w.control = 100.0;
        assert!(w.is_mastered());
    }

    #[test]
    fn is_mastered_false_below_max() {
        let mut w = wielder();
        w.control = 50.0;
        assert!(!w.is_mastered());
    }

    #[test]
    fn is_mastered_false_when_disabled() {
        let mut w = wielder();
        w.control = 100.0;
        w.enabled = false;
        assert!(!w.is_mastered());
    }

    #[test]
    fn is_fumbled_true_at_zero() {
        let w = wielder();
        assert!(w.is_fumbled());
    }

    #[test]
    fn is_fumbled_false_above_zero() {
        let mut w = wielder();
        w.control = 1.0;
        assert!(!w.is_fumbled());
    }

    #[test]
    fn control_fraction_zero_when_fumbled() {
        let w = wielder();
        assert_eq!(w.control_fraction(), 0.0);
    }

    #[test]
    fn control_fraction_one_at_max() {
        let mut w = wielder();
        w.control = 100.0;
        assert_eq!(w.control_fraction(), 1.0);
    }

    #[test]
    fn control_fraction_half_at_midpoint() {
        let mut w = wielder();
        w.control = 50.0;
        assert_eq!(w.control_fraction(), 0.5);
    }

    #[test]
    fn control_fraction_zero_when_max_zero() {
        let mut w = wielder();
        w.max_control = 0.0;
        assert_eq!(w.control_fraction(), 0.0);
    }

    #[test]
    fn effective_grip_scales() {
        let mut w = wielder();
        w.control = 50.0;
        assert_eq!(w.effective_grip(2.0), 1.0);
    }

    #[test]
    fn effective_grip_zero_when_fumbled() {
        let w = wielder();
        assert_eq!(w.effective_grip(10.0), 0.0);
    }

    #[test]
    fn just_mastered_cleared_on_next_master() {
        let mut w = wielder();
        w.master(100.0);
        assert!(w.just_mastered);
        w.master(1.0);
        assert!(!w.just_mastered);
    }

    #[test]
    fn just_fumbled_cleared_on_next_fumble() {
        let mut w = wielder();
        w.control = 10.0;
        w.fumble(10.0);
        assert!(w.just_fumbled);
        w.control = 10.0;
        w.fumble(1.0);
        assert!(!w.just_fumbled);
    }
}
