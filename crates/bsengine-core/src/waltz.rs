use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Waltz {
    pub rhythm: f32,
    pub max_rhythm: f32,
    pub grace_rate: f32,
    pub just_dancing: bool,
    pub just_still: bool,
    pub enabled: bool,
}

impl Default for Waltz {
    fn default() -> Self {
        Self {
            rhythm: 0.0,
            max_rhythm: 100.0,
            grace_rate: 1.0,
            just_dancing: false,
            just_still: false,
            enabled: true,
        }
    }
}

impl Waltz {
    pub fn step(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_dancing = false;
        self.just_still = false;
        let prev = self.rhythm;
        self.rhythm = (self.rhythm + amount).clamp(0.0, self.max_rhythm);
        if self.rhythm >= self.max_rhythm && prev < self.max_rhythm {
            self.just_dancing = true;
        }
    }

    pub fn halt(&mut self, amount: f32) {
        if !self.enabled || self.rhythm <= 0.0 {
            return;
        }
        self.just_dancing = false;
        self.just_still = false;
        let prev = self.rhythm;
        self.rhythm = (self.rhythm - amount).max(0.0);
        if self.rhythm <= 0.0 && prev > 0.0 {
            self.just_still = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.rhythm >= self.max_rhythm {
            return;
        }
        self.step(self.grace_rate * dt);
    }

    pub fn is_dancing(&self) -> bool {
        self.enabled && self.rhythm >= self.max_rhythm
    }

    pub fn is_still(&self) -> bool {
        self.rhythm <= 0.0
    }

    pub fn rhythm_fraction(&self) -> f32 {
        if self.max_rhythm <= 0.0 {
            return 0.0;
        }
        self.rhythm / self.max_rhythm
    }

    pub fn effective_grace(&self, scale: f32) -> f32 {
        self.rhythm_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waltz() -> Waltz {
        Waltz {
            rhythm: 0.0,
            max_rhythm: 100.0,
            grace_rate: 10.0,
            just_dancing: false,
            just_still: false,
            enabled: true,
        }
    }

    #[test]
    fn default_rhythm_zero() {
        let w = Waltz::default();
        assert_eq!(w.rhythm, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Waltz::default().enabled);
    }

    #[test]
    fn step_increases_rhythm() {
        let mut w = waltz();
        w.step(30.0);
        assert_eq!(w.rhythm, 30.0);
    }

    #[test]
    fn step_clamps_at_max() {
        let mut w = waltz();
        w.step(200.0);
        assert_eq!(w.rhythm, 100.0);
    }

    #[test]
    fn step_no_op_when_disabled() {
        let mut w = waltz();
        w.enabled = false;
        w.step(50.0);
        assert_eq!(w.rhythm, 0.0);
    }

    #[test]
    fn step_sets_just_dancing_at_max() {
        let mut w = waltz();
        w.step(100.0);
        assert!(w.just_dancing);
    }

    #[test]
    fn step_no_just_dancing_if_already_max() {
        let mut w = waltz();
        w.rhythm = 100.0;
        w.step(1.0);
        assert!(!w.just_dancing);
    }

    #[test]
    fn halt_decreases_rhythm() {
        let mut w = waltz();
        w.rhythm = 60.0;
        w.halt(20.0);
        assert_eq!(w.rhythm, 40.0);
    }

    #[test]
    fn halt_clamps_at_zero() {
        let mut w = waltz();
        w.rhythm = 30.0;
        w.halt(200.0);
        assert_eq!(w.rhythm, 0.0);
    }

    #[test]
    fn halt_no_op_when_disabled() {
        let mut w = waltz();
        w.rhythm = 50.0;
        w.enabled = false;
        w.halt(10.0);
        assert_eq!(w.rhythm, 50.0);
    }

    #[test]
    fn halt_no_op_when_already_still() {
        let mut w = waltz();
        w.halt(10.0);
        assert_eq!(w.rhythm, 0.0);
    }

    #[test]
    fn halt_sets_just_still_at_zero() {
        let mut w = waltz();
        w.rhythm = 10.0;
        w.halt(10.0);
        assert!(w.just_still);
    }

    #[test]
    fn halt_no_just_still_if_already_zero() {
        let mut w = waltz();
        w.halt(1.0);
        assert!(!w.just_still);
    }

    #[test]
    fn tick_increases_rhythm() {
        let mut w = waltz();
        w.tick(1.0);
        assert_eq!(w.rhythm, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = waltz();
        w.tick(2.0);
        assert_eq!(w.rhythm, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = waltz();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.rhythm, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_dancing() {
        let mut w = waltz();
        w.rhythm = 100.0;
        w.tick(1.0);
        assert_eq!(w.rhythm, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = waltz();
        w.grace_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.rhythm, 0.0);
    }

    #[test]
    fn is_dancing_true_at_max() {
        let mut w = waltz();
        w.rhythm = 100.0;
        assert!(w.is_dancing());
    }

    #[test]
    fn is_dancing_false_below_max() {
        let mut w = waltz();
        w.rhythm = 50.0;
        assert!(!w.is_dancing());
    }

    #[test]
    fn is_dancing_false_when_disabled() {
        let mut w = waltz();
        w.rhythm = 100.0;
        w.enabled = false;
        assert!(!w.is_dancing());
    }

    #[test]
    fn is_still_true_at_zero() {
        let w = waltz();
        assert!(w.is_still());
    }

    #[test]
    fn is_still_false_above_zero() {
        let mut w = waltz();
        w.rhythm = 1.0;
        assert!(!w.is_still());
    }

    #[test]
    fn rhythm_fraction_zero_when_still() {
        let w = waltz();
        assert_eq!(w.rhythm_fraction(), 0.0);
    }

    #[test]
    fn rhythm_fraction_one_at_max() {
        let mut w = waltz();
        w.rhythm = 100.0;
        assert_eq!(w.rhythm_fraction(), 1.0);
    }

    #[test]
    fn rhythm_fraction_half_at_midpoint() {
        let mut w = waltz();
        w.rhythm = 50.0;
        assert_eq!(w.rhythm_fraction(), 0.5);
    }

    #[test]
    fn rhythm_fraction_zero_when_max_zero() {
        let mut w = waltz();
        w.max_rhythm = 0.0;
        assert_eq!(w.rhythm_fraction(), 0.0);
    }

    #[test]
    fn effective_grace_scales() {
        let mut w = waltz();
        w.rhythm = 50.0;
        assert_eq!(w.effective_grace(2.0), 1.0);
    }

    #[test]
    fn effective_grace_zero_when_still() {
        let w = waltz();
        assert_eq!(w.effective_grace(10.0), 0.0);
    }

    #[test]
    fn just_dancing_cleared_on_next_step() {
        let mut w = waltz();
        w.step(100.0);
        assert!(w.just_dancing);
        w.step(1.0);
        assert!(!w.just_dancing);
    }

    #[test]
    fn just_still_cleared_on_next_halt() {
        let mut w = waltz();
        w.rhythm = 10.0;
        w.halt(10.0);
        assert!(w.just_still);
        w.rhythm = 10.0;
        w.halt(1.0);
        assert!(!w.just_still);
    }
}
