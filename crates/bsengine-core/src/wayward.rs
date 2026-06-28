use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wayward {
    pub deviation: f32,
    pub max_deviation: f32,
    pub stray_rate: f32,
    pub just_errant: bool,
    pub just_corrected: bool,
    pub enabled: bool,
}

impl Default for Wayward {
    fn default() -> Self {
        Self {
            deviation: 0.0,
            max_deviation: 100.0,
            stray_rate: 1.0,
            just_errant: false,
            just_corrected: false,
            enabled: true,
        }
    }
}

impl Wayward {
    pub fn stray(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_errant = false;
        self.just_corrected = false;
        let prev = self.deviation;
        self.deviation = (self.deviation + amount).clamp(0.0, self.max_deviation);
        if self.deviation >= self.max_deviation && prev < self.max_deviation {
            self.just_errant = true;
        }
    }

    pub fn correct(&mut self, amount: f32) {
        if !self.enabled || self.deviation <= 0.0 {
            return;
        }
        self.just_errant = false;
        self.just_corrected = false;
        let prev = self.deviation;
        self.deviation = (self.deviation - amount).max(0.0);
        if self.deviation <= 0.0 && prev > 0.0 {
            self.just_corrected = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.deviation >= self.max_deviation {
            return;
        }
        self.stray(self.stray_rate * dt);
    }

    pub fn is_errant(&self) -> bool {
        self.enabled && self.deviation >= self.max_deviation
    }
    pub fn is_corrected(&self) -> bool {
        self.deviation <= 0.0
    }

    pub fn deviation_fraction(&self) -> f32 {
        if self.max_deviation <= 0.0 {
            return 0.0;
        }
        self.deviation / self.max_deviation
    }

    pub fn effective_wander(&self, scale: f32) -> f32 {
        self.deviation_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wayward() -> Wayward {
        Wayward {
            deviation: 0.0,
            max_deviation: 100.0,
            stray_rate: 10.0,
            just_errant: false,
            just_corrected: false,
            enabled: true,
        }
    }

    #[test]
    fn default_deviation_zero() {
        assert_eq!(Wayward::default().deviation, 0.0);
    }
    #[test]
    fn default_enabled() {
        assert!(Wayward::default().enabled);
    }
    #[test]
    fn stray_increases_deviation() {
        let mut w = wayward();
        w.stray(30.0);
        assert_eq!(w.deviation, 30.0);
    }
    #[test]
    fn stray_clamps_at_max() {
        let mut w = wayward();
        w.stray(200.0);
        assert_eq!(w.deviation, 100.0);
    }
    #[test]
    fn stray_no_op_when_disabled() {
        let mut w = wayward();
        w.enabled = false;
        w.stray(50.0);
        assert_eq!(w.deviation, 0.0);
    }
    #[test]
    fn stray_sets_just_errant_at_max() {
        let mut w = wayward();
        w.stray(100.0);
        assert!(w.just_errant);
    }
    #[test]
    fn stray_no_just_errant_if_already_max() {
        let mut w = wayward();
        w.deviation = 100.0;
        w.stray(1.0);
        assert!(!w.just_errant);
    }
    #[test]
    fn correct_decreases_deviation() {
        let mut w = wayward();
        w.deviation = 60.0;
        w.correct(20.0);
        assert_eq!(w.deviation, 40.0);
    }
    #[test]
    fn correct_clamps_at_zero() {
        let mut w = wayward();
        w.deviation = 30.0;
        w.correct(200.0);
        assert_eq!(w.deviation, 0.0);
    }
    #[test]
    fn correct_no_op_when_disabled() {
        let mut w = wayward();
        w.deviation = 50.0;
        w.enabled = false;
        w.correct(10.0);
        assert_eq!(w.deviation, 50.0);
    }
    #[test]
    fn correct_no_op_when_already_corrected() {
        let mut w = wayward();
        w.correct(10.0);
        assert_eq!(w.deviation, 0.0);
    }
    #[test]
    fn correct_sets_just_corrected_at_zero() {
        let mut w = wayward();
        w.deviation = 10.0;
        w.correct(10.0);
        assert!(w.just_corrected);
    }
    #[test]
    fn correct_no_just_corrected_if_already_zero() {
        let mut w = wayward();
        w.correct(1.0);
        assert!(!w.just_corrected);
    }
    #[test]
    fn tick_increases_deviation() {
        let mut w = wayward();
        w.tick(1.0);
        assert_eq!(w.deviation, 10.0);
    }
    #[test]
    fn tick_scales_with_dt() {
        let mut w = wayward();
        w.tick(2.0);
        assert_eq!(w.deviation, 20.0);
    }
    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wayward();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.deviation, 0.0);
    }
    #[test]
    fn tick_no_op_when_already_errant() {
        let mut w = wayward();
        w.deviation = 100.0;
        w.tick(1.0);
        assert_eq!(w.deviation, 100.0);
    }
    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wayward();
        w.stray_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.deviation, 0.0);
    }
    #[test]
    fn is_errant_true_at_max() {
        let mut w = wayward();
        w.deviation = 100.0;
        assert!(w.is_errant());
    }
    #[test]
    fn is_errant_false_below_max() {
        let mut w = wayward();
        w.deviation = 50.0;
        assert!(!w.is_errant());
    }
    #[test]
    fn is_errant_false_when_disabled() {
        let mut w = wayward();
        w.deviation = 100.0;
        w.enabled = false;
        assert!(!w.is_errant());
    }
    #[test]
    fn is_corrected_true_at_zero() {
        let w = wayward();
        assert!(w.is_corrected());
    }
    #[test]
    fn is_corrected_false_above_zero() {
        let mut w = wayward();
        w.deviation = 1.0;
        assert!(!w.is_corrected());
    }
    #[test]
    fn deviation_fraction_zero_when_corrected() {
        let w = wayward();
        assert_eq!(w.deviation_fraction(), 0.0);
    }
    #[test]
    fn deviation_fraction_one_at_max() {
        let mut w = wayward();
        w.deviation = 100.0;
        assert_eq!(w.deviation_fraction(), 1.0);
    }
    #[test]
    fn deviation_fraction_half_at_midpoint() {
        let mut w = wayward();
        w.deviation = 50.0;
        assert_eq!(w.deviation_fraction(), 0.5);
    }
    #[test]
    fn deviation_fraction_zero_when_max_zero() {
        let mut w = wayward();
        w.max_deviation = 0.0;
        assert_eq!(w.deviation_fraction(), 0.0);
    }
    #[test]
    fn effective_wander_scales() {
        let mut w = wayward();
        w.deviation = 50.0;
        assert_eq!(w.effective_wander(2.0), 1.0);
    }
    #[test]
    fn effective_wander_zero_when_corrected() {
        let w = wayward();
        assert_eq!(w.effective_wander(10.0), 0.0);
    }
    #[test]
    fn just_errant_cleared_on_next_stray() {
        let mut w = wayward();
        w.stray(100.0);
        assert!(w.just_errant);
        w.stray(1.0);
        assert!(!w.just_errant);
    }
    #[test]
    fn just_corrected_cleared_on_next_correct() {
        let mut w = wayward();
        w.deviation = 10.0;
        w.correct(10.0);
        assert!(w.just_corrected);
        w.deviation = 10.0;
        w.correct(1.0);
        assert!(!w.just_corrected);
    }
}
