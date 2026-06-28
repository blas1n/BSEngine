use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrongly {
    pub error: f32,
    pub max_error: f32,
    pub deviate_rate: f32,
    pub just_mistaken: bool,
    pub just_corrected: bool,
    pub enabled: bool,
}

impl Default for Wrongly {
    fn default() -> Self {
        Self {
            error: 0.0,
            max_error: 100.0,
            deviate_rate: 1.0,
            just_mistaken: false,
            just_corrected: false,
            enabled: true,
        }
    }
}

impl Wrongly {
    pub fn deviate(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_mistaken = false;
        self.just_corrected = false;
        let prev = self.error;
        self.error = (self.error + amount).clamp(0.0, self.max_error);
        if self.error >= self.max_error && prev < self.max_error {
            self.just_mistaken = true;
        }
    }

    pub fn correct(&mut self, amount: f32) {
        if !self.enabled || self.error <= 0.0 {
            return;
        }
        self.just_mistaken = false;
        self.just_corrected = false;
        let prev = self.error;
        self.error = (self.error - amount).max(0.0);
        if self.error <= 0.0 && prev > 0.0 {
            self.just_corrected = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.error >= self.max_error {
            return;
        }
        self.deviate(self.deviate_rate * dt);
    }

    pub fn is_mistaken(&self) -> bool {
        self.enabled && self.error >= self.max_error
    }

    pub fn is_corrected(&self) -> bool {
        self.error <= 0.0
    }

    pub fn error_fraction(&self) -> f32 {
        if self.max_error <= 0.0 {
            return 0.0;
        }
        self.error / self.max_error
    }

    pub fn effective_fault(&self, scale: f32) -> f32 {
        self.error_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrongly() -> Wrongly {
        Wrongly {
            error: 0.0,
            max_error: 100.0,
            deviate_rate: 10.0,
            just_mistaken: false,
            just_corrected: false,
            enabled: true,
        }
    }

    #[test]
    fn default_error_zero() {
        let w = Wrongly::default();
        assert_eq!(w.error, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wrongly::default().enabled);
    }

    #[test]
    fn deviate_increases_error() {
        let mut w = wrongly();
        w.deviate(30.0);
        assert_eq!(w.error, 30.0);
    }

    #[test]
    fn deviate_clamps_at_max() {
        let mut w = wrongly();
        w.deviate(200.0);
        assert_eq!(w.error, 100.0);
    }

    #[test]
    fn deviate_no_op_when_disabled() {
        let mut w = wrongly();
        w.enabled = false;
        w.deviate(50.0);
        assert_eq!(w.error, 0.0);
    }

    #[test]
    fn deviate_sets_just_mistaken_at_max() {
        let mut w = wrongly();
        w.deviate(100.0);
        assert!(w.just_mistaken);
    }

    #[test]
    fn deviate_no_just_mistaken_if_already_max() {
        let mut w = wrongly();
        w.error = 100.0;
        w.deviate(1.0);
        assert!(!w.just_mistaken);
    }

    #[test]
    fn correct_decreases_error() {
        let mut w = wrongly();
        w.error = 60.0;
        w.correct(20.0);
        assert_eq!(w.error, 40.0);
    }

    #[test]
    fn correct_clamps_at_zero() {
        let mut w = wrongly();
        w.error = 30.0;
        w.correct(200.0);
        assert_eq!(w.error, 0.0);
    }

    #[test]
    fn correct_no_op_when_disabled() {
        let mut w = wrongly();
        w.error = 50.0;
        w.enabled = false;
        w.correct(10.0);
        assert_eq!(w.error, 50.0);
    }

    #[test]
    fn correct_no_op_when_already_corrected() {
        let mut w = wrongly();
        w.correct(10.0);
        assert_eq!(w.error, 0.0);
    }

    #[test]
    fn correct_sets_just_corrected_at_zero() {
        let mut w = wrongly();
        w.error = 10.0;
        w.correct(10.0);
        assert!(w.just_corrected);
    }

    #[test]
    fn correct_no_just_corrected_if_already_zero() {
        let mut w = wrongly();
        w.correct(1.0);
        assert!(!w.just_corrected);
    }

    #[test]
    fn tick_increases_error() {
        let mut w = wrongly();
        w.tick(1.0);
        assert_eq!(w.error, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wrongly();
        w.tick(2.0);
        assert_eq!(w.error, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wrongly();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.error, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_mistaken() {
        let mut w = wrongly();
        w.error = 100.0;
        w.tick(1.0);
        assert_eq!(w.error, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wrongly();
        w.deviate_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.error, 0.0);
    }

    #[test]
    fn is_mistaken_true_at_max() {
        let mut w = wrongly();
        w.error = 100.0;
        assert!(w.is_mistaken());
    }

    #[test]
    fn is_mistaken_false_below_max() {
        let mut w = wrongly();
        w.error = 50.0;
        assert!(!w.is_mistaken());
    }

    #[test]
    fn is_mistaken_false_when_disabled() {
        let mut w = wrongly();
        w.error = 100.0;
        w.enabled = false;
        assert!(!w.is_mistaken());
    }

    #[test]
    fn is_corrected_true_at_zero() {
        let w = wrongly();
        assert!(w.is_corrected());
    }

    #[test]
    fn is_corrected_false_above_zero() {
        let mut w = wrongly();
        w.error = 1.0;
        assert!(!w.is_corrected());
    }

    #[test]
    fn error_fraction_zero_when_corrected() {
        let w = wrongly();
        assert_eq!(w.error_fraction(), 0.0);
    }

    #[test]
    fn error_fraction_one_at_max() {
        let mut w = wrongly();
        w.error = 100.0;
        assert_eq!(w.error_fraction(), 1.0);
    }

    #[test]
    fn error_fraction_half_at_midpoint() {
        let mut w = wrongly();
        w.error = 50.0;
        assert_eq!(w.error_fraction(), 0.5);
    }

    #[test]
    fn error_fraction_zero_when_max_zero() {
        let mut w = wrongly();
        w.max_error = 0.0;
        assert_eq!(w.error_fraction(), 0.0);
    }

    #[test]
    fn effective_fault_scales() {
        let mut w = wrongly();
        w.error = 50.0;
        assert_eq!(w.effective_fault(2.0), 1.0);
    }

    #[test]
    fn effective_fault_zero_when_corrected() {
        let w = wrongly();
        assert_eq!(w.effective_fault(10.0), 0.0);
    }

    #[test]
    fn just_mistaken_cleared_on_next_deviate() {
        let mut w = wrongly();
        w.deviate(100.0);
        assert!(w.just_mistaken);
        w.deviate(1.0);
        assert!(!w.just_mistaken);
    }

    #[test]
    fn just_corrected_cleared_on_next_correct() {
        let mut w = wrongly();
        w.error = 10.0;
        w.correct(10.0);
        assert!(w.just_corrected);
        w.error = 10.0;
        w.correct(1.0);
        assert!(!w.just_corrected);
    }
}
