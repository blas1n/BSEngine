use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wean {
    pub dependence: f32,
    pub max_dependence: f32,
    pub detach_rate: f32,
    pub just_weaned: bool,
    pub just_relapsed: bool,
    pub enabled: bool,
}

impl Default for Wean {
    fn default() -> Self {
        Self {
            dependence: 0.0,
            max_dependence: 100.0,
            detach_rate: 1.0,
            just_weaned: false,
            just_relapsed: false,
            enabled: true,
        }
    }
}

impl Wean {
    pub fn detach(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_weaned = false;
        self.just_relapsed = false;
        let prev = self.dependence;
        self.dependence = (self.dependence + amount).clamp(0.0, self.max_dependence);
        if self.dependence >= self.max_dependence && prev < self.max_dependence {
            self.just_weaned = true;
        }
    }

    pub fn relapse(&mut self, amount: f32) {
        if !self.enabled || self.dependence <= 0.0 {
            return;
        }
        self.just_weaned = false;
        self.just_relapsed = false;
        let prev = self.dependence;
        self.dependence = (self.dependence - amount).max(0.0);
        if self.dependence <= 0.0 && prev > 0.0 {
            self.just_relapsed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.dependence >= self.max_dependence {
            return;
        }
        self.detach(self.detach_rate * dt);
    }

    pub fn is_weaned(&self) -> bool {
        self.enabled && self.dependence >= self.max_dependence
    }

    pub fn is_relapsed(&self) -> bool {
        self.dependence <= 0.0
    }

    pub fn dependence_fraction(&self) -> f32 {
        if self.max_dependence <= 0.0 {
            return 0.0;
        }
        self.dependence / self.max_dependence
    }

    pub fn effective_independence(&self, scale: f32) -> f32 {
        self.dependence_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wean() -> Wean {
        Wean {
            dependence: 0.0,
            max_dependence: 100.0,
            detach_rate: 10.0,
            just_weaned: false,
            just_relapsed: false,
            enabled: true,
        }
    }

    #[test]
    fn default_dependence_zero() {
        let w = Wean::default();
        assert_eq!(w.dependence, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wean::default().enabled);
    }

    #[test]
    fn detach_increases_dependence() {
        let mut w = wean();
        w.detach(30.0);
        assert_eq!(w.dependence, 30.0);
    }

    #[test]
    fn detach_clamps_at_max() {
        let mut w = wean();
        w.detach(200.0);
        assert_eq!(w.dependence, 100.0);
    }

    #[test]
    fn detach_no_op_when_disabled() {
        let mut w = wean();
        w.enabled = false;
        w.detach(50.0);
        assert_eq!(w.dependence, 0.0);
    }

    #[test]
    fn detach_sets_just_weaned_at_max() {
        let mut w = wean();
        w.detach(100.0);
        assert!(w.just_weaned);
    }

    #[test]
    fn detach_no_just_weaned_if_already_max() {
        let mut w = wean();
        w.dependence = 100.0;
        w.detach(1.0);
        assert!(!w.just_weaned);
    }

    #[test]
    fn relapse_decreases_dependence() {
        let mut w = wean();
        w.dependence = 60.0;
        w.relapse(20.0);
        assert_eq!(w.dependence, 40.0);
    }

    #[test]
    fn relapse_clamps_at_zero() {
        let mut w = wean();
        w.dependence = 30.0;
        w.relapse(200.0);
        assert_eq!(w.dependence, 0.0);
    }

    #[test]
    fn relapse_no_op_when_disabled() {
        let mut w = wean();
        w.dependence = 50.0;
        w.enabled = false;
        w.relapse(10.0);
        assert_eq!(w.dependence, 50.0);
    }

    #[test]
    fn relapse_no_op_when_already_relapsed() {
        let mut w = wean();
        w.relapse(10.0);
        assert_eq!(w.dependence, 0.0);
    }

    #[test]
    fn relapse_sets_just_relapsed_at_zero() {
        let mut w = wean();
        w.dependence = 10.0;
        w.relapse(10.0);
        assert!(w.just_relapsed);
    }

    #[test]
    fn relapse_no_just_relapsed_if_already_zero() {
        let mut w = wean();
        w.relapse(1.0);
        assert!(!w.just_relapsed);
    }

    #[test]
    fn tick_increases_dependence() {
        let mut w = wean();
        w.tick(1.0);
        assert_eq!(w.dependence, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wean();
        w.tick(2.0);
        assert_eq!(w.dependence, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wean();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.dependence, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_weaned() {
        let mut w = wean();
        w.dependence = 100.0;
        w.tick(1.0);
        assert_eq!(w.dependence, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wean();
        w.detach_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.dependence, 0.0);
    }

    #[test]
    fn is_weaned_true_at_max() {
        let mut w = wean();
        w.dependence = 100.0;
        assert!(w.is_weaned());
    }

    #[test]
    fn is_weaned_false_below_max() {
        let mut w = wean();
        w.dependence = 50.0;
        assert!(!w.is_weaned());
    }

    #[test]
    fn is_weaned_false_when_disabled() {
        let mut w = wean();
        w.dependence = 100.0;
        w.enabled = false;
        assert!(!w.is_weaned());
    }

    #[test]
    fn is_relapsed_true_at_zero() {
        let w = wean();
        assert!(w.is_relapsed());
    }

    #[test]
    fn is_relapsed_false_above_zero() {
        let mut w = wean();
        w.dependence = 1.0;
        assert!(!w.is_relapsed());
    }

    #[test]
    fn dependence_fraction_zero_when_relapsed() {
        let w = wean();
        assert_eq!(w.dependence_fraction(), 0.0);
    }

    #[test]
    fn dependence_fraction_one_at_max() {
        let mut w = wean();
        w.dependence = 100.0;
        assert_eq!(w.dependence_fraction(), 1.0);
    }

    #[test]
    fn dependence_fraction_half_at_midpoint() {
        let mut w = wean();
        w.dependence = 50.0;
        assert_eq!(w.dependence_fraction(), 0.5);
    }

    #[test]
    fn dependence_fraction_zero_when_max_zero() {
        let mut w = wean();
        w.max_dependence = 0.0;
        assert_eq!(w.dependence_fraction(), 0.0);
    }

    #[test]
    fn effective_independence_scales() {
        let mut w = wean();
        w.dependence = 50.0;
        assert_eq!(w.effective_independence(2.0), 1.0);
    }

    #[test]
    fn effective_independence_zero_when_relapsed() {
        let w = wean();
        assert_eq!(w.effective_independence(10.0), 0.0);
    }

    #[test]
    fn just_weaned_cleared_on_next_detach() {
        let mut w = wean();
        w.detach(100.0);
        assert!(w.just_weaned);
        w.detach(1.0);
        assert!(!w.just_weaned);
    }

    #[test]
    fn just_relapsed_cleared_on_next_relapse() {
        let mut w = wean();
        w.dependence = 10.0;
        w.relapse(10.0);
        assert!(w.just_relapsed);
        w.dependence = 10.0;
        w.relapse(1.0);
        assert!(!w.just_relapsed);
    }
}
